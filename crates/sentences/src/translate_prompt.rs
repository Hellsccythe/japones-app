//! Prompt, schema and parsing for translating one Japanese sentence (the
//! "Texto" tab has no corpus translation), shared by every AI provider.

use serde_json::{json, Value};

use crate::{Error, Result};

pub const SYSTEM: &str = "You are a Japanese teacher translating sentences for a learner.";

pub fn user_prompt(sentence: &str, language: &str) -> String {
    format!(
        "Translate this Japanese sentence into natural {language}. \
         Keep the meaning and tone; no notes, no romaji, no brackets.\n\n{sentence}"
    )
}

/// `{ "translation": string }`.
pub fn schema() -> Value {
    json!({
        "type": "object",
        "properties": {"translation": {"type": "string"}},
        "required": ["translation"],
        "additionalProperties": false
    })
}

#[derive(serde::Deserialize)]
struct Translation {
    translation: String,
}

pub fn parse(text: &str) -> Result<String> {
    let t: Translation = serde_json::from_str(text.trim())?;
    let t = t.translation.trim();
    if t.is_empty() {
        return Err(Error::Api("a IA não devolveu a tradução".into()));
    }
    Ok(t.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_has_the_sentence_and_language() {
        let p = user_prompt("日本には見学するところがあります。", "português");
        assert!(p.contains("日本には見学するところがあります。") && p.contains("português"));
    }

    #[test]
    fn schema_is_one_required_string() {
        let s = schema();
        assert_eq!(s["properties"]["translation"]["type"], "string");
        assert_eq!(s["required"][0], "translation");
        assert_eq!(s["additionalProperties"], false);
    }

    #[test]
    fn parse_trims_and_rejects_garbage_or_empty() {
        assert_eq!(parse(r#" {"translation":"  No Japão, há lugares para visitar. "} "#).unwrap(), "No Japão, há lugares para visitar.");
        assert!(parse("not json").is_err());
        assert!(parse(r#"{"translation":"  "}"#).is_err());
    }
}
