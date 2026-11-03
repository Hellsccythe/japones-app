//! Client for the AnkiConnect add-on (https://ankiweb.net/shared/info/2055492159).

use base64::Engine;
use serde_json::{json, Value};

use crate::{Card, Error, Result, BACK, CSS, FIELDS, FRONT, MODEL_NAME};

pub const DEFAULT_URL: &str = "http://127.0.0.1:8765";

pub struct AnkiConnect {
    http: reqwest::Client,
    url: String,
}

impl AnkiConnect {
    pub fn new(url: Option<String>) -> Self {
        Self {
            http: reqwest::Client::new(),
            url: url.filter(|u| !u.is_empty()).unwrap_or_else(|| DEFAULT_URL.into()),
        }
    }

    async fn call(&self, action: &str, params: Value) -> Result<Value> {
        let resp: Value = self
            .http
            .post(&self.url)
            .json(&json!({"action": action, "version": 6, "params": params}))
            .send()
            .await
            .map_err(Error::Unreachable)?
            .json()
            .await
            .map_err(Error::Unreachable)?;
        match resp["error"].as_str() {
            Some(e) => Err(Error::Connect(e.to_string())),
            None => Ok(resp["result"].clone()),
        }
    }

    pub async fn deck_names(&self) -> Result<Vec<String>> {
        Ok(serde_json::from_value(self.call("deckNames", json!({})).await?).unwrap_or_default())
    }

    /// Creates the deck and the note type if they don't exist yet.
    async fn ensure_setup(&self, deck: &str) -> Result<()> {
        self.call("createDeck", json!({"deck": deck})).await?;
        let models: Vec<String> =
            serde_json::from_value(self.call("modelNames", json!({})).await?).unwrap_or_default();
        if !models.iter().any(|m| m == MODEL_NAME) {
            self.call(
                "createModel",
                json!({
                    "modelName": MODEL_NAME,
                    "inOrderFields": FIELDS,
                    "css": CSS,
                    "isCloze": false,
                    "cardTemplates": [{"Name": "Leitura", "Front": FRONT, "Back": BACK}]
                }),
            )
            .await?;
        }
        Ok(())
    }

    /// Adds the cards to `deck`. Returns how many were added; duplicates
    /// (same sentence already in the deck) are skipped by Anki.
    pub async fn add_cards(&self, deck: &str, cards: &[Card], tags: &[String]) -> Result<usize> {
        self.ensure_setup(deck).await?;
        let notes: Vec<Value> = cards
            .iter()
            .map(|c| {
                // AnkiConnect appends the [sound:] tag to the Audio field, after the credit.
                let [word, sentence, furigana, translation, note, audio] = c.fields("");
                let mut n = json!({
                    "deckName": deck,
                    "modelName": MODEL_NAME,
                    "fields": {
                        "Word": word, "Sentence": sentence, "Furigana": furigana,
                        "Translation": translation, "Note": note, "Audio": audio,
                    },
                    "tags": tags,
                    "options": {"allowDuplicate": false, "duplicateScope": "deck"}
                });
                if let Some(wav) = &c.audio {
                    n["audio"] = json!([{
                        "data": base64::engine::general_purpose::STANDARD.encode(wav),
                        "filename": c.audio_filename(),
                        "fields": ["Audio"]
                    }]);
                }
                n
            })
            .collect();

        // addNotes returns null for each note that failed (e.g. duplicate).
        let ids = self.call("addNotes", json!({"notes": notes})).await;
        match ids {
            Ok(Value::Array(ids)) => Ok(ids.iter().filter(|id| !id.is_null()).count()),
            Ok(_) => Ok(0),
            Err(e) => Err(e),
        }
    }
}
