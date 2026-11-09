use std::collections::{HashMap, HashSet};
use std::path::Path;

use jp_core::Analyzer;
use rusqlite::{params, Connection, OptionalExtension};

use crate::{contains_word, word_lemmas, Example, Result, Source};

/// Sentence length (in characters) ranked first: long enough to give
/// context, short enough for a flashcard.
const IDEAL_LEN: usize = 16;

/// SQLite database holding the Tatoeba sentences and the lemma index.
pub struct Store {
    conn: Connection,
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS sentences (id INTEGER PRIMARY KEY, lang TEXT NOT NULL, text TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS links (jpn_id INTEGER NOT NULL, trans_id INTEGER NOT NULL, PRIMARY KEY (jpn_id, trans_id));
CREATE TABLE IF NOT EXISTS lemma_index (lemma TEXT NOT NULL, sentence_id INTEGER NOT NULL, PRIMARY KEY (lemma, sentence_id)) WITHOUT ROWID;
CREATE INDEX IF NOT EXISTS sentences_lang ON sentences(lang);
CREATE TABLE IF NOT EXISTS vocab_gloss (base TEXT NOT NULL, reading TEXT NOT NULL, lang TEXT NOT NULL, gloss TEXT NOT NULL, PRIMARY KEY (base, reading, lang)) WITHOUT ROWID;
";

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;")?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self { conn })
    }

    pub fn japanese_count(&self) -> Result<i64> {
        Ok(self
            .conn
            .query_row("SELECT count(*) FROM sentences WHERE lang = 'jpn'", [], |r| r.get(0))?)
    }

    pub fn indexed_count(&self) -> Result<i64> {
        Ok(self
            .conn
            .query_row("SELECT count(DISTINCT sentence_id) FROM lemma_index", [], |r| r.get(0))?)
    }

    /// Inserts `(id, lang, text)` rows. Rows whose id is not in `keep` are
    /// skipped when a filter is given (used to load only linked translations).
    pub fn insert_sentences(
        &mut self,
        rows: impl Iterator<Item = (i64, String, String)>,
        keep: Option<&HashSet<i64>>,
    ) -> Result<usize> {
        let tx = self.conn.transaction()?;
        let mut n = 0;
        {
            let mut stmt =
                tx.prepare("INSERT OR REPLACE INTO sentences (id, lang, text) VALUES (?1, ?2, ?3)")?;
            for (id, lang, text) in rows {
                if keep.map_or(true, |k| k.contains(&id)) {
                    stmt.execute(params![id, lang, text])?;
                    n += 1;
                }
            }
        }
        tx.commit()?;
        Ok(n)
    }

    pub fn insert_links(&mut self, rows: impl Iterator<Item = (i64, i64)>) -> Result<HashSet<i64>> {
        let tx = self.conn.transaction()?;
        let mut targets = HashSet::new();
        {
            let mut stmt =
                tx.prepare("INSERT OR IGNORE INTO links (jpn_id, trans_id) VALUES (?1, ?2)")?;
            for (a, b) in rows {
                stmt.execute(params![a, b])?;
                targets.insert(b);
            }
        }
        tx.commit()?;
        Ok(targets)
    }

    /// (Re)builds the lemma index for every Japanese sentence.
    /// `progress` is called with (done, total) every few thousand sentences.
    pub fn build_index(&mut self, analyzer: &Analyzer, mut progress: impl FnMut(usize, usize)) -> Result<()> {
        let rows: Vec<(i64, String)> = {
            let mut stmt = self.conn.prepare("SELECT id, text FROM sentences WHERE lang = 'jpn'")?;
            let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
            rows.collect::<std::result::Result<_, _>>()?
        };
        let total = rows.len();
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM lemma_index", [])?;
        {
            let mut stmt =
                tx.prepare("INSERT OR IGNORE INTO lemma_index (lemma, sentence_id) VALUES (?1, ?2)")?;
            for (i, (id, text)) in rows.iter().enumerate() {
                for lemma in analyzer.lemmas(text)? {
                    stmt.execute(params![lemma, id])?;
                }
                if i % 5000 == 0 {
                    progress(i, total);
                }
            }
        }
        tx.commit()?;
        progress(total, total);
        Ok(())
    }

    /// Finds up to `limit` corpus sentences containing `word` in any
    /// conjugation. Sentences with a translation come first, then those
    /// closest to [`IDEAL_LEN`].
    pub fn find(&self, analyzer: &Analyzer, word: &str, langs: &[String], limit: usize) -> Result<Vec<Example>> {
        let lemmas = word_lemmas(analyzer, word)?;

        // Candidates containing every lemma; contiguity is checked afterwards.
        let placeholders = vec!["?"; lemmas.len()].join(",");
        let sql = format!(
            "SELECT s.id, s.text FROM sentences s
             JOIN (SELECT sentence_id FROM lemma_index WHERE lemma IN ({placeholders})
                   GROUP BY sentence_id HAVING count(DISTINCT lemma) = {n}) m ON m.sentence_id = s.id
             ORDER BY (SELECT count(*) FROM links l WHERE l.jpn_id = s.id) = 0,
                      abs(length(s.text) - {IDEAL_LEN}), s.id
             LIMIT {cap}",
            n = lemmas.iter().collect::<HashSet<_>>().len(),
            cap = limit * 10,
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let candidates: Vec<(i64, String)> = stmt
            .query_map(rusqlite::params_from_iter(lemmas.iter()), |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<std::result::Result<_, _>>()?;

        let mut out = Vec::new();
        for (id, text) in candidates {
            if out.len() >= limit {
                break;
            }
            if lemmas.len() > 1 && !contains_word(analyzer, &text, &lemmas)? {
                continue;
            }
            out.push(Example {
                translation: self.translation(id, langs)?,
                japanese: text,
                note: None,
                source: Source::Tatoeba { id },
            });
        }
        Ok(out)
    }

    fn translation(&self, jpn_id: i64, langs: &[String]) -> Result<Option<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT t.text FROM links l JOIN sentences t ON t.id = l.trans_id
             WHERE l.jpn_id = ?1 AND t.lang = ?2 LIMIT 1",
        )?;
        for lang in langs {
            let mut rows = stmt.query(params![jpn_id, lang])?;
            if let Some(row) = rows.next()? {
                return Ok(Some(row.get(0)?));
            }
        }
        Ok(None)
    }
}

/// Vocabulary meanings translated by the AI, keyed by `(base, reading)` and language.
/// `reading` is "" when the word has none.
impl Store {
    pub fn cached_glosses(&self, keys: &[(String, String)], lang: &str) -> Result<HashMap<(String, String), String>> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT gloss FROM vocab_gloss WHERE base = ?1 AND reading = ?2 AND lang = ?3")?;
        let mut out = HashMap::new();
        for (base, reading) in keys {
            let gloss: Option<String> = stmt.query_row(params![base, reading, lang], |r| r.get(0)).optional()?;
            if let Some(g) = gloss.filter(|g| !g.is_empty()) {
                out.insert((base.clone(), reading.clone()), g);
            }
        }
        Ok(out)
    }

    /// Saves `(base, reading, gloss)` rows; empty glosses are skipped.
    pub fn save_glosses(&mut self, rows: &[(String, String, String)], lang: &str) -> Result<()> {
        let tx = self.conn.transaction()?;
        {
            let mut stmt = tx.prepare_cached(
                "INSERT OR REPLACE INTO vocab_gloss (base, reading, lang, gloss) VALUES (?1, ?2, ?3, ?4)",
            )?;
            for (base, reading, gloss) in rows.iter().filter(|(_, _, g)| !g.trim().is_empty()) {
                stmt.execute(params![base, reading, lang, gloss.trim()])?;
            }
        }
        tx.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_conjugated_forms_with_translation() {
        let dir = std::env::temp_dir().join(format!("jp-store-test-{}", std::process::id()));
        let mut store = Store::open(&dir.join("t.db")).unwrap();
        let analyzer = Analyzer::new().unwrap();

        let s = |id: i64, lang: &str, t: &str| (id, lang.to_string(), t.to_string());
        store
            .insert_sentences(
                vec![
                    s(1, "jpn", "私は毎朝パンを食べます。"),
                    s(2, "jpn", "昨日、寿司を食べた。"),
                    s(3, "jpn", "本を読む。"),
                    s(10, "por", "Ontem eu comi sushi."),
                ]
                .into_iter(),
                None,
            )
            .unwrap();
        store.insert_links(vec![(2, 10)].into_iter()).unwrap();
        store.build_index(&analyzer, |_, _| {}).unwrap();

        let found = store.find(&analyzer, "食べる", &["por".into()], 5).unwrap();
        assert_eq!(found.len(), 2);
        // The one with a translation is ranked first.
        assert_eq!(found[0].japanese, "昨日、寿司を食べた。");
        assert_eq!(found[0].translation.as_deref(), Some("Ontem eu comi sushi."));

        drop(store);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn gloss_cache_roundtrip_and_skips_empty() {
        let dir = std::env::temp_dir().join(format!("jp-gloss-test-{}", std::process::id()));
        let mut s = Store::open(&dir.join("t.db")).unwrap();
        s.save_glosses(
            &[("見学".into(), "けんがく".into(), "visitar".into()), ("に".into(), String::new(), String::new())],
            "português",
        )
        .unwrap();
        let got = s
            .cached_glosses(&[("見学".into(), "けんがく".into()), ("に".into(), String::new())], "português")
            .unwrap();
        assert_eq!(got.get(&("見学".into(), "けんがく".into())).map(String::as_str), Some("visitar"));
        assert!(!got.contains_key(&("に".into(), String::new())));
        assert!(s.cached_glosses(&[("見学".into(), "けんがく".into())], "english").unwrap().is_empty());

        // Saving again replaces the old meaning.
        s.save_glosses(&[("見学".into(), "けんがく".into(), "visita".into())], "português").unwrap();
        let got = s.cached_glosses(&[("見学".into(), "けんがく".into())], "português").unwrap();
        assert_eq!(got.values().next().map(String::as_str), Some("visita"));

        drop(s);
        let _ = std::fs::remove_dir_all(dir);
    }
}
