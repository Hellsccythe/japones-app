//! Client for the AnkiConnect add-on (https://ankiweb.net/shared/info/2055492159).

use std::time::Duration;

use base64::Engine;
use serde_json::{json, Value};

use crate::{Card, Error, Result, BACK, CSS, FIELDS, FRONT, MODEL_NAME};

pub const DEFAULT_URL: &str = "http://127.0.0.1:8765";

/// Nobody listening (or no answer in time) means Anki isn't open.
fn request_error(e: reqwest::Error) -> Error {
    if e.is_connect() || e.is_timeout() {
        Error::NotRunning
    } else {
        Error::Unreachable(e)
    }
}

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

    /// Checks that Anki is open with a recent AnkiConnect. Returns its version.
    pub async fn check(&self) -> Result<u32> {
        let body = self
            .http
            .post(&self.url)
            .timeout(Duration::from_secs(2))
            .json(&json!({"action": "version", "version": 6}))
            .send()
            .await
            .map_err(request_error)?
            .text()
            .await
            .map_err(request_error)?;
        // Something else answering on the port (or an old add-on) isn't AnkiConnect v6.
        let version = serde_json::from_str::<Value>(&body)
            .ok()
            .and_then(|v| v["result"].as_u64())
            .ok_or(Error::NoAddon)? as u32;
        if version < 6 {
            return Err(Error::Incompatible(version));
        }
        Ok(version)
    }

    async fn call(&self, action: &str, params: Value) -> Result<Value> {
        let resp: Value = self
            .http
            .post(&self.url)
            .json(&json!({"action": action, "version": 6, "params": params}))
            .send()
            .await
            .map_err(request_error)?
            .json()
            .await
            .map_err(request_error)?;
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
        self.check().await?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    /// Serves one fixed HTTP response per connection and returns its URL.
    async fn serve(content_type: &'static str, body: &'static str) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move {
            while let Ok((mut sock, _)) = listener.accept().await {
                let mut buf = [0u8; 4096];
                let _ = sock.read(&mut buf).await;
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = sock.write_all(resp.as_bytes()).await;
            }
        });
        url
    }

    #[tokio::test]
    async fn check_reads_version() {
        let client = AnkiConnect::new(Some(serve("application/json", r#"{"result":6,"error":null}"#).await));
        assert_eq!(client.check().await.unwrap(), 6);
    }

    #[tokio::test]
    async fn check_not_running() {
        // Bind and drop to get a port nobody listens on.
        let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        let client = AnkiConnect::new(Some(format!("http://127.0.0.1:{port}")));
        assert!(matches!(client.check().await, Err(Error::NotRunning)));
    }

    #[tokio::test]
    async fn check_no_addon_on_html() {
        let client = AnkiConnect::new(Some(serve("text/html", "<html></html>").await));
        assert!(matches!(client.check().await, Err(Error::NoAddon)));
    }

    #[tokio::test]
    async fn check_incompatible() {
        let client = AnkiConnect::new(Some(serve("application/json", r#"{"result":5,"error":null}"#).await));
        assert!(matches!(client.check().await, Err(Error::Incompatible(5))));
    }
}
