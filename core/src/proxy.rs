//! Shared proxy configuration for grove's `ureq`-based HTTP clients.
//!
//! `ureq::Proxy::try_from_env()` reads `ALL_PROXY`/`HTTPS_PROXY`/`HTTP_PROXY`
//! (and lowercase variants) plus `NO_PROXY`, but silently returns `None` if a
//! configured value fails to parse — indistinguishable from "no proxy
//! configured". [`configured_proxy`] is the single place all of grove's ureq
//! agents (registry fetch, the mcp-llm explorer's chat client, and its health
//! probe / engine discovery) get their proxy setting from, and it adds a
//! warning for the malformed-value case so a typo'd proxy URL doesn't silently
//! fall back to a direct (and possibly blocked) connection.

const PROXY_ENV_VARS: &[&str] =
    &["ALL_PROXY", "all_proxy", "HTTPS_PROXY", "https_proxy", "HTTP_PROXY", "http_proxy"];

/// Resolve the proxy to use for a grove-initiated HTTP request, from the
/// standard `ALL_PROXY`/`HTTPS_PROXY`/`HTTP_PROXY` (+ `NO_PROXY`) environment
/// variables. Returns `None` if no proxy is configured, or if the configured
/// value fails to parse (in which case a warning is printed so the failure
/// isn't silent — see the module docs).
///
/// Callers that must never be proxied (e.g. local inference-engine discovery
/// probing fixed `localhost` ports) should not call this — pass `None` for
/// their agent's proxy explicitly instead.
pub fn configured_proxy() -> Option<ureq::Proxy> {
    let proxy = ureq::Proxy::try_from_env();
    if proxy.is_none() {
        if let Some(name) =
            PROXY_ENV_VARS.iter().find(|name| std::env::var(name).is_ok_and(|v| !v.is_empty()))
        {
            eprintln!("grove: ignoring invalid proxy URL in ${name}");
        }
    }
    proxy
}

/// A `ureq::Config` builder pre-wired with [`configured_proxy`].
///
/// The single place every grove ureq agent starts from, so a new call site
/// can't forget to thread the proxy through — it just layers its own
/// timeouts/options on top and calls `.build().new_agent()`. Callers that must
/// never be proxied (e.g. local inference-engine discovery) should not use
/// this — build from `ureq::config::Config::builder()` directly and pass
/// `.proxy(None)` or omit it.
pub fn default_config() -> ureq::config::ConfigBuilder<ureq::typestate::AgentScope> {
    ureq::config::Config::builder().proxy(configured_proxy())
}

#[cfg(test)]
mod default_config_tests {
    use super::default_config;

    #[test]
    fn builds_an_agent() {
        // Smoke test: the builder chain type-checks and produces an agent.
        let _agent = default_config().build().new_agent();
    }
}

/// Process-wide lock serializing every test (in this module and in
/// `fetch.rs`) that mutates the proxy env vars — they're process-global, so
/// two such tests racing under cargo's default parallel test runner can
/// observe each other's values and fail spuriously.
#[cfg(test)]
pub(crate) static PROXY_ENV_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    // Proxy env vars are process-global; serialize the tests that mutate them
    // so they don't race across threads (including with fetch.rs's own proxy
    // test — see `PROXY_ENV_TEST_LOCK`).
    use super::PROXY_ENV_TEST_LOCK as ENV_LOCK;

    struct EnvVarGuard {
        vars: Vec<(&'static str, Option<String>)>,
    }

    impl EnvVarGuard {
        fn set(pairs: &[(&'static str, &str)]) -> Self {
            let vars = pairs
                .iter()
                .map(|(name, value)| {
                    let prev = std::env::var(name).ok();
                    unsafe { std::env::set_var(name, value) };
                    (*name, prev)
                })
                .collect();
            EnvVarGuard { vars }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            for (name, prev) in &self.vars {
                match prev {
                    Some(v) => unsafe { std::env::set_var(name, v) },
                    None => unsafe { std::env::remove_var(name) },
                }
            }
        }
    }

    #[test]
    fn no_proxy_env_yields_none() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _guard = EnvVarGuard::set(&[
            ("ALL_PROXY", ""),
            ("HTTPS_PROXY", ""),
            ("HTTP_PROXY", ""),
            ("NO_PROXY", ""),
        ]);
        for name in PROXY_ENV_VARS {
            unsafe { std::env::remove_var(name) };
        }
        assert!(configured_proxy().is_none());
    }

    #[test]
    fn http_proxy_env_is_picked_up() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _guard = EnvVarGuard::set(&[("HTTP_PROXY", "http://127.0.0.1:9999")]);
        let proxy = configured_proxy().expect("HTTP_PROXY should be picked up");
        assert_eq!(proxy.host(), "127.0.0.1");
        assert_eq!(proxy.port(), 9999);
    }

    #[test]
    fn no_proxy_excludes_matching_host() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _guard = EnvVarGuard::set(&[
            ("HTTP_PROXY", "http://127.0.0.1:9999"),
            ("NO_PROXY", "example.test"),
        ]);
        let proxy = configured_proxy().expect("HTTP_PROXY should still be picked up");
        let bypassed: ureq::http::Uri = "http://example.test/".parse().unwrap();
        assert!(proxy.is_no_proxy(&bypassed), "example.test should bypass the proxy per NO_PROXY");
    }

    #[test]
    fn malformed_proxy_url_is_ignored_not_panicking() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _guard = EnvVarGuard::set(&[("HTTP_PROXY", "://not a valid uri")]);
        // Must not panic; a malformed value degrades to "no proxy" (with a
        // warning on stderr, not asserted here).
        assert!(configured_proxy().is_none());
    }

    /// The round-trip test the original PR review praised: prove a request is
    /// actually routed through the configured proxy, not just that the env var
    /// parsed. `example.invalid` (RFC 2606) can't resolve via DNS, so a
    /// successful call here is only possible if the request went to our fake
    /// local proxy listener instead of attempting a direct connection.
    #[test]
    fn request_is_actually_routed_through_configured_proxy() {
        let _lock = ENV_LOCK.lock().unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let _guard = EnvVarGuard::set(&[("HTTP_PROXY", &format!("http://127.0.0.1:{port}"))]);

        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();

            // A single `read()` can return a partial request (the OS is free
            // to deliver a small write as more than one segment), so drain
            // the stream until the header terminator shows up.
            fn read_headers(stream: &mut std::net::TcpStream) -> String {
                let mut acc = String::new();
                let mut buf = [0u8; 512];
                while !acc.contains("\r\n\r\n") {
                    let n = stream.read(&mut buf).unwrap();
                    assert!(n > 0, "peer closed before sending a full request");
                    acc.push_str(&String::from_utf8_lossy(&buf[..n]));
                }
                acc
            }

            loop {
                let raw = read_headers(&mut stream);
                if raw.starts_with("CONNECT") {
                    // ureq tunnels through an HTTP proxy via CONNECT even for
                    // a plain `http://` target; acknowledge the tunnel and
                    // keep reading the real request on the same connection.
                    stream.write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n").unwrap();
                    continue;
                }
                stream
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                    .unwrap();
                return raw;
            }
        });

        let agent: ureq::Agent = ureq::config::Config::builder()
            .proxy(configured_proxy())
            .build()
            .new_agent();
        let resp = agent
            .get("http://example.invalid/proxied-path")
            .call()
            .expect("the request should succeed via the local fake proxy");
        assert_eq!(resp.status(), 200);

        let raw_request = handle.join().unwrap();
        assert!(
            raw_request.contains("/proxied-path") && raw_request.contains("example.invalid"),
            "expected the final request (post-CONNECT) to target /proxied-path on \
             example.invalid, got: {raw_request}"
        );
    }
}
