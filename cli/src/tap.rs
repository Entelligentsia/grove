//! `grove tap` — a logging reverse proxy for explore-mode LLM traffic.
//!
//! Sits in front of the configured OpenAI-compatible provider and prints every
//! request (system / user / tool messages, the model's tool calls, sampling
//! params) and response (assistant content, tool calls, token usage), so you can
//! watch the inner explorer's conversation with the local model. Requests are
//! forwarded unchanged — point grove at the tap by setting the explore `base_url`
//! to `http://localhost:<port>/v1` (via `grove config` or `.grove/explore.json`).
//!
//! A debug aid for the experimental mcp-llm mode; not on any hot path. The server
//! side is hand-rolled on [`TcpListener`] (grove is dependency-light); the upstream
//! leg uses `ureq`. Non-streaming only, which is all the explorer's client emits.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;

use anyhow::{Context, Result};
use serde_json::Value;

use grove_core::ExploreConfig;

/// Run the tap: listen on `listen`, forward to `upstream` (or the provider from
/// the explore config), logging bodies. Blocks, serving until interrupted.
pub fn run(root: &Path, listen: u16, upstream: Option<String>, brief: bool) -> Result<()> {
    let upstream = resolve_upstream(root, upstream)?;
    let addr = format!("127.0.0.1:{listen}");
    let listener = TcpListener::bind(&addr).with_context(|| format!("could not bind {addr}"))?;
    eprintln!("grove tap: {addr} -> {upstream}");
    eprintln!("  point your explore base_url at http://{addr}/v1");
    eprintln!("  (run `grove config`, or edit .grove/explore.json) — Ctrl-C to stop\n");
    for stream in listener.incoming() {
        match stream {
            Ok(s) => {
                let up = upstream.clone();
                std::thread::spawn(move || {
                    if let Err(e) = handle(s, &up, brief) {
                        eprintln!("grove tap: connection error: {e}");
                    }
                });
            }
            Err(e) => eprintln!("grove tap: accept error: {e}"),
        }
    }
    Ok(())
}

/// Resolve the upstream base: explicit `--upstream`, else the explore config's
/// `base_url` with any trailing `/v1` stripped (the incoming path carries it).
fn resolve_upstream(root: &Path, upstream: Option<String>) -> Result<String> {
    let raw = match upstream {
        Some(u) => u,
        None => {
            let cfg = ExploreConfig::load(root).context(
                "no --upstream given and no .grove/explore.json to derive it from — pass --upstream <url>",
            )?;
            cfg.base_url
        }
    };
    Ok(strip_v1(&raw))
}

/// `http://host:port/v1/` -> `http://host:port` (the tap re-appends the path,
/// which already includes `/v1`).
fn strip_v1(raw: &str) -> String {
    let base = raw.trim_end_matches('/');
    let base = base.strip_suffix("/v1").unwrap_or(base);
    base.trim_end_matches('/').to_string()
}

fn handle(stream: TcpStream, upstream: &str, brief: bool) -> Result<()> {
    stream.set_nodelay(true).ok();
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut writer = stream;

    // Request line.
    let mut request_line = String::new();
    if reader.read_line(&mut request_line)? == 0 {
        return Ok(()); // client closed
    }
    let mut it = request_line.split_whitespace();
    let method = it.next().unwrap_or("").to_string();
    let path = it.next().unwrap_or("/").to_string();
    if method.is_empty() {
        return Ok(());
    }

    // Headers.
    let mut headers: Vec<(String, String)> = Vec::new();
    let mut content_length = 0usize;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            break;
        }
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some((k, v)) = line.split_once(':') {
            let (k, v) = (k.trim(), v.trim());
            if k.eq_ignore_ascii_case("content-length") {
                content_length = v.parse().unwrap_or(0);
            }
            headers.push((k.to_string(), v.to_string()));
        }
    }

    // Body.
    let mut body = vec![0u8; content_length];
    if content_length > 0 {
        reader.read_exact(&mut body)?;
    }

    let is_post = method.eq_ignore_ascii_case("POST");
    if is_post {
        log_request(&path, &body, brief);
    }

    // Forward via ureq.
    let url = format!("{upstream}{path}");
    let mut req = ureq::request(&method, &url);
    for (k, v) in &headers {
        let kl = k.to_ascii_lowercase();
        if kl == "host" || kl == "content-length" || kl == "connection" || kl == "accept-encoding" {
            continue;
        }
        req = req.set(k, v);
    }
    let t0 = std::time::Instant::now();
    let result = if body.is_empty() {
        req.call()
    } else {
        req.send_bytes(&body)
    };
    let (status, resp_headers, resp_body) = match result {
        Ok(r) => read_response(r),
        Err(ureq::Error::Status(code, r)) => {
            let (_s, h, b) = read_response(r);
            (code, h, b)
        }
        Err(e) => {
            eprintln!("grove tap: upstream {upstream} error: {e}");
            let msg = format!("{{\"error\":\"grove tap: upstream unreachable: {e}\"}}");
            return write_response(
                &mut writer,
                502,
                &[("Content-Type".to_string(), "application/json".to_string())],
                msg.as_bytes(),
            );
        }
    };
    let ms = t0.elapsed().as_millis();
    if is_post {
        log_response(&path, &resp_body, ms, brief);
    }
    write_response(&mut writer, status, &resp_headers, &resp_body)
}

/// Read a ureq response into (status, headers, body), dropping hop-by-hop and
/// length headers (we re-emit our own `Content-Length`).
fn read_response(resp: ureq::Response) -> (u16, Vec<(String, String)>, Vec<u8>) {
    let status = resp.status();
    let mut headers = Vec::new();
    for name in resp.headers_names() {
        let nl = name.to_ascii_lowercase();
        if nl == "transfer-encoding" || nl == "connection" || nl == "content-length" {
            continue;
        }
        if let Some(v) = resp.header(&name) {
            headers.push((name, v.to_string()));
        }
    }
    let mut body = Vec::new();
    let _ = resp.into_reader().read_to_end(&mut body);
    (status, headers, body)
}

fn write_response(
    w: &mut TcpStream,
    status: u16,
    headers: &[(String, String)],
    body: &[u8],
) -> Result<()> {
    write!(w, "HTTP/1.1 {status} \r\n")?;
    for (k, v) in headers {
        write!(w, "{k}: {v}\r\n")?;
    }
    write!(w, "Content-Length: {}\r\n", body.len())?;
    write!(w, "Connection: close\r\n\r\n")?;
    w.write_all(body)?;
    w.flush()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Pretty logging
// ---------------------------------------------------------------------------

fn log_request(path: &str, body: &[u8], brief: bool) {
    println!("\n→ POST {path}");
    let obj: Value = match serde_json::from_slice(body) {
        Ok(v) => v,
        Err(_) => {
            if !body.is_empty() {
                println!("  {}", truncate(&String::from_utf8_lossy(body), 2000));
            }
            return;
        }
    };
    let Some(msgs) = obj.get("messages").and_then(Value::as_array) else {
        println!("  {}", truncate(&obj.to_string(), 800));
        return;
    };
    let model = obj.get("model").and_then(Value::as_str).unwrap_or("?");
    let temp = num(obj.get("temperature"));
    let maxt = num(obj
        .get("max_completion_tokens")
        .or_else(|| obj.get("max_tokens")));
    let ntools = obj.get("tools").and_then(Value::as_array).map_or(0, Vec::len);
    println!("  model={model} temp={temp} max_tokens={maxt} tools={ntools} messages={}", msgs.len());
    if brief {
        return;
    }
    for m in msgs {
        let role = m.get("role").and_then(Value::as_str).unwrap_or("?");
        if let Some(tcs) = m.get("tool_calls").and_then(Value::as_array) {
            let calls: Vec<String> = tcs
                .iter()
                .map(|c| format!("{}({})", call_name(c), truncate(&call_args(c), 300)))
                .collect();
            if !calls.is_empty() {
                println!("  [{role}] tool_calls: {}", calls.join(", "));
            }
        }
        if let Some(content) = m.get("content").and_then(Value::as_str) {
            if !content.is_empty() {
                println!("  [{role}] {}", truncate(content, 4000));
            }
        }
    }
}

fn log_response(path: &str, body: &[u8], ms: u128, brief: bool) {
    println!("← {path}  ({ms}ms)");
    let obj: Value = match serde_json::from_slice(body) {
        Ok(v) => v,
        Err(_) => {
            if !body.is_empty() {
                println!("  {}", truncate(&String::from_utf8_lossy(body), 1000));
            }
            return;
        }
    };
    if let Some(msg) = obj
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|c| c.first())
        .and_then(|c| c.get("message"))
    {
        if let Some(content) = msg.get("content").and_then(Value::as_str) {
            if !content.is_empty() {
                println!("  [assistant] {}", truncate(content, 4000));
            }
        }
        for c in msg.get("tool_calls").and_then(Value::as_array).unwrap_or(&Vec::new()) {
            println!("  [assistant] call: {}({})", call_name(c), truncate(&call_args(c), 500));
        }
    } else if !brief {
        println!("  {}", truncate(&obj.to_string(), 800));
    }
    if let Some(u) = obj.get("usage") {
        println!(
            "  usage: prompt={} completion={} total={}",
            num(u.get("prompt_tokens")),
            num(u.get("completion_tokens")),
            num(u.get("total_tokens")),
        );
    }
}

fn call_name(c: &Value) -> String {
    c.get("function")
        .and_then(|f| f.get("name"))
        .and_then(Value::as_str)
        .unwrap_or("?")
        .to_string()
}

/// Tool-call arguments — a JSON-encoded string (OpenAI/Ollama) or an object
/// (llama.cpp); render either compactly.
fn call_args(c: &Value) -> String {
    match c.get("function").and_then(|f| f.get("arguments")) {
        Some(Value::String(s)) => s.clone(),
        Some(other) => other.to_string(),
        None => String::new(),
    }
}

fn num(v: Option<&Value>) -> String {
    v.map_or_else(|| "-".to_string(), ToString::to_string)
}

/// Char-safe truncation (bodies contain multibyte text; never slice mid-char).
fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut t: String = s.chars().take(max).collect();
    t.push('…');
    t
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn strip_v1_normalizes_base_urls() {
        assert_eq!(strip_v1("http://localhost:11434/v1"), "http://localhost:11434");
        assert_eq!(strip_v1("http://localhost:11434/v1/"), "http://localhost:11434");
        assert_eq!(strip_v1("http://localhost:11434"), "http://localhost:11434");
        assert_eq!(strip_v1("http://host/v1//"), "http://host");
    }

    #[test]
    fn truncate_is_char_safe() {
        let s = "é".repeat(100);
        let out = truncate(&s, 10);
        assert!(out.chars().count() <= 11); // 10 + ellipsis
        assert!(out.ends_with('…'));
        assert_eq!(truncate("short", 10), "short");
    }

    #[test]
    fn call_args_handles_string_and_object() {
        let s = json!({"function": {"arguments": "{\"a\":1}"}});
        assert_eq!(call_args(&s), "{\"a\":1}");
        let o = json!({"function": {"arguments": {"a": 1}}});
        assert_eq!(call_args(&o), "{\"a\":1}");
        assert_eq!(call_args(&json!({})), "");
    }
}
