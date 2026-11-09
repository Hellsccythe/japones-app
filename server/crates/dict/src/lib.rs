//! JMdict lookups (meaning, readings, part of speech), using the JSON
//! release of jmdict-simplified (https://github.com/scriptin/jmdict-simplified).

use std::io::Read;
use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("database: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),
    #[error("zip: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// A dictionary entry, trimmed to what the app shows.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub kanji: Vec<String>,
    pub kana: Vec<String>,
    pub senses: Vec<Sense>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sense {
    /// JMdict tags, e.g. "v1" (ichidan), "v5k" (godan -ku), "vt" (transitive).
    pub pos: Vec<String>,
    pub glosses: Vec<String>,
}

// --- jmdict-simplified JSON shape (only the fields we need) ---
#[derive(Deserialize)]
struct RawFile {
    words: Vec<RawWord>,
}
#[derive(Deserialize)]
struct RawWord {
    id: String,
    kanji: Vec<RawText>,
    kana: Vec<RawText>,
    sense: Vec<RawSense>,
}
#[derive(Deserialize)]
struct RawText {
    text: String,
    common: bool,
}
#[derive(Deserialize)]
struct RawSense {
    #[serde(rename = "partOfSpeech")]
    part_of_speech: Vec<String>,
    gloss: Vec<RawGloss>,
}
#[derive(Deserialize)]
struct RawGloss {
    text: String,
}

pub struct Dict {
    conn: Connection,
}

impl Dict {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS dict_entries (id INTEGER PRIMARY KEY, common INTEGER NOT NULL, json TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS dict_keys (key TEXT NOT NULL, entry_id INTEGER NOT NULL, PRIMARY KEY (key, entry_id)) WITHOUT ROWID;",
        )?;
        Ok(Self { conn })
    }

    pub fn is_empty(&self) -> Result<bool> {
        let n: i64 = self.conn.query_row("SELECT count(*) FROM dict_entries", [], |r| r.get(0))?;
        Ok(n == 0)
    }

    /// Imports a jmdict-simplified `.json` (or the `.json.zip` release asset).
    pub fn import_file(&mut self, path: &Path) -> Result<usize> {
        let bytes = std::fs::read(path)?;
        self.import_bytes(&bytes)
    }

    fn import_bytes(&mut self, bytes: &[u8]) -> Result<usize> {
        let json = if bytes.starts_with(b"PK") {
            let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))?;
            let mut file = archive.by_index(0)?;
            let mut s = Vec::new();
            file.read_to_end(&mut s)?;
            s
        } else {
            bytes.to_vec()
        };
        let raw: RawFile = serde_json::from_slice(&json)?;

        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM dict_entries", [])?;
        tx.execute("DELETE FROM dict_keys", [])?;
        {
            let mut ins = tx.prepare("INSERT INTO dict_entries (id, common, json) VALUES (?1, ?2, ?3)")?;
            let mut key = tx.prepare("INSERT OR IGNORE INTO dict_keys (key, entry_id) VALUES (?1, ?2)")?;
            for w in &raw.words {
                let id: i64 = w.id.parse().map_err(|_| Error::Other(format!("bad id {}", w.id)))?;
                let common = w.kanji.iter().chain(&w.kana).any(|t| t.common);
                let entry = Entry {
                    kanji: w.kanji.iter().map(|t| t.text.clone()).collect(),
                    kana: w.kana.iter().map(|t| t.text.clone()).collect(),
                    senses: w
                        .sense
                        .iter()
                        .map(|s| Sense {
                            pos: s.part_of_speech.clone(),
                            glosses: s.gloss.iter().map(|g| g.text.clone()).collect(),
                        })
                        .collect(),
                };
                ins.execute(params![id, common, serde_json::to_string(&entry)?])?;
                for t in w.kanji.iter().chain(&w.kana) {
                    key.execute(params![t.text, id])?;
                }
            }
        }
        tx.commit()?;
        Ok(raw.words.len())
    }

    /// Downloads the latest English JMdict release from GitHub and imports it.
    pub async fn download_and_import(&mut self) -> Result<usize> {
        let http = reqwest::Client::builder().user_agent("japones-app").build()?;
        let release: serde_json::Value = http
            .get("https://api.github.com/repos/scriptin/jmdict-simplified/releases/latest")
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let url = release["assets"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|a| {
                let name = a["name"].as_str().unwrap_or("");
                name.starts_with("jmdict-eng-") && !name.contains("common") && name.ends_with(".json.zip")
            })
            .and_then(|a| a["browser_download_url"].as_str())
            .ok_or_else(|| Error::Other("JMdict asset not found in latest release".into()))?
            .to_string();
        let bytes = http.get(url).send().await?.error_for_status()?.bytes().await?;
        self.import_bytes(&bytes)
    }

    /// Entries whose kanji or kana form is exactly `word`, common words first.
    pub fn lookup(&self, word: &str) -> Result<Vec<Entry>> {
        let mut stmt = self.conn.prepare(
            "SELECT e.json FROM dict_keys k JOIN dict_entries e ON e.id = k.entry_id
             WHERE k.key = ?1 ORDER BY e.common DESC, e.id LIMIT 5",
        )?;
        let rows = stmt.query_map([word], |r| r.get::<_, String>(0))?;
        rows.map(|j| Ok(serde_json::from_str(&j?)?)).collect()
    }

    /// The first (most common) entry, if any.
    pub fn first(&self, word: &str) -> Result<Option<Entry>> {
        let json: Option<String> = self
            .conn
            .query_row(
                "SELECT e.json FROM dict_keys k JOIN dict_entries e ON e.id = k.entry_id
                 WHERE k.key = ?1 ORDER BY e.common DESC, e.id LIMIT 1",
                [word],
                |r| r.get(0),
            )
            .optional()?;
        json.map(|j| Ok(serde_json::from_str(&j)?)).transpose()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imports_and_looks_up() {
        let dir = std::env::temp_dir().join(format!("dict-test-{}", std::process::id()));
        let mut d = Dict::open(&dir.join("d.db")).unwrap();
        let json = r#"{"words":[{"id":"1358280","kanji":[{"common":true,"text":"食べる","tags":[]}],
            "kana":[{"common":true,"text":"たべる","tags":[],"appliesToKanji":["*"]}],
            "sense":[{"partOfSpeech":["v1","vt"],"gloss":[{"lang":"eng","text":"to eat"}]}]}]}"#;
        assert_eq!(d.import_bytes(json.as_bytes()).unwrap(), 1);
        let e = d.first("たべる").unwrap().unwrap();
        assert_eq!(e.kanji, vec!["食べる"]);
        assert_eq!(e.senses[0].pos, vec!["v1", "vt"]);
        drop(d);
        let _ = std::fs::remove_dir_all(dir);
    }
}
