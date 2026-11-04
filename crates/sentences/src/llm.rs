//! Example sentence generation through the Claude Messages API.
//! There's no official Rust SDK, so this calls the HTTP API directly.

use serde_json::json;

use crate::prompt::{self, GenerateRequest};
use crate::vocab_prompt;
use crate::{Error, Example, Result};

pub const DEFAULT_MODEL: &str = "claude-opus-5-5";
const API_URL: &str = "https://api.anthropic.com/v1/messages";
const API_VERSION: &str = "2026-10-21";
const FALLBACK_BETA: &str = "server-side-fallback-2026-07-01";

pub struct LlmClient {
    http: reqwest::Client,
    api_key: String,
    model: String,
}

impl LlmClient {
    pub fn new(api_key: impl Into<String>, model: Option<String>) -> Self {
        Self {
            http: reqwest::Client::new(),
            api_key: api_key.into(),
            model: model.filter(|m| !m.is_empty()).unwrap_or_else(|| DEFAULT_MODEL.into()),
        }
    }

    pub async fn generate(&self, req: &GenerateRequest) -> Result<Vec<Example>> {
        let value = self.ask(prompt::SYSTEM, &prompt::user_prompt(req), prompt::schema(None), 16000).await?;
        parse_response(&value, &self.model)
    }

    /// Short meanings in `language` for `(base, english gloss)` pairs, one per word.
    pub async fn translate_vocabulary(&self, words: &[(String, String)], language: &str) -> Result<Vec<String>> {
        let value = self
            .ask(
                vocab_prompt::SYSTEM,
                &vocab_prompt::user_prompt(words, language),
                vocab_prompt::schema(words.len()),
                4000,
            )
            .await?;
        vocab_prompt::parse(response_text(&value)?, words.len())
    }

    /// One Messages API call with structured output; returns the response JSON.
    async fn ask(&self, system: &str, user: &str, schema: serde_json::Value, max_tokens: u32) -> Result<serde_json::Value> {
        let mut body = json!({
            "model": self.model,
            "max_tokens": max_tokens,
            "system": system,
            "messages": [{"role": "user", "content": user}],
            "output_config": {
                // Short, well-specified tasks: low effort keeps them fast and cheap.
                "effort": "low",
                "format": {"type": "json_schema", "schema": schema}
            }
        });

        let mut request = self
            .http
            .post(API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", API_VERSION);

        // Models with safety classifiers: let the API retry a refused request
        // on a fallback model instead of failing.
        if matches!(self.model.as_str(), "claude-opus-5-5" | "claude-opus-5" | "claude-fable-5-1" | "claude-sonnet-5-5") {
            body["fallbacks"] = json!("default");
            request = request.header("anthropic-beta", FALLBACK_BETA);
        }

        let resp = request.json(&body).send().await?;
        let status = resp.status();
        let value: serde_json::Value = resp.json().await?;
        if !status.is_success() {
            let msg = value["error"]["message"].as_str().unwrap_or("unknown error");
            return Err(Error::Api(format!("{status}: {msg}")));
        }
        Ok(value)
    }
}

/// Extracts the sentences from a successful Messages API response.
fn parse_response(value: &serde_json::Value, requested_model: &str) -> Result<Vec<Example>> {
    let text = response_text(value)?;
    // Report the model that actually answered (may differ after a fallback).
    let model = value["model"].as_str().unwrap_or(requested_model);
    prompt::parse(text, model)
}

/// The JSON text of a successful response, or why there is none.
fn response_text(value: &serde_json::Value) -> Result<&str> {
    match value["stop_reason"].as_str() {
        Some("refusal") => return Err(Error::Api("the model declined this request".into())),
        Some("max_tokens") => return Err(Error::Api("response was cut off (max_tokens)".into())),
        _ => {}
    }

    // Thinking blocks may come first; the JSON is in the text block.
    value["content"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|b| b["type"] == "text")
        .and_then(|b| b["text"].as_str())
        .ok_or_else(|| Error::Api("no text block in response".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Source;

    #[test]
    fn parses_text_block_after_thinking() {
        let inner = json!({"sentences": [{"japanese": "パンを食べる。", "translation": "Como pão.", "note": "forma do dicionário"}]});
        let resp = json!({
            "model": "claude-opus-5-5",
            "stop_reason": "end_turn",
            "content": [
                {"type": "thinking", "thinking": ""},
                {"type": "text", "text": inner.to_string()}
            ]
        });
        let out = parse_response(&resp, "x").unwrap();
        assert_eq!(out[0].japanese, "パンを食べる。");
        assert_eq!(out[0].source, Source::Llm { model: "claude-opus-5-5".into() });
    }

    #[test]
    fn refusal_is_an_error() {
        let resp = json!({"stop_reason": "refusal", "content": []});
        assert!(parse_response(&resp, "x").is_err());
    }
}
