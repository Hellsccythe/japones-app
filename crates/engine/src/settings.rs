use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Sentence generator: "local" (llama.cpp) or "claude".
    pub llm_provider: String,
    /// Catalog id of the local model (see `local_llm::MODELS`).
    pub local_model: String,
    /// Generation threads for the local model; 0 = auto.
    pub llm_threads: u32,

    /// Empty means "use the ANTHROPIC_API_KEY environment variable".
    pub anthropic_api_key: String,
    pub model: String,
    /// JLPT level hint for generated sentences ("N5".."N1", or empty).
    pub level: String,
    /// Tatoeba translation languages in order of preference (ISO 639-3).
    pub translation_langs: Vec<String>,
    /// Language name used in the LLM prompt for translations/notes.
    pub translation_language_name: String,

    /// "embedded" (VOICEVOX CORE inside the app) or "http" (a running VOICEVOX).
    pub tts_backend: String,
    /// The user accepted the VOICEVOX ONNX Runtime and voice model terms
    /// (required before downloading them).
    pub voice_terms_accepted: bool,
    pub voicevox_url: String,
    /// VOICEVOX style id (same ids embedded and over HTTP).
    pub speaker: u32,
    pub speed: f32,

    pub anki_connect_url: String,
    pub deck: String,

    /// The first-run checklist was closed (it can be reopened in the settings).
    pub setup_done: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            llm_provider: "local".into(),
            local_model: local_llm::DEFAULT_ID.into(),
            llm_threads: 0,
            anthropic_api_key: String::new(),
            model: sentences::llm::DEFAULT_MODEL.into(),
            level: String::new(),
            translation_langs: vec!["por".into(), "eng".into()],
            translation_language_name: "português".into(),
            tts_backend: "embedded".into(),
            voice_terms_accepted: false,
            voicevox_url: tts::DEFAULT_URL.into(),
            speaker: tts::DEFAULT_SPEAKER,
            speed: 0.9,
            anki_connect_url: anki::connect::DEFAULT_URL.into(),
            deck: "Japonês::Frases".into(),
            setup_done: false,
        }
    }
}

impl Settings {
    pub fn load(path: &Path) -> Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(s) => Ok(serde_json::from_str(&s)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.into()),
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        std::fs::write(path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }

    pub fn uses_voicevox_http(&self) -> bool {
        self.tts_backend == "http"
    }

    pub fn uses_claude(&self) -> bool {
        self.llm_provider == "claude"
    }

    pub fn api_key(&self) -> Option<String> {
        Some(self.anthropic_api_key.trim().to_string())
            .filter(|k| !k.is_empty())
            .or_else(|| std::env::var("ANTHROPIC_API_KEY").ok().filter(|k| !k.is_empty()))
    }
}
