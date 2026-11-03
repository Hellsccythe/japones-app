//! Example sentences for a word: real sentences from the Tatoeba corpus
//! (indexed by lemma, so conjugated forms match) and LLM-generated ones.

pub mod llm;
pub mod prompt;
pub mod store;
pub mod tatoeba;

use jp_core::Analyzer;
use serde::{Deserialize, Serialize};

pub use llm::LlmClient;
pub use prompt::GenerateRequest;
pub use store::Store;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("database: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("analysis: {0}")]
    Analysis(#[from] jp_core::Error),
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("Claude API: {0}")]
    Api(String),
    #[error("invalid model output: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Source {
    Tatoeba { id: i64 },
    Llm { model: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Example {
    pub japanese: String,
    pub translation: Option<String>,
    /// Short grammar/usage note (LLM only).
    pub note: Option<String>,
    pub source: Source,
}

/// Lemmas that identify `word`, used both for indexing lookups and for
/// checking that an LLM sentence really contains the word.
pub fn word_lemmas(analyzer: &Analyzer, word: &str) -> Result<Vec<String>> {
    let lemmas = analyzer.lemmas(word)?;
    Ok(if lemmas.is_empty() { vec![word.to_string()] } else { lemmas })
}

/// True if `sentence` contains `word_lemmas` as a contiguous run of lemmas
/// (so 勉強する matches 勉強しました but not a sentence with 勉強 and する apart).
pub fn contains_word(analyzer: &Analyzer, sentence: &str, word_lemmas: &[String]) -> Result<bool> {
    let lemmas = analyzer.lemmas(sentence)?;
    Ok(lemmas.windows(word_lemmas.len()).any(|w| w == word_lemmas))
}
