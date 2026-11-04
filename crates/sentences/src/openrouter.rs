//! Sentence generation and vocabulary translation through OpenRouter
//! (https://openrouter.ai), which serves many models behind one
//! OpenAI-compatible chat API. Same prompts and schemas as the Claude client.

use serde_json::{json, Value};

use crate::prompt::{self, GenerateRequest};
use crate::{translate_prompt, vocab_prompt};
use crate::{Error, Example, Result};

/// OpenRouter picks a capable model for each request.
pub const DEFAULT_MODEL: &str = "openrouter/auto";
const API_URL: &str = "https://openrouter.ai/api/v1/chat/completions";
const KEY_URL: &str = "https://openrouter.ai/api/v1/key";
/// Optional attribution headers OpenRouter shows on its dashboards.
const REFERER: &str = "https://github.com/Hellsccythe/japones-app";
const TITLE: &str = "Japones App";

pub struct OpenRouterClient {
    http: reqwest::Client,
    api_key: String,
    model: String,
}

impl OpenRouterClient {
    pub fn new(api_key: impl Into<String>, model: Option<String>) -> Self {
        Self {
            http: reqwest::Client::new(),
            api_key: api_key.into(),
            model: model.filter(|m| !m.trim().is_empty()).unwrap_or_else(|| DEFAULT_MODEL.into()),
        }
    }

    pub async fn generate(&self, req: &GenerateRequest) -> Result<Vec<Example>> {
        let value = self.ask(prompt::SYSTEM, &prompt::user_prompt(req), prompt::schema(None), 8000).await?;
        prompt::parse(strip_fences(response_text(&value)?), answered_by(&value, &self.model))
    }

    /// Short meanings in `language` for `(base, english gloss)` pairs, one per word.
    pub async fn translate_vocabulary(&self, words: &[(String, String)], language: &str) -> Result<Vec<String>> {
        let value = self
            .ask(
                vocab_prompt::SYSTEM,
                &vocab_prompt::user_prompt(words, language),
                vocab_prompt::schema(None),
                4000,
            )
            .await?;
        vocab_prompt::parse(strip_fences(response_text(&value)?), words)
    }

    /// The sentence translated into `language`.
    pub async fn translate_sentence(&self, sentence: &str, language: &str) -> Result<String> {
        let value = self
            .ask(translate_prompt::SYSTEM, &translate_prompt::user_prompt(sentence, language), translate_prompt::schema(), 2000)
            .await?;
        translate_prompt::parse(strip_fences(response_text(&value)?))
    }

    /// Checks the key with the key-info endpoint: generates nothing, costs nothing.
    pub async fn check(&self) -> Result<()> {
        let resp = self.http.get(KEY_URL).bearer_auth(&self.api_key).send().await?;
        check_status(resp.status().as_u16())
    }

    async fn ask(&self, system: &str, user: &str, schema: Value, max_tokens: u32) -> Result<Value> {
        let resp = self
            .http
            .post(API_URL)
            .bearer_auth(&self.api_key)
            .header("HTTP-Referer", REFERER)
            .header("X-Title", TITLE)
            .json(&request_body(&self.model, system, user, schema, max_tokens))
            .send()
            .await?;
        let status = resp.status();
        let value: Value = resp.json().await?;
        if !status.is_success() {
            let msg = value["error"]["message"].as_str().unwrap_or("erro desconhecido");
            return Err(Error::Api(format!("OpenRouter {status}: {msg}")));
        }
        Ok(value)
    }
}

/// Chat Completions body with a strict JSON Schema answer.
fn request_body(model: &str, system: &str, user: &str, schema: Value, max_tokens: u32) -> Value {
    json!({
        "model": model,
        "max_tokens": max_tokens,
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": user}
        ],
        "response_format": {
            "type": "json_schema",
            "json_schema": {"name": "answer", "strict": true, "schema": schema}
        },
        // Only route to providers that honor response_format.
        "provider": {"require_parameters": true}
    })
}

/// The answer text, or why there is none.
fn response_text(value: &Value) -> Result<&str> {
    if let Some(msg) = value["error"]["message"].as_str() {
        return Err(Error::Api(format!("OpenRouter: {msg}")));
    }
    let choice = &value["choices"][0];
    if choice["finish_reason"] == "length" {
        return Err(Error::Api("a resposta veio cortada (max_tokens)".into()));
    }
    choice["message"]["content"]
        .as_str()
        .filter(|t| !t.trim().is_empty())
        .ok_or_else(|| Error::Api("OpenRouter: resposta sem texto".into()))
}

/// The model that answered (with `openrouter/auto` it's the one picked).
fn answered_by<'a>(value: &'a Value, requested: &'a str) -> &'a str {
    value["model"].as_str().unwrap_or(requested)
}

/// Some models wrap JSON in a Markdown code block even when asked not to.
fn strip_fences(text: &str) -> &str {
    let t = text.trim();
    let Some(rest) = t.strip_prefix("```") else { return t };
    let rest = rest.split_once('\n').map_or("", |(_, body)| body);
    rest.trim_end().strip_suffix("```").unwrap_or(rest).trim()
}

/// Result of the key check by HTTP status.
fn check_status(status: u16) -> Result<()> {
    match status {
        200..=299 => Ok(()),
        401 | 403 => Err(Error::Api("chave inválida".into())),
        s => Err(Error::Api(format!("o OpenRouter respondeu {s}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn body_asks_for_strict_json_from_providers_that_support_it() {
        let schema = json!({"type": "object"});
        let b = request_body("openrouter/auto", "sys", "user", schema.clone(), 900);
        assert_eq!(b["model"], "openrouter/auto");
        assert_eq!(b["max_tokens"], 900);
        assert_eq!(b["messages"][0], json!({"role": "system", "content": "sys"}));
        assert_eq!(b["messages"][1], json!({"role": "user", "content": "user"}));
        assert_eq!(b["response_format"]["type"], "json_schema");
        assert_eq!(b["response_format"]["json_schema"]["strict"], true);
        assert_eq!(b["response_format"]["json_schema"]["schema"], schema);
        // Without this, OpenRouter may route to a provider that ignores the schema.
        assert_eq!(b["provider"]["require_parameters"], true);
    }

    #[test]
    fn reads_the_message_content_and_the_model_that_answered() {
        let v = json!({"model": "google/gemma-x", "choices": [{"finish_reason": "stop", "message": {"content": "{\"a\":1}"}}]});
        assert_eq!(response_text(&v).unwrap(), "{\"a\":1}");
        assert_eq!(answered_by(&v, "openrouter/auto"), "google/gemma-x");
        assert_eq!(answered_by(&json!({}), "openrouter/auto"), "openrouter/auto");
    }

    #[test]
    fn cut_off_or_empty_answers_are_errors() {
        let cut = json!({"choices": [{"finish_reason": "length", "message": {"content": "{\"a\""}}]});
        assert!(matches!(response_text(&cut), Err(Error::Api(m)) if m.contains("cortada")));
        assert!(response_text(&json!({"choices": []})).is_err());
        // OpenRouter can answer 200 with an error object (e.g. the upstream provider failed).
        let err = json!({"error": {"message": "Provider returned error"}});
        assert!(matches!(response_text(&err), Err(Error::Api(m)) if m.contains("Provider returned error")));
    }

    #[test]
    fn strips_markdown_fences_around_json() {
        assert_eq!(strip_fences("```json\n{\"a\":1}\n```"), "{\"a\":1}");
        assert_eq!(strip_fences("```\n{\"a\":1}\n```  "), "{\"a\":1}");
        assert_eq!(strip_fences("  {\"a\":1} "), "{\"a\":1}");
    }

    #[test]
    fn key_check_status() {
        assert!(check_status(200).is_ok());
        assert!(matches!(check_status(401), Err(Error::Api(m)) if m.contains("chave inválida")));
        assert!(matches!(check_status(429), Err(Error::Api(m)) if m.contains("429")));
    }

    #[test]
    fn empty_model_falls_back_to_auto() {
        assert_eq!(OpenRouterClient::new("k", Some(String::new())).model, DEFAULT_MODEL);
        assert_eq!(OpenRouterClient::new("k", None).model, "openrouter/auto");
        assert_eq!(OpenRouterClient::new("k", Some("x/y".into())).model, "x/y");
    }
}
