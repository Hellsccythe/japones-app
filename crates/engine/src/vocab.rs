//! Word-by-word vocabulary of a sentence, for the sentence breakdown.

use jp_core::{has_kanji, Token, WordClass};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VocabEntry {
    /// Dictionary form.
    pub base: String,
    /// Reading in hiragana, only when `base` has kanji.
    pub reading: Option<String>,
    pub class: WordClass,
    /// Short meaning (JMdict English, or a translation); empty when unknown.
    pub gloss: String,
}

/// Groups `tokens` by dictionary form, in sentence order, skipping
/// punctuation, symbols and whitespace.
pub fn vocabulary(
    tokens: &[Token],
    lookup: impl Fn(&str) -> Option<String>,
    kana: impl Fn(&str) -> Option<String>,
) -> Vec<VocabEntry> {
    let mut out: Vec<VocabEntry> = Vec::new();
    for t in tokens {
        if is_filler(t) || out.iter().any(|e| e.base == t.base) {
            continue;
        }
        out.push(VocabEntry {
            reading: if has_kanji(&t.base) { kana(&t.base) } else { None },
            gloss: lookup(&t.base).unwrap_or_default(),
            base: t.base.clone(),
            class: t.class,
        });
    }
    out
}

/// Punctuation, symbols and whitespace (kana and kanji count as letters).
fn is_filler(t: &Token) -> bool {
    t.class == WordClass::Other && t.surface.chars().all(|c| !c.is_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::*;
    use jp_core::Analyzer;

    #[test]
    fn groups_conjugations_and_keeps_order() {
        let a = Analyzer::new().unwrap();
        let t = a.analyze("見学していってください。").unwrap();
        let v = vocabulary(&t, |_| None, |s| a.to_kana(s).ok());
        let bases: Vec<_> = v.iter().map(|e| e.base.as_str()).collect();
        assert_eq!(bases.first(), Some(&"見学"));
        assert!(bases.contains(&"する") && bases.contains(&"いく"), "{bases:?}");
        assert_eq!(bases.iter().filter(|b| **b == "て").count(), 1); // repeated, listed once
        assert!(!bases.contains(&"。"));
    }

    #[test]
    fn unknown_word_has_empty_gloss() {
        let a = Analyzer::new().unwrap();
        let v = vocabulary(&a.analyze("ねえ").unwrap(), |_| None, |s| a.to_kana(s).ok());
        assert!(v.iter().all(|e| e.gloss.is_empty()));
    }

    #[test]
    fn reading_only_with_kanji() {
        let a = Analyzer::new().unwrap();
        let v = vocabulary(&a.analyze("見学する").unwrap(), |_| Some("x".into()), |s| a.to_kana(s).ok());
        assert_eq!(v[0].reading.as_deref(), Some("けんがく"));
        assert_eq!(v[1].reading, None);
        assert_eq!(v[0].gloss, "x");
    }

    #[test]
    fn punctuation_only_is_empty() {
        let a = Analyzer::new().unwrap();
        assert!(vocabulary(&a.analyze("。").unwrap(), |_| None, |_| None).is_empty());
        assert!(vocabulary(&a.analyze("").unwrap(), |_| None, |_| None).is_empty());
    }
}
