//! The OpenAI chat-completions **wire model** for the inner explorer.
//!
//! These serde types model the request/response subset the explorer needs, plus
//! the one piece of provider-specific knowledge — how tool-call `arguments` are
//! encoded — normalized here (see [`normalize_arguments`]) so callers always see
//! a uniform [`ToolCall`]. Optional fields are omitted when unset, so the emitted
//! body stays lean and both Ollama and llama.cpp accept it.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

/// A chat message role. Serialized lowercase (`system` / `user` / `assistant`
/// / `tool`), matching the OpenAI schema both providers implement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// System / steering prompt.
    System,
    /// End-user turn.
    User,
    /// Model turn — may carry `tool_calls`.
    Assistant,
    /// A tool result fed back into the conversation; must carry the
    /// `tool_call_id` it answers.
    Tool,
}

/// A single conversation turn.
///
/// `content` is optional because an assistant turn that only requests tools may
/// omit it; a `tool` turn must set both `content` (the result) and
/// `tool_call_id` (which call it answers). Empty collections/None fields are
/// omitted on the wire.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    /// The turn's role.
    pub role: Role,
    /// The textual content, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// Tool calls requested by an assistant turn (normalized).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
    /// For a `tool` turn: the id of the call this message answers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// Optional participant name (rarely used; passed through if set).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl Message {
    /// A `system` steering message.
    pub fn system(content: impl Into<String>) -> Self {
        Self::text(Role::System, content)
    }

    /// A `user` message.
    pub fn user(content: impl Into<String>) -> Self {
        Self::text(Role::User, content)
    }

    /// An `assistant` message carrying free text (no tool calls).
    pub fn assistant(content: impl Into<String>) -> Self {
        Self::text(Role::Assistant, content)
    }

    /// A `tool` result message answering a specific `tool_call_id`.
    pub fn tool(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Message {
            role: Role::Tool,
            content: Some(content.into()),
            tool_calls: Vec::new(),
            tool_call_id: Some(tool_call_id.into()),
            name: None,
        }
    }

    fn text(role: Role, content: impl Into<String>) -> Self {
        Message {
            role,
            content: Some(content.into()),
            tool_calls: Vec::new(),
            tool_call_id: None,
            name: None,
        }
    }
}

/// A normalized tool call requested by the model.
///
/// Regardless of how the provider encoded `arguments` on the wire (a JSON
/// object or a JSON-encoded string), `arguments` is always the *parsed* value
/// here — typically a `Value::Object`. On serialization (feeding an assistant
/// turn back to the model) it is re-emitted in the canonical OpenAI shape
/// (`{"id", "type":"function", "function":{"name","arguments":"<json string>"}}`).
#[derive(Debug, Clone, PartialEq)]
pub struct ToolCall {
    /// Provider-assigned call id (correlates with a later `tool` message).
    pub id: String,
    /// The function name the model wants to invoke.
    pub name: String,
    /// The call arguments, normalized to a parsed JSON value.
    pub arguments: Value,
}

impl Serialize for ToolCall {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Canonical OpenAI shape: arguments as a JSON-encoded string.
        let arguments =
            serde_json::to_string(&self.arguments).map_err(serde::ser::Error::custom)?;
        let wire = serde_json::json!({
            "id": self.id,
            "type": "function",
            "function": { "name": self.name, "arguments": arguments },
        });
        wire.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ToolCall {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Raw {
            #[serde(default)]
            id: String,
            #[serde(default)]
            function: RawFunction,
        }
        #[derive(Deserialize, Default)]
        struct RawFunction {
            #[serde(default)]
            name: String,
            #[serde(default)]
            arguments: Value,
        }

        let raw = Raw::deserialize(deserializer)?;
        Ok(ToolCall {
            id: raw.id,
            name: raw.function.name,
            arguments: normalize_arguments(raw.function.arguments),
        })
    }
}

/// Normalize a wire `arguments` value into a parsed JSON value.
///
/// - A JSON-encoded **string** (Ollama / OpenAI spec) is parsed; if it is empty
///   or unparseable it degrades gracefully (empty object / the raw string).
/// - An already-parsed **object** (llama.cpp) passes through untouched.
/// - `null`/absent becomes an empty object, so callers never special-case it.
fn normalize_arguments(v: Value) -> Value {
    match v {
        Value::String(s) => {
            if s.trim().is_empty() {
                Value::Object(serde_json::Map::new())
            } else {
                serde_json::from_str(&s).unwrap_or(Value::String(s))
            }
        }
        Value::Null => Value::Object(serde_json::Map::new()),
        other => other,
    }
}

/// A tool declaration offered to the model (request side).
///
/// `parameters` is a raw JSON-schema `Value`, so any schema passes through
/// untouched.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Tool {
    /// Always `"function"` for the current OpenAI tool protocol.
    #[serde(rename = "type")]
    pub kind: String,
    /// The function declaration.
    pub function: ToolFunction,
}

impl Tool {
    /// Construct a `function`-type tool from its name, description, and a
    /// JSON-schema parameters value.
    pub fn function(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: Value,
    ) -> Self {
        Tool {
            kind: "function".to_string(),
            function: ToolFunction {
                name: name.into(),
                description: description.into(),
                parameters,
            },
        }
    }
}

/// The function half of a [`Tool`] declaration.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ToolFunction {
    /// The function name the model may call.
    pub name: String,
    /// A human/model-readable description of what it does.
    pub description: String,
    /// JSON-schema describing the call arguments.
    pub parameters: Value,
}

/// A non-streaming chat-completions request.
///
/// `model` is set by [`OpenAiCompatClient::chat`](super::client::OpenAiCompatClient::chat)
/// from config, so callers can leave it empty (via [`ChatRequest::new`]);
/// optional fields are omitted from the wire body when unset.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChatRequest {
    /// The model identifier (filled by the client from config).
    pub model: String,
    /// The conversation so far.
    pub messages: Vec<Message>,
    /// Tools the model may call.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<Tool>,
    /// Tool-choice directive (`"auto"`, `"none"`, or a forced function object).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<Value>,
    /// Sampling temperature, if overriding the server default.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    /// Max generated tokens (OpenAI `max_completion_tokens`). The reference
    /// bench pins this at 1024; leaving it unset lets small models ramble.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_completion_tokens: Option<u32>,
    /// Nucleus sampling (bench uses 0.95).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    /// Top-k sampling — sent inside the bench's qwen `extra_body` (20).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<u32>,
    /// qwen chat-template kwargs — the bench sends `{"enable_thinking": false}`
    /// here, which is decisive for qwen3.x tool-calling quality.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chat_template_kwargs: Option<Value>,
    /// Optional `reasoning_effort` passthrough (bench: `"none"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<String>,
}

impl ChatRequest {
    /// A request from a message list, with no tools and server-default
    /// sampling. `model` is left empty for the client to fill.
    pub fn new(messages: Vec<Message>) -> Self {
        ChatRequest {
            model: String::new(),
            messages,
            tools: Vec::new(),
            tool_choice: None,
            temperature: None,
            max_completion_tokens: None,
            top_p: None,
            top_k: None,
            chat_template_kwargs: None,
            reasoning_effort: None,
        }
    }

    /// Builder: attach tool declarations.
    pub fn with_tools(mut self, tools: Vec<Tool>) -> Self {
        self.tools = tools;
        self
    }

    /// Builder: set the tool-choice directive.
    pub fn with_tool_choice(mut self, choice: Value) -> Self {
        self.tool_choice = Some(choice);
        self
    }

    /// Builder: apply the reference bench's sampling parameters. `qwen` gates
    /// the `top_k` + `enable_thinking:false` knobs the bench sends only for
    /// qwen models (`llm.py`: `if "qwen" in self.model`).
    pub fn with_bench_sampling(
        mut self,
        temperature: f32,
        top_p: f32,
        max_completion_tokens: u32,
        reasoning_effort: Option<String>,
        qwen: bool,
    ) -> Self {
        self.temperature = Some(temperature);
        self.top_p = Some(top_p);
        self.max_completion_tokens = Some(max_completion_tokens);
        self.reasoning_effort = reasoning_effort;
        if qwen {
            self.top_k = Some(20);
            self.chat_template_kwargs = Some(serde_json::json!({ "enable_thinking": false }));
        }
        self
    }

    /// Builder: apply the `base-q4-v2-hf` reference-explore sampling
    /// (`run_eval.py`): `temperature` + `max_tokens` + `chat_template_kwargs`
    /// carrying `enable_thinking` — and nothing else. Unlike
    /// [`Self::with_bench_sampling`], this sends **no** `top_p` / `top_k` /
    /// `reasoning_effort` (the winning harness passed only these three knobs), and
    /// `think` is the interim winner's setting (**on**), not forced off.
    pub fn with_explore_sampling(mut self, temperature: f32, max_tokens: u32, think: bool) -> Self {
        self.temperature = Some(temperature);
        self.max_completion_tokens = Some(max_tokens);
        self.top_p = None;
        self.top_k = None;
        self.reasoning_effort = None;
        self.chat_template_kwargs = Some(serde_json::json!({ "enable_thinking": think }));
        self
    }
}

/// A non-streaming chat-completions response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatResponse {
    /// The completion choices; we use the first.
    #[serde(default)]
    pub choices: Vec<Choice>,
    /// Token accounting for this completion, when the provider reports it. Both
    /// Ollama and llama.cpp emit an OpenAI-style `usage` object; it is the source
    /// of the per-turn / per-call token metrics the trace subsystem records.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
}

impl ChatResponse {
    /// The first choice's message, if any.
    pub fn first_message(&self) -> Option<&Message> {
        self.choices.first().map(|c| &c.message)
    }
}

/// OpenAI-style token accounting for a completion. All fields default to `0`
/// when a provider omits them, so a partial `usage` object never fails to parse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Usage {
    /// Tokens consumed by the prompt (input).
    #[serde(default)]
    pub prompt_tokens: u32,
    /// Tokens generated in the completion (output).
    #[serde(default)]
    pub completion_tokens: u32,
    /// Prompt + completion. Providers usually report this; when absent it can be
    /// recomputed as `prompt_tokens + completion_tokens`.
    #[serde(default)]
    pub total_tokens: u32,
}

/// A single completion choice.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Choice {
    /// The assistant message for this choice.
    pub message: Message,
    /// Why generation stopped (`stop`, `tool_calls`, …), if reported.
    #[serde(default)]
    pub finish_reason: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_request_serializes_lean() {
        let req = ChatRequest::new(vec![
            Message::system("be helpful"),
            Message::user("hello"),
        ]);
        let v: Value = serde_json::to_value(&req).unwrap();
        // Present.
        assert_eq!(v["messages"][0]["role"], "system");
        assert_eq!(v["messages"][1]["role"], "user");
        assert_eq!(v["messages"][1]["content"], "hello");
        // Omitted when unset (lean body both providers accept).
        assert!(v.get("tools").is_none(), "empty tools omitted");
        assert!(v.get("tool_choice").is_none(), "unset tool_choice omitted");
        assert!(v.get("temperature").is_none(), "unset temperature omitted");
        // A user message carries no tool_call_id / tool_calls on the wire.
        assert!(v["messages"][1].get("tool_call_id").is_none());
        assert!(v["messages"][1].get("tool_calls").is_none());
    }

    #[test]
    fn request_with_tools_and_choice_serializes() {
        let tool = Tool::function(
            "search",
            "search the code",
            serde_json::json!({"type": "object", "properties": {"q": {"type": "string"}}}),
        );
        let req = ChatRequest::new(vec![Message::user("find foo")])
            .with_tools(vec![tool])
            .with_tool_choice(serde_json::json!("auto"));
        let v: Value = serde_json::to_value(&req).unwrap();
        assert_eq!(v["tools"][0]["type"], "function");
        assert_eq!(v["tools"][0]["function"]["name"], "search");
        assert_eq!(v["tools"][0]["function"]["parameters"]["type"], "object");
        assert_eq!(v["tool_choice"], "auto");
    }

    #[test]
    fn tool_message_carries_call_id() {
        let m = Message::tool("call_42", "{\"result\": 1}");
        let v: Value = serde_json::to_value(&m).unwrap();
        assert_eq!(v["role"], "tool");
        assert_eq!(v["tool_call_id"], "call_42");
        assert_eq!(v["content"], "{\"result\": 1}");
    }

    // The two providers encode tool-call `arguments` differently: llama.cpp
    // sends an already-parsed object, Ollama (and the OpenAI spec) sends a
    // JSON-encoded string. Both must normalize to the identical ToolCall.
    const LLAMACPP_RESPONSE: &str = r#"{
        "choices": [{
            "finish_reason": "tool_calls",
            "message": {
                "role": "assistant",
                "content": null,
                "tool_calls": [{
                    "id": "call_1",
                    "type": "function",
                    "function": { "name": "search", "arguments": {"q": "foo", "n": 3} }
                }]
            }
        }]
    }"#;

    const OLLAMA_RESPONSE: &str = r#"{
        "choices": [{
            "finish_reason": "tool_calls",
            "message": {
                "role": "assistant",
                "tool_calls": [{
                    "id": "call_1",
                    "type": "function",
                    "function": { "name": "search", "arguments": "{\"q\": \"foo\", \"n\": 3}" }
                }]
            }
        }]
    }"#;

    #[test]
    fn provider_dialects_normalize_to_identical_tool_calls() {
        let a: ChatResponse = serde_json::from_str(LLAMACPP_RESPONSE).unwrap();
        let b: ChatResponse = serde_json::from_str(OLLAMA_RESPONSE).unwrap();

        let ta = &a.first_message().unwrap().tool_calls[0];
        let tb = &b.first_message().unwrap().tool_calls[0];

        assert_eq!(ta, tb, "object-args and string-args normalize identically");
        assert_eq!(ta.id, "call_1");
        assert_eq!(ta.name, "search");
        assert_eq!(ta.arguments["q"], "foo");
        assert_eq!(ta.arguments["n"], 3);
    }

    #[test]
    fn empty_and_null_arguments_normalize_to_object() {
        assert_eq!(normalize_arguments(Value::Null), serde_json::json!({}));
        assert_eq!(normalize_arguments(Value::String(String::new())), serde_json::json!({}));
        assert_eq!(normalize_arguments(Value::String("   ".into())), serde_json::json!({}));
    }

    #[test]
    fn unparseable_string_arguments_degrade_gracefully() {
        // A non-JSON arguments string is preserved rather than lost/panicking.
        let got = normalize_arguments(Value::String("not json".into()));
        assert_eq!(got, Value::String("not json".into()));
    }

    #[test]
    fn tool_call_round_trips_through_canonical_shape() {
        let tc = ToolCall {
            id: "call_9".to_string(),
            name: "search".to_string(),
            arguments: serde_json::json!({"q": "bar"}),
        };
        let v: Value = serde_json::to_value(&tc).unwrap();
        // Serialized in canonical OpenAI shape: arguments is a JSON *string*.
        assert_eq!(v["type"], "function");
        assert_eq!(v["function"]["name"], "search");
        assert!(v["function"]["arguments"].is_string());
        // …and deserializing it back yields the same normalized ToolCall.
        let back: ToolCall = serde_json::from_value(v).unwrap();
        assert_eq!(back, tc);
    }

    #[test]
    fn usage_is_parsed_when_present_and_absent() {
        // Present: the token accounting is captured for the trace metrics.
        let with = r#"{"choices":[{"message":{"role":"assistant","content":"hi"}}],
            "usage":{"prompt_tokens":120,"completion_tokens":8,"total_tokens":128}}"#;
        let resp: ChatResponse = serde_json::from_str(with).unwrap();
        let u = resp.usage.expect("usage present");
        assert_eq!(u.prompt_tokens, 120);
        assert_eq!(u.completion_tokens, 8);
        assert_eq!(u.total_tokens, 128);

        // Absent: older/leaner responses still parse, usage is None.
        let without = r#"{"choices":[{"message":{"role":"assistant","content":"hi"}}]}"#;
        let resp: ChatResponse = serde_json::from_str(without).unwrap();
        assert!(resp.usage.is_none());

        // Partial: a usage object missing a field defaults it to 0, never fails.
        let partial = r#"{"choices":[],"usage":{"prompt_tokens":5}}"#;
        let resp: ChatResponse = serde_json::from_str(partial).unwrap();
        let u = resp.usage.unwrap();
        assert_eq!(u.prompt_tokens, 5);
        assert_eq!(u.completion_tokens, 0);
    }

    #[test]
    fn response_without_tool_calls_is_plain_text() {
        let json = r#"{"choices":[{"message":{"role":"assistant","content":"hi there"}}]}"#;
        let resp: ChatResponse = serde_json::from_str(json).unwrap();
        let m = resp.first_message().unwrap();
        assert_eq!(m.content.as_deref(), Some("hi there"));
        assert!(m.tool_calls.is_empty());
    }
}
