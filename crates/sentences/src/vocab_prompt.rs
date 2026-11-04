//! Prompt, schema and parsing for translating a sentence's vocabulary
//! (short meanings), shared by Claude and the local model.

use serde_json::{json, Value};

use crate::Result;

pub const SYSTEM: &str = "You are a Japanese teacher writing short glossary entries for a learner.";

/// `words` are `(dictionary form, English gloss or "")`.
pub fn user_prompt(words: &[(String, String)], language: &str) -> String {
    let list: Vec<String> = words
        .iter()
        .enumerate()
        .map(|(i, (base, gloss))| {
            if gloss.is_empty() {
                format!("{}. {base}", i + 1)
            } else {
                format!("{}. {base} — {gloss}", i + 1)
            }
        })
        .collect();
    format!(
        "Translate the meaning of each Japanese word below into {language}. \
         Each word is in dictionary form, sometimes followed by its English meaning. \
         Give one short meaning per word (one to four words, no explanations, no brackets), \
         in the same order, written in {language}. For particles and auxiliaries, give their \
         grammatical role in a few words.\n\n{}",
        list.join("\n")
    )
}

/// `{ "translations": [string; count] }`, with the length pinned.
pub fn schema(count: usize) -> Value {
    json!({
        "type": "object",
        "properties": {
            "translations": {"type": "array", "items": {"type": "string"}, "minItems": count, "maxItems": count}
        },
        "required": ["translations"],
        "additionalProperties": false
    })
}

#[derive(serde::Deserialize)]
struct Translations {
    translations: Vec<String>,
}

/// Exactly `count` translations: missing ones become "" and extra ones are dropped.
pub fn parse(text: &str, count: usize) -> Result<Vec<String>> {
    let t: Translations = serde_json::from_str(text.trim())?;
    let mut out: Vec<String> = t.translations.into_iter().take(count).map(|s| s.trim().to_string()).collect();
    out.resize(count, String::new());
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_pads_and_truncates() {
        assert_eq!(parse(r#"{"translations":["visitar"]}"#, 2).unwrap(), vec!["visitar".to_string(), String::new()]);
        assert_eq!(parse(r#"{"translations":["a","b","c"]}"#, 2).unwrap().len(), 2);
    }

    #[test]
    fn parse_trims_and_rejects_garbage() {
        assert_eq!(parse(r#" {"translations":[" ir "]} "#, 1).unwrap(), vec!["ir".to_string()]);
        assert!(parse("not json", 1).is_err());
    }

    #[test]
    fn schema_fixes_count() {
        let s = schema(3);
        assert_eq!(s["properties"]["translations"]["minItems"], 3);
        assert_eq!(s["properties"]["translations"]["maxItems"], 3);
    }

    #[test]
    fn prompt_lists_words_and_language() {
        let p = user_prompt(&[("見学".into(), "field trip".into())], "português");
        assert!(p.contains("見学") && p.contains("field trip") && p.contains("português"));
    }

    #[test]
    fn prompt_handles_words_without_gloss() {
        let p = user_prompt(&[("ねえ".into(), String::new())], "português");
        assert!(p.contains("ねえ"));
    }
}
