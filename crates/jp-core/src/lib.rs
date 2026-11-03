//! Japanese text analysis: morphological analysis (lindera + IPADIC),
//! lemma extraction and furigana alignment.

mod furigana;

use std::borrow::Cow;

use lindera::dictionary::load_dictionary;
use lindera::mode::Mode;
use lindera::segmenter::Segmenter;
use serde::{Deserialize, Serialize};
use wana_kana::ConvertJapanese;

pub use furigana::{align, has_kanji, is_kanji, Segment};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("lindera: {0}")]
    Lindera(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// One morpheme as returned by the analyzer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Morph {
    pub surface: String,
    /// Dictionary form (e.g. 食べ -> 食べる). Falls back to the surface.
    pub base: String,
    /// Reading in hiragana, if the dictionary knows it.
    pub reading: Option<String>,
    /// Coarse part of speech (動詞, 名詞, ...).
    pub pos: String,
}

/// Wraps a lindera segmenter. Loading the dictionary is the expensive part,
/// so build one `Analyzer` and share it.
pub struct Analyzer {
    segmenter: Segmenter,
}

// IPADIC detail columns.
const POS: usize = 0;
const BASE_FORM: usize = 6;
const READING: usize = 7;

impl Analyzer {
    pub fn new() -> Result<Self> {
        let dictionary =
            load_dictionary("embedded://ipadic").map_err(|e| Error::Lindera(e.to_string()))?;
        Ok(Self {
            segmenter: Segmenter::new(Mode::Normal, dictionary, None),
        })
    }

    pub fn morphs(&self, text: &str) -> Result<Vec<Morph>> {
        let mut tokens = self
            .segmenter
            .segment(Cow::Borrowed(text))
            .map_err(|e| Error::Lindera(e.to_string()))?;

        Ok(tokens
            .iter_mut()
            .map(|t| {
                let surface = t.surface.to_string();
                let details: Vec<String> = t.details().iter().map(|s| s.to_string()).collect();
                let field = |i: usize| {
                    details
                        .get(i)
                        .filter(|s| !s.is_empty() && *s != "*")
                        .cloned()
                };
                Morph {
                    base: field(BASE_FORM).unwrap_or_else(|| surface.clone()),
                    reading: field(READING).map(|r| r.to_hiragana()),
                    pos: field(POS).unwrap_or_default(),
                    surface,
                }
            })
            .collect())
    }

    /// Dictionary forms of every content word in `text`, in order.
    pub fn lemmas(&self, text: &str) -> Result<Vec<String>> {
        Ok(self
            .morphs(text)?
            .into_iter()
            .filter(|m| !matches!(m.pos.as_str(), "記号" | "助詞" | "助動詞"))
            .map(|m| m.base)
            .collect())
    }

    /// Splits `text` into segments, attaching hiragana readings to kanji runs.
    pub fn furigana(&self, text: &str) -> Result<Vec<Segment>> {
        let mut out: Vec<Segment> = Vec::new();
        for m in self.morphs(text)? {
            let segs = match &m.reading {
                Some(r) if has_kanji(&m.surface) => align(&m.surface, r),
                _ => vec![Segment::plain(&m.surface)],
            };
            for s in segs {
                // Merge neighbouring plain text to keep output compact.
                match (out.last_mut(), &s.reading) {
                    (Some(last), None) if last.reading.is_none() => last.text.push_str(&s.text),
                    _ => out.push(s),
                }
            }
        }
        Ok(out)
    }

    /// Anki `{{furigana:}}` format: ` 食[た]べる`.
    pub fn to_anki(&self, text: &str) -> Result<String> {
        Ok(furigana::render_anki(&self.furigana(text)?))
    }

    /// HTML with `<ruby>` tags.
    pub fn to_html(&self, text: &str) -> Result<String> {
        Ok(furigana::render_html(&self.furigana(text)?))
    }

    /// Whole-text reading in hiragana (kana-only rendering).
    pub fn to_kana(&self, text: &str) -> Result<String> {
        Ok(self
            .furigana(text)?
            .into_iter()
            .map(|s| s.reading.unwrap_or(s.text))
            .collect())
    }
}

pub use furigana::{render_anki, render_html};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lemmatizes_conjugated_verb() {
        let a = Analyzer::new().unwrap();
        let lemmas = a.lemmas("昨日、寿司を食べました。").unwrap();
        assert!(lemmas.contains(&"食べる".to_string()), "{lemmas:?}");
    }

    #[test]
    fn furigana_skips_okurigana() {
        let a = Analyzer::new().unwrap();
        assert_eq!(a.to_anki("食べる").unwrap(), " 食[た]べる");
        assert_eq!(
            a.to_html("学校に行く").unwrap(),
            "<ruby>学校<rt>がっこう</rt></ruby>に<ruby>行<rt>い</rt></ruby>く"
        );
    }
}
