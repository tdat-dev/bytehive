//! The ingestion gate's classifier. One POST to an OpenAI-compatible endpoint.
//! Settings only takes the key; endpoint and model are the defaults below, or
//! whatever is in ~/.bytehive/ingest-provider.json. The key stays here and is
//! never sent to the page. No agent CLI is called, so no plan quota is touched.

use crate::error::CommandError;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::Duration;

const DIR: &str = ".bytehive";
const FILE: &str = "ingest-provider.json";
const SCHEMA_NAME: &str = "ingest";

const SYSTEM: &str = "You classify one message for a coding-agent orchestrator. Reply with one JSON object and nothing else.
Shape: {\"is_actionable\": <boolean>, \"data\": <object or null>}.
Rules:
- is_actionable is false only when the message is clearly small talk: a greeting, thanks, acknowledgement or emoji, with no task and no question.
- is_actionable is true for anything else, including coding tasks and questions. When unsure, true.
- If the message asks to estimate the cost of tokens for a model, set data to {\"intent\":\"estimate_cost\",\"model\":\"<model id>\",\"inputTokens\":<number>,\"outputTokens\":<number>}.
- Token amounts written as 120k or \"250 nghìn\" mean 120000 and 250000.
- If the message asks to convert money, set data to {\"intent\":\"convert_currency\",\"amount\":<number>,\"from\":\"<code>\",\"to\":\"<code>\"}.
- Otherwise data is null.
Never explain. Never use markdown.";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConfig {
    pub base_url: String,
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(default = "default_mode")]
    pub json_mode: String,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}

fn default_mode() -> String {
    "schema".into()
}

fn default_timeout() -> u64 {
    12_000
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            base_url: "https://9router.zoldify.com/v1".into(),
            model: "cx/gpt-6-luna".into(),
            api_key: None,
            json_mode: default_mode(),
            timeout_ms: default_timeout(),
        }
    }
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderView {
    pub has_key: bool,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct Usage {
    #[serde(rename = "in")]
    pub input: u64,
    pub out: u64,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct ClassifyReply {
    pub content: String,
    pub usage: Usage,
    pub model: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SaveInput {
    #[serde(default)]
    pub api_key: Option<String>,
}

#[tauri::command]
pub fn ingest_provider_get() -> Result<ProviderView, CommandError> {
    Ok(view(&read_config().unwrap_or_default()))
}

#[tauri::command]
pub fn ingest_provider_save(input: SaveInput) -> Result<ProviderView, CommandError> {
    let old = read_config().unwrap_or_default();
    let next = merge(&old, input);
    write_config(&next)?;
    Ok(view(&next))
}

#[tauri::command]
pub async fn ingest_classify(text: String) -> Result<ClassifyReply, CommandError> {
    let cfg = read_config()?;
    if cfg.base_url.trim().is_empty() || cfg.model.trim().is_empty() {
        return Err(CommandError::Failed("provider_unconfigured".into()));
    }
    let _ = rustls::crypto::ring::default_provider().install_default();
    let (reply, learned) = classify(cfg, &text).await?;
    if let Some(next) = learned {
        let _ = write_config(&next);
    }
    Ok(reply)
}

/// The call itself. The second value is a config worth keeping: it comes back
/// only once the plain form worked, so one bad request can't change the mode.
async fn classify(mut cfg: ProviderConfig, text: &str) -> Result<(ClassifyReply, Option<ProviderConfig>), CommandError> {
    let mut res = post(&cfg, text).await?;
    let mut learned = false;
    if refuses_schema(&cfg.json_mode, res.status().as_u16()) {
        cfg.json_mode = "prompt".into();
        res = post(&cfg, text).await?;
        learned = true;
    }
    let status = res.status();
    if !status.is_success() {
        return Err(CommandError::Failed(format!("http_{}", status.as_u16())));
    }
    let value: Value = res.json().await.map_err(|_| CommandError::Failed("bad_response".into()))?;
    let reply = reply_from(&value, &cfg.model)?;
    Ok((reply, learned.then_some(cfg)))
}

async fn post(cfg: &ProviderConfig, text: &str) -> Result<reqwest::Response, CommandError> {
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    let mut request = CLIENT
        .get_or_init(reqwest::Client::new)
        .post(endpoint(&cfg.base_url))
        .timeout(Duration::from_millis(cfg.timeout_ms.max(1000)))
        .json(&build_body(cfg, text));
    if let Some(key) = cfg.api_key.as_deref().filter(|k| !k.trim().is_empty()) {
        request = request.bearer_auth(key);
    }
    request.send().await.map_err(|e| {
        if e.is_timeout() {
            CommandError::Failed("timeout".into())
        } else {
            CommandError::Failed("network".into())
        }
    })
}

/// A provider without schema support rejects the request outright. It answers
/// the same question in plain JSON, so nobody has to pick a format.
fn refuses_schema(json_mode: &str, status: u16) -> bool {
    json_mode == "schema" && matches!(status, 400 | 422)
}

fn view(cfg: &ProviderConfig) -> ProviderView {
    ProviderView { has_key: cfg.api_key.as_deref().is_some_and(|k| !k.trim().is_empty()) }
}

/// Put a new key into the stored config and leave the rest alone. An empty key
/// means keep the old one, since the page never receives the key back.
fn merge(old: &ProviderConfig, input: SaveInput) -> ProviderConfig {
    let api_key = match input.api_key {
        Some(k) if !k.trim().is_empty() => Some(k.trim().to_string()),
        _ => old.api_key.clone(),
    };
    ProviderConfig { api_key, ..old.clone() }
}

fn endpoint(base_url: &str) -> String {
    format!("{}/chat/completions", base_url.trim_end_matches('/'))
}

fn schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "is_actionable": { "type": "boolean" },
            "data": { "type": ["object", "null"] }
        },
        "required": ["is_actionable", "data"],
        "additionalProperties": false
    })
}

fn build_body(cfg: &ProviderConfig, text: &str) -> Value {
    let mut body = json!({
        "model": &cfg.model,
        "temperature": 0,
        "max_tokens": 300,
        "messages": [
            { "role": "system", "content": SYSTEM },
            { "role": "user", "content": text }
        ]
    });
    if cfg.json_mode == "schema" {
        body["response_format"] = json!({
            "type": "json_schema",
            "json_schema": { "name": SCHEMA_NAME, "strict": true, "schema": schema() }
        });
    }
    body
}

fn reply_from(value: &Value, fallback_model: &str) -> Result<ClassifyReply, CommandError> {
    let content = value
        .pointer("/choices/0/message/content")
        .and_then(Value::as_str)
        .ok_or_else(|| CommandError::Failed("bad_response".into()))?;
    let usage = value.get("usage");
    Ok(ClassifyReply {
        content: content.to_string(),
        usage: Usage {
            input: usage.and_then(|u| u.get("prompt_tokens")).and_then(Value::as_u64).unwrap_or(0),
            out: usage.and_then(|u| u.get("completion_tokens")).and_then(Value::as_u64).unwrap_or(0),
        },
        model: value
            .get("model")
            .and_then(Value::as_str)
            .unwrap_or(fallback_model)
            .to_string(),
    })
}

fn config_path() -> Result<PathBuf, CommandError> {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map_err(|_| CommandError::Failed("no home folder".into()))?;
    Ok(Path::new(&home).join(DIR).join(FILE))
}

fn read_config() -> Result<ProviderConfig, CommandError> {
    let text = std::fs::read_to_string(config_path()?)
        .map_err(|_| CommandError::Failed("provider_unconfigured".into()))?;
    serde_json::from_str(&text).map_err(|_| CommandError::Failed("provider_unconfigured".into()))
}

fn write_config(cfg: &ProviderConfig) -> Result<(), CommandError> {
    let path = config_path()?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| CommandError::Failed(format!("cannot create config folder: {e}")))?;
    }
    let text = serde_json::to_string_pretty(cfg).map_err(|e| CommandError::Failed(e.to_string()))?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text).map_err(|e| CommandError::Failed(format!("cannot write config: {e}")))?;
    std::fs::rename(&tmp, &path).map_err(|e| CommandError::Failed(format!("cannot replace config: {e}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> ProviderConfig {
        ProviderConfig {
            base_url: "https://9router.zoldify.com/v1".into(),
            model: "cx/gpt-6-luna".into(),
            api_key: Some("sk-test".into()),
            json_mode: "schema".into(),
            timeout_ms: 12_000,
        }
    }

    #[test]
    fn joins_the_endpoint_with_or_without_a_trailing_slash() {
        assert_eq!(endpoint("https://x/v1"), "https://x/v1/chat/completions");
        assert_eq!(endpoint("https://x/v1/"), "https://x/v1/chat/completions");
    }

    #[test]
    fn schema_mode_sends_a_strict_schema_and_temperature_zero() {
        let body = build_body(&cfg(), "cảm ơn nhé");
        assert_eq!(body["temperature"], 0);
        assert_eq!(body["model"], "cx/gpt-6-luna");
        assert_eq!(body["response_format"]["json_schema"]["strict"], true);
        assert_eq!(body["response_format"]["json_schema"]["schema"]["required"][0], "is_actionable");
        assert_eq!(body["messages"][1]["content"], "cảm ơn nhé");
    }

    #[test]
    fn prompt_mode_omits_the_response_format() {
        let mut c = cfg();
        c.json_mode = "prompt".into();
        assert!(build_body(&c, "x").get("response_format").is_none());
    }

    #[test]
    fn reads_content_and_usage_from_a_reply() {
        let value = json!({
            "model": "cx/gpt-6-luna",
            "choices": [{ "message": { "content": "{\"is_actionable\":false,\"data\":null}" } }],
            "usage": { "prompt_tokens": 12, "completion_tokens": 5 }
        });
        let r = reply_from(&value, "fallback").unwrap();
        assert_eq!(r.usage.input, 12);
        assert_eq!(r.usage.out, 5);
        assert!(r.content.contains("is_actionable"));
        assert_eq!(r.model, "cx/gpt-6-luna");
    }

    #[test]
    fn a_reply_without_content_is_an_error() {
        assert!(reply_from(&json!({}), "x").is_err());
    }

    #[test]
    fn an_empty_key_keeps_the_old_one() {
        let next = merge(&cfg(), SaveInput { api_key: Some("  ".into()) });
        assert_eq!(next.api_key.as_deref(), Some("sk-test"));
    }

    #[test]
    fn a_new_key_changes_nothing_else() {
        let mut old = cfg();
        old.base_url = "https://mine.example/v1".into();
        old.json_mode = "prompt".into();
        let next = merge(&old, SaveInput { api_key: Some(" sk-new ".into()) });
        assert_eq!(next.api_key.as_deref(), Some("sk-new"));
        assert_eq!(next.base_url, "https://mine.example/v1");
        assert_eq!(next.json_mode, "prompt");
    }

    #[test]
    fn only_a_rejected_schema_request_falls_back() {
        assert!(refuses_schema("schema", 400));
        assert!(refuses_schema("schema", 422));
        assert!(!refuses_schema("prompt", 400));
        assert!(!refuses_schema("schema", 401));
        assert!(!refuses_schema("schema", 500));
    }

    /// A local endpoint that answers every request by looking at its text.
    fn stub(answer: fn(&str) -> u16) -> String {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let mut stream = stream.unwrap();
                let (mut got, mut chunk) = (Vec::new(), [0u8; 4096]);
                loop {
                    let n = stream.read(&mut chunk).unwrap();
                    got.extend_from_slice(&chunk[..n]);
                    let text = String::from_utf8_lossy(&got).to_lowercase();
                    let Some(head) = text.find("\r\n\r\n") else { continue };
                    let length = text
                        .lines()
                        .find_map(|l| l.strip_prefix("content-length:"))
                        .and_then(|v| v.trim().parse::<usize>().ok())
                        .unwrap_or(0);
                    if n == 0 || got.len() >= head + 4 + length {
                        break;
                    }
                }
                let status = answer(&String::from_utf8_lossy(&got));
                let body = r#"{"choices":[{"message":{"content":"{}"}}]}"#;
                let _ = write!(
                    stream,
                    "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
        });
        format!("http://127.0.0.1:{port}/v1")
    }

    fn ask(base_url: String) -> Result<(ClassifyReply, Option<ProviderConfig>), CommandError> {
        let _ = rustls::crypto::ring::default_provider().install_default();
        tauri::async_runtime::block_on(classify(ProviderConfig { base_url, ..cfg() }, "x"))
    }

    #[test]
    fn a_provider_without_schema_support_is_asked_again_and_remembered() {
        let url = stub(|req| if req.contains("response_format") { 400 } else { 200 });
        let (reply, learned) = ask(url).unwrap();
        assert_eq!(reply.content, "{}");
        assert_eq!(learned.unwrap().json_mode, "prompt");
    }

    #[test]
    fn a_provider_that_takes_the_schema_changes_nothing() {
        let (_, learned) = ask(stub(|_| 200)).unwrap();
        assert!(learned.is_none());
    }

    #[test]
    fn a_request_that_fails_both_ways_learns_nothing() {
        let err = ask(stub(|_| 400)).err().unwrap();
        assert_eq!(err.to_string(), "http_400");
    }

    #[test]
    fn a_refused_key_is_not_retried_as_a_format_problem() {
        let err = ask(stub(|req| if req.contains("response_format") { 401 } else { 200 })).err().unwrap();
        assert_eq!(err.to_string(), "http_401");
    }

    #[test]
    fn a_key_never_goes_back_to_the_page() {
        let v = view(&cfg());
        assert!(v.has_key);
        let text = serde_json::to_string(&v).unwrap();
        assert!(!text.contains("sk-test"));
    }
}
