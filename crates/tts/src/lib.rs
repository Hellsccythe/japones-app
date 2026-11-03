//! Text-to-speech with VOICEVOX (https://voicevox.hiroshiba.jp): embedded in
//! the app ([`embedded`]) or through a running VOICEVOX engine's HTTP API
//! (by default on port 50021).

mod catalog;
pub mod embedded;

use serde::{Deserialize, Serialize};

pub use catalog::{CHARACTERS, PACKS, STYLES};

/// A downloadable `.vvm` file with the voices it contains.
#[derive(Debug, Clone, Serialize)]
pub struct VoicePack {
    pub file: &'static str,
    pub url: &'static str,
    pub size: u64,
    pub sha256: &'static str,
    pub characters: &'static [&'static str],
}

/// One voice style. Ids are the same as in the VOICEVOX app and engine.
#[derive(Debug, Clone, Serialize)]
pub struct Style {
    pub id: u32,
    pub character: &'static str,
    pub name: &'static str,
    pub pack: &'static str,
}

/// Credit required by a character's terms when publishing its audio.
#[derive(Debug, Clone, Serialize)]
pub struct Character {
    pub name: &'static str,
    /// e.g. "VOICEVOX:ずんだもん"
    pub credit: &'static str,
    pub terms_url: &'static str,
    /// Extra restrictions in Portuguese, empty if none.
    pub note: &'static str,
}

pub fn style(id: u32) -> Option<&'static Style> {
    STYLES.iter().find(|s| s.id == id)
}

pub fn character(name: &str) -> Option<&'static Character> {
    CHARACTERS.iter().find(|c| c.name == name)
}

/// Credit line for audio made with style `id` ("VOICEVOX" alone for voices
/// outside the catalog, e.g. ones added to a VOICEVOX engine).
pub fn credit(id: u32) -> String {
    style(id)
        .and_then(|s| character(s.character))
        .map_or_else(|| "VOICEVOX".to_string(), |c| c.credit.to_string())
}

/// Speakers grouped as the VOICEVOX engine lists them, for the given packs.
pub fn speakers_in(packs: &[&str]) -> Vec<Speaker> {
    let mut out: Vec<Speaker> = Vec::new();
    for s in STYLES.iter().filter(|s| packs.contains(&s.pack)) {
        let style = SpeakerStyle { id: s.id, name: s.name.to_string() };
        match out.iter_mut().find(|sp| sp.name == s.character) {
            Some(sp) => sp.styles.push(style),
            None => out.push(Speaker { name: s.character.to_string(), styles: vec![style] }),
        }
    }
    out
}

pub const DEFAULT_URL: &str = "http://127.0.0.1:50021";
/// ずんだもん (ノーマル) in the stock VOICEVOX speaker list.
pub const DEFAULT_SPEAKER: u32 = 3;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("VOICEVOX não respondeu ({0}). O VOICEVOX está aberto?")]
    Unreachable(reqwest::Error),
    #[error("VOICEVOX: {0}")]
    Engine(String),
    #[error("voz embutida: {0}")]
    Core(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// A style as listed by the VOICEVOX engine's `/speakers`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeakerStyle {
    pub id: u32,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Speaker {
    pub name: String,
    pub styles: Vec<SpeakerStyle>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct VoiceOptions {
    pub speaker: u32,
    /// 1.0 = normal; learners usually like ~0.8–0.9.
    pub speed: f32,
}

impl Default for VoiceOptions {
    fn default() -> Self {
        Self { speaker: DEFAULT_SPEAKER, speed: 1.0 }
    }
}

pub struct Voicevox {
    http: reqwest::Client,
    url: String,
}

impl Voicevox {
    pub fn new(url: Option<String>) -> Self {
        Self {
            http: reqwest::Client::new(),
            url: url
                .filter(|u| !u.is_empty())
                .unwrap_or_else(|| DEFAULT_URL.into())
                .trim_end_matches('/')
                .to_string(),
        }
    }

    async fn check(resp: reqwest::Response) -> Result<reqwest::Response> {
        if resp.status().is_success() {
            Ok(resp)
        } else {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            Err(Error::Engine(format!("{status}: {body}")))
        }
    }

    pub async fn speakers(&self) -> Result<Vec<Speaker>> {
        let resp = self
            .http
            .get(format!("{}/speakers", self.url))
            .send()
            .await
            .map_err(Error::Unreachable)?;
        Self::check(resp).await?.json().await.map_err(Error::Unreachable)
    }

    /// Synthesizes `text` and returns WAV bytes.
    pub async fn synthesize(&self, text: &str, opts: VoiceOptions) -> Result<Vec<u8>> {
        let speaker = opts.speaker.to_string();
        let resp = self
            .http
            .post(format!("{}/audio_query", self.url))
            .query(&[("text", text), ("speaker", &speaker)])
            .send()
            .await
            .map_err(Error::Unreachable)?;
        let mut query: serde_json::Value =
            Self::check(resp).await?.json().await.map_err(Error::Unreachable)?;
        query["speedScale"] = serde_json::json!(opts.speed);

        let resp = self
            .http
            .post(format!("{}/synthesis", self.url))
            .query(&[("speaker", &speaker)])
            .json(&query)
            .send()
            .await
            .map_err(Error::Unreachable)?;
        let bytes = Self::check(resp).await?.bytes().await.map_err(Error::Unreachable)?;
        Ok(bytes.to_vec())
    }
}
