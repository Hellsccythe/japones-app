//! Anki export: straight into a running Anki through AnkiConnect, or to an
//! `.apkg` file. Both use the same note type ("Japones App").

pub mod apkg;
pub mod connect;

use serde::{Deserialize, Serialize};

pub use apkg::{CardKind, Review, Schedule};
pub use connect::AnkiConnect;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("AnkiConnect não respondeu ({0}). O Anki está aberto com o add-on AnkiConnect instalado?")]
    Unreachable(reqwest::Error),
    #[error("AnkiConnect: {0}")]
    Connect(String),
    #[error("apkg: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("apkg: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("apkg: {0}")]
    Json(#[from] serde_json::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

/// One flashcard, ready to export.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Card {
    pub word: String,
    pub sentence: String,
    /// Sentence in Anki furigana syntax (` 食[た]べる`).
    pub sentence_furigana: String,
    #[serde(default)]
    pub translation: String,
    #[serde(default)]
    pub note: String,
    /// WAV bytes, if audio was generated.
    #[serde(default, skip_serializing)]
    pub audio: Option<Vec<u8>>,
    /// Voice credit required by its terms, e.g. "VOICEVOX:ずんだもん".
    #[serde(default)]
    pub audio_credit: Option<String>,
    /// Study progress to carry into Anki (`.apkg` only); `None` = a new card.
    #[serde(skip)]
    pub schedule: Option<Schedule>,
}

impl Card {
    /// Stable media file name per sentence and voice, so re-exporting reuses it
    /// and a different voice doesn't overwrite the old file.
    pub fn audio_filename(&self) -> String {
        let key = format!("{}\0{}", self.sentence, self.audio_credit.as_deref().unwrap_or(""));
        format!("japones_app_{:016x}.wav", fnv1a(key.as_bytes()))
    }

    /// Audio field content: the sound tag (if any) followed by the voice credit.
    fn audio_field(&self, sound: &str) -> String {
        match &self.audio_credit {
            Some(c) if self.audio.is_some() => format!("{sound}<div class=\"credit\">{c}</div>"),
            _ => sound.to_string(),
        }
    }

    fn fields(&self, sound: &str) -> [String; 6] {
        [
            self.word.clone(),
            self.sentence.clone(),
            self.sentence_furigana.clone(),
            self.translation.clone(),
            self.note.clone(),
            self.audio_field(sound),
        ]
    }
}

fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |h, b| (h ^ *b as u64).wrapping_mul(0x100000001b3))
}

pub const MODEL_NAME: &str = "Japones App";
pub(crate) const MODEL_ID: i64 = 1_730_000_000_001;
pub(crate) const FIELDS: [&str; 6] = ["Word", "Sentence", "Furigana", "Translation", "Note", "Audio"];

pub(crate) const FRONT: &str = r#"<div class="jp">{{Sentence}}</div>
<div class="word">{{Word}}</div>"#;

pub(crate) const BACK: &str = r#"<div class="jp">{{furigana:Furigana}}</div>
<div class="word">{{Word}}</div>
<hr id="answer">
<div class="tr">{{Translation}}</div>
{{#Note}}<div class="note">{{Note}}</div>{{/Note}}
{{Audio}}"#;

pub(crate) const CSS: &str = r#".card { font-family: "Noto Sans JP", "Yu Gothic", "Hiragino Sans", sans-serif; text-align: center; font-size: 22px; }
.jp { font-size: 32px; line-height: 2.2; }
.word { color: #888; font-size: 16px; margin-top: 8px; }
.tr { font-size: 20px; }
.note { color: #666; font-size: 15px; margin-top: 8px; }
ruby rt { font-size: 0.5em; }
.credit { font-size: 11px; color: #999; margin-top: 6px; }"#;
