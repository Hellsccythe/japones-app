//! Word-by-word vocabulary of a sentence, for the sentence breakdown.

use std::collections::HashMap;

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
    lookup: impl Fn(&str, WordClass) -> Option<String>,
    kana: impl Fn(&str) -> Option<String>,
) -> Vec<VocabEntry> {
    let mut out: Vec<VocabEntry> = Vec::new();
    for t in tokens {
        if is_filler(t) || out.iter().any(|e| e.base == t.base) {
            continue;
        }
        out.push(VocabEntry {
            reading: if has_kanji(&t.base) { kana(&t.base) } else { None },
            gloss: lookup(&t.base, t.class).unwrap_or_default(),
            base: t.base.clone(),
            class: t.class,
        });
    }
    out
}

/// Short meaning for a word: the first sense, among the dictionary `entries`
/// for its spelling, whose part of speech fits `class`. Kana spellings are
/// shared by unrelated words (て is also 手, いく also 幾), so verbs, particles
/// and auxiliaries without a fitting sense get nothing rather than a wrong
/// meaning; nouns and adjectives fall back to the first sense.
pub fn pick_gloss(entries: &[dict::Entry], class: WordClass) -> Option<String> {
    let fits = |pos: &str| match class {
        WordClass::Verb => pos.starts_with('v') && pos != "vt" && pos != "vi",
        WordClass::Particle => pos == "prt" || pos == "conj",
        WordClass::Auxiliary => pos.starts_with("aux"),
        WordClass::Adjective => pos.starts_with("adj") || pos.starts_with("adv"),
        WordClass::Noun => pos.starts_with('n') || pos == "pn",
        WordClass::Other => false,
    };
    let senses = || entries.iter().flat_map(|e| &e.senses);
    let sense = senses().find(|s| s.pos.iter().any(|p| fits(p))).or_else(|| match class {
        WordClass::Verb | WordClass::Particle | WordClass::Auxiliary => None,
        _ => senses().next(),
    })?;
    let gloss = sense.glosses.iter().take(3).cloned().collect::<Vec<_>>().join("; ");
    Some(gloss).filter(|g| !g.is_empty())
}

/// Cache key of an entry: `(base, reading)`, with "" for no reading.
pub fn cache_key(e: &VocabEntry) -> (String, String) {
    (e.base.clone(), e.reading.clone().unwrap_or_default())
}

/// Replaces glosses found in `cached`; returns the entries and the indexes still to translate.
pub fn apply_cache(
    mut entries: Vec<VocabEntry>,
    cached: &HashMap<(String, String), String>,
) -> (Vec<VocabEntry>, Vec<usize>) {
    let mut missing = Vec::new();
    for (i, e) in entries.iter_mut().enumerate() {
        match cached.get(&cache_key(e)) {
            Some(g) => e.gloss = g.clone(),
            None => missing.push(i),
        }
    }
    (entries, missing)
}

/// Puts `translations[i]` into `entries[missing[i]]`. Empty translations keep the
/// original gloss and aren't returned for caching. Returns `(entries, rows to cache)`.
pub fn merge_translations(
    mut entries: Vec<VocabEntry>,
    missing: &[usize],
    translations: &[String],
) -> (Vec<VocabEntry>, Vec<(String, String, String)>) {
    let mut rows = Vec::new();
    for (&i, t) in missing.iter().zip(translations) {
        let t = t.trim();
        let Some(e) = entries.get_mut(i) else { continue };
        if t.is_empty() {
            continue;
        }
        e.gloss = t.to_string();
        let (base, reading) = cache_key(e);
        rows.push((base, reading, e.gloss.clone()));
    }
    (entries, rows)
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
        let v = vocabulary(&t, |_, _| None, |s| a.to_kana(s).ok());
        let bases: Vec<_> = v.iter().map(|e| e.base.as_str()).collect();
        assert_eq!(bases.first(), Some(&"見学"));
        assert!(bases.contains(&"する") && bases.contains(&"いく"), "{bases:?}");
        assert_eq!(bases.iter().filter(|b| **b == "て").count(), 1); // repeated, listed once
        assert!(!bases.contains(&"。"));
    }

    #[test]
    fn unknown_word_has_empty_gloss() {
        let a = Analyzer::new().unwrap();
        let v = vocabulary(&a.analyze("ねえ").unwrap(), |_, _| None, |s| a.to_kana(s).ok());
        assert!(v.iter().all(|e| e.gloss.is_empty()));
    }

    #[test]
    fn reading_only_with_kanji() {
        let a = Analyzer::new().unwrap();
        let v = vocabulary(&a.analyze("見学する").unwrap(), |_, _| Some("x".into()), |s| a.to_kana(s).ok());
        assert_eq!(v[0].reading.as_deref(), Some("けんがく"));
        assert_eq!(v[1].reading, None);
        assert_eq!(v[0].gloss, "x");
    }

    fn entry(base: &str, reading: Option<&str>, gloss: &str) -> VocabEntry {
        VocabEntry { base: base.into(), reading: reading.map(Into::into), class: WordClass::Noun, gloss: gloss.into() }
    }

    #[test]
    fn cached_entries_are_not_asked_again() {
        let entries = vec![entry("見学", Some("けんがく"), "field trip"), entry("ねえ", None, "")];
        let mut cached = std::collections::HashMap::new();
        cached.insert(("見学".to_string(), "けんがく".to_string()), "visita".to_string());
        let (filled, missing) = apply_cache(entries, &cached);
        assert_eq!(filled[0].gloss, "visita");
        assert_eq!(missing, vec![1]);

        cached.insert(("ねえ".to_string(), String::new()), "ei".to_string());
        let (_, missing) = apply_cache(filled, &cached);
        assert!(missing.is_empty(), "everything cached: the provider isn't called");
    }

    #[test]
    fn short_answer_keeps_the_english_and_caches_only_real_translations() {
        let entries = vec![entry("見学", Some("けんがく"), "field trip"), entry("する", None, "to do")];
        // The AI answered one item for two words (parse pads with "").
        let (out, rows) = merge_translations(entries, &[0, 1], &["visitar".into(), String::new()]);
        assert_eq!(out[0].gloss, "visitar");
        assert_eq!(out[1].gloss, "to do");
        assert_eq!(rows, vec![("見学".to_string(), "けんがく".to_string(), "visitar".to_string())]);
    }

    #[test]
    fn word_without_gloss_gets_translated() {
        let (out, rows) = merge_translations(vec![entry("ねえ", None, "")], &[0], &["ei".into()]);
        assert_eq!(out[0].gloss, "ei");
        assert_eq!(rows[0].1, "");
    }

    fn dict_entry(kana: &str, senses: &[(&[&str], &str)]) -> dict::Entry {
        dict::Entry {
            kanji: vec![],
            kana: vec![kana.into()],
            senses: senses
                .iter()
                .map(|(pos, g)| dict::Sense { pos: pos.iter().map(|p| p.to_string()).collect(), glosses: vec![g.to_string()] })
                .collect(),
        }
    }

    #[test]
    fn gloss_matches_the_word_class() {
        // て: 手 (noun) comes first in JMdict, the particle later.
        let te = [dict_entry("て", &[(&["n"], "hand")]), dict_entry("て", &[(&["prt"], "and then")])];
        assert_eq!(pick_gloss(&te, WordClass::Particle).as_deref(), Some("and then"));
        // いく: 幾 (prefix) before 行く (godan verb).
        let iku = [dict_entry("いく", &[(&["pref"], "some")]), dict_entry("いく", &[(&["v5k-s", "vi"], "to go")])];
        assert_eq!(pick_gloss(&iku, WordClass::Verb).as_deref(), Some("to go"));
    }

    #[test]
    fn no_matching_sense_is_empty_for_grammar_words() {
        let te = [dict_entry("て", &[(&["n"], "hand")])];
        assert_eq!(pick_gloss(&te, WordClass::Particle), None);
        assert_eq!(pick_gloss(&te, WordClass::Verb), None);
        // Nouns and adjectives fall back to the first sense (JMdict tags vary a lot).
        let dozo = [dict_entry("どうぞ", &[(&["adv"], "please")])];
        assert_eq!(pick_gloss(&dozo, WordClass::Noun).as_deref(), Some("please"));
    }

    #[test]
    fn punctuation_only_is_empty() {
        let a = Analyzer::new().unwrap();
        assert!(vocabulary(&a.analyze("。").unwrap(), |_, _| None, |_| None).is_empty());
        assert!(vocabulary(&a.analyze("").unwrap(), |_, _| None, |_| None).is_empty());
    }
}
