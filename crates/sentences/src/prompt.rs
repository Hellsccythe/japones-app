//! Prompt, output schema and parsing shared by every sentence generator
//! (Claude and the local model), so they produce the same shape.

use serde::Deserialize;
use serde_json::{json, Value};

use crate::{Example, Result, Source};

pub const SYSTEM: &str = "You are a Japanese teacher writing example sentences for a learner's Anki deck.";

/// A sentence shorter than this (in characters) is too terse to be a useful card.
pub const MIN_SENTENCE_CHARS: usize = 6;

#[derive(Debug, Clone, serde::Serialize, Deserialize)]
pub struct GenerateRequest {
    /// Word to practice, in dictionary form (e.g. 食べる).
    pub word: String,
    pub count: usize,
    /// JLPT level hint ("N5".."N1"); empty means no constraint.
    #[serde(default)]
    pub level: String,
    /// Language for translations and notes, e.g. "português".
    pub translation_language: String,
}

pub fn user_prompt(req: &GenerateRequest) -> String {
    let level = if req.level.is_empty() {
        String::new()
    } else {
        format!(" Use vocabulary and grammar appropriate for JLPT {}.", req.level)
    };
    format!(
        "Write {count} natural Japanese example sentences that use the word 「{word}」.{level} \
         Vary the conjugation/form across sentences (plain, polite, past, negative, te-form, etc.). \
         Each one must be a complete sentence, short and useful for flashcards. \
         Write the Japanese in normal kanji/kana orthography, without readings in brackets. \
         For each sentence give a translation into {lang} and a one-line note in {lang} \
         explaining which form of 「{word}」 is used. \
         The translation and the note must be written in {lang}, never in English.",
        count = req.count,
        word = req.word,
        lang = req.translation_language,
    )
}

/// JSON Schema of the answer. `exact_count` pins the array length, which a
/// grammar can enforce (the Claude API's structured outputs don't need it).
pub fn schema(exact_count: Option<usize>) -> Value {
    let mut sentences = json!({
        "type": "array",
        "items": {
            "type": "object",
            "properties": {
                "japanese": {"type": "string"},
                "translation": {"type": "string"},
                "note": {"type": "string"}
            },
            "required": ["japanese", "translation", "note"],
            "additionalProperties": false
        }
    });
    if let Some(n) = exact_count {
        sentences["minItems"] = n.into();
        sentences["maxItems"] = n.into();
    }
    json!({
        "type": "object",
        "properties": {"sentences": sentences},
        "required": ["sentences"],
        "additionalProperties": false
    })
}

#[derive(Deserialize)]
struct Generated {
    sentences: Vec<GeneratedSentence>,
}

#[derive(Deserialize)]
struct GeneratedSentence {
    japanese: String,
    translation: String,
    note: String,
}

/// Parses the model's JSON answer into examples attributed to `model`.
pub fn parse(text: &str, model: &str) -> Result<Vec<Example>> {
    let generated: Generated = serde_json::from_str(text.trim())?;
    Ok(generated
        .sentences
        .into_iter()
        .map(|s| Example {
            japanese: s.japanese.trim().to_string(),
            translation: Some(s.translation),
            note: Some(s.note),
            source: Source::Llm { model: model.to_string() },
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_count_pins_array_length() {
        let s = schema(Some(3));
        assert_eq!(s["properties"]["sentences"]["minItems"], 3);
        assert!(schema(None)["properties"]["sentences"].get("maxItems").is_none());
    }

    #[test]
    fn parse_trims_and_attributes() {
        let out = parse(r#" {"sentences":[{"japanese":" 食べる。","translation":"t","note":"n"}]} "#, "m").unwrap();
        assert_eq!(out[0].japanese, "食べる。");
        assert_eq!(out[0].source, Source::Llm { model: "m".into() });
    }
}
