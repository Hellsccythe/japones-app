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
         written in {language}. Answer with one item per word: `word` is the word exactly as \
         listed and `meaning` its meaning. For particles and auxiliaries, give their \
         grammatical role in a few words.\n\n{}",
        list.join("\n")
    )
}

/// `{ "translations": [{word, meaning}] }`. `count` pins the length, which
/// only the local grammar needs (the remote APIs may reject array limits).
pub fn schema(count: Option<usize>) -> Value {
    let mut list = json!({
        "type": "array",
        "items": {
            "type": "object",
            "properties": {"word": {"type": "string"}, "meaning": {"type": "string"}},
            "required": ["word", "meaning"],
            "additionalProperties": false
        }
    });
    if let Some(n) = count {
        list["minItems"] = json!(n);
        list["maxItems"] = json!(n);
    }
    json!({
        "type": "object",
        "properties": {"translations": list},
        "required": ["translations"],
        "additionalProperties": false
    })
}

#[derive(serde::Deserialize)]
struct Translations {
    translations: Vec<Item>,
}

#[derive(serde::Deserialize)]
struct Item {
    word: String,
    meaning: String,
}

/// One meaning per entry of `words`, matched by dictionary form so a skipped
/// word can't shift the others; words the AI left out get "".
pub fn parse(text: &str, words: &[(String, String)]) -> Result<Vec<String>> {
    let t: Translations = serde_json::from_str(text.trim())?;
    let mut out = vec![String::new(); words.len()];
    for item in t.translations {
        let word = item.word.trim();
        if let Some(i) = words.iter().enumerate().position(|(i, (base, _))| base == word && out[i].is_empty()) {
            out[i] = item.meaning.trim().to_string();
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(bases: &[&str]) -> Vec<(String, String)> {
        bases.iter().map(|b| (b.to_string(), String::new())).collect()
    }

    #[test]
    fn a_skipped_word_does_not_shift_the_others() {
        // Asked for 3, the AI skipped the middle one: meanings stay on their own words.
        let out = parse(
            r#"{"translations":[{"word":"見学","meaning":"visita"},{"word":"行く","meaning":"ir"}]}"#,
            &words(&["見学", "する", "行く"]),
        )
        .unwrap();
        assert_eq!(out, vec!["visita".to_string(), String::new(), "ir".to_string()]);
    }

    #[test]
    fn unknown_and_repeated_answers_are_ignored() {
        let out = parse(
            r#"{"translations":[{"word":"猫","meaning":"gato"},{"word":"見学","meaning":" visita "},{"word":"見学","meaning":"outra"}]}"#,
            &words(&["見学"]),
        )
        .unwrap();
        assert_eq!(out, vec!["visita".to_string()]);
    }

    #[test]
    fn parse_rejects_garbage() {
        assert!(parse("not json", &words(&["見学"])).is_err());
    }

    #[test]
    fn schema_pins_the_count_only_when_asked() {
        // The local grammar can enforce the length; the remote APIs don't need it.
        let s = schema(Some(3));
        assert_eq!(s["properties"]["translations"]["minItems"], 3);
        assert_eq!(s["properties"]["translations"]["maxItems"], 3);
        let s = schema(None);
        assert!(s["properties"]["translations"].get("minItems").is_none());
        assert_eq!(s["properties"]["translations"]["items"]["required"], json!(["word", "meaning"]));
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
