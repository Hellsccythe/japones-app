use serde::{Deserialize, Serialize};
use wana_kana::ConvertJapanese;

/// A run of text, optionally with a reading to show above it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Segment {
    pub text: String,
    pub reading: Option<String>,
}

impl Segment {
    pub fn plain(text: &str) -> Self {
        Self { text: text.to_string(), reading: None }
    }

    fn ruby(text: &str, reading: &str) -> Self {
        Self { text: text.to_string(), reading: Some(reading.to_string()) }
    }
}

pub fn is_kanji(c: char) -> bool {
    matches!(c,
        '\u{4E00}'..='\u{9FFF}'   // CJK unified ideographs
        | '\u{3400}'..='\u{4DBF}' // extension A
        | '\u{F900}'..='\u{FAFF}' // compatibility ideographs
        | '々' | 'ヶ' | '〆')
}

pub fn has_kanji(s: &str) -> bool {
    s.chars().any(is_kanji)
}

/// Aligns a hiragana `reading` to `surface`, giving each kanji run only its
/// own part of the reading: (食べる, たべる) -> [食(た), べる].
/// Falls back to the whole reading over the whole word when the kana in the
/// surface can't be matched against the reading.
pub fn align(surface: &str, reading: &str) -> Vec<Segment> {
    // Split into alternating runs of kanji / non-kanji.
    let mut runs: Vec<(bool, String)> = Vec::new();
    for c in surface.chars() {
        let k = is_kanji(c);
        match runs.last_mut() {
            Some((kind, s)) if *kind == k => s.push(c),
            _ => runs.push((k, c.to_string())),
        }
    }

    let reading: Vec<char> = reading.chars().collect();
    let mut assigned = Vec::new();
    if solve(&runs, 0, &reading, 0, &mut assigned) {
        runs.iter()
            .zip(assigned)
            .map(|((kanji, text), r)| if *kanji { Segment::ruby(text, &r) } else { Segment::plain(text) })
            .collect()
    } else {
        vec![Segment::ruby(surface, &reading.iter().collect::<String>())]
    }
}

/// Backtracking: kana runs must match the reading literally, kanji runs take
/// at least one reading character each.
fn solve(runs: &[(bool, String)], i: usize, reading: &[char], pos: usize, out: &mut Vec<String>) -> bool {
    if i == runs.len() {
        return pos == reading.len();
    }
    let (kanji, text) = &runs[i];
    if !kanji {
        let want: Vec<char> = text.to_hiragana().chars().collect();
        if reading[pos..].starts_with(&want) {
            out.push(text.clone());
            if solve(runs, i + 1, reading, pos + want.len(), out) {
                return true;
            }
            out.pop();
        }
        return false;
    }
    for end in (pos + 1)..=reading.len() {
        out.push(reading[pos..end].iter().collect());
        if solve(runs, i + 1, reading, end, out) {
            return true;
        }
        out.pop();
    }
    false
}

/// Anki furigana syntax. The leading space marks where the base text starts;
/// Anki hides it when rendering.
pub fn render_anki(segs: &[Segment]) -> String {
    segs.iter()
        .map(|s| match &s.reading {
            Some(r) => format!(" {}[{}]", s.text, r),
            None => s.text.clone(),
        })
        .collect()
}

pub fn render_html(segs: &[Segment]) -> String {
    segs.iter()
        .map(|s| match &s.reading {
            Some(r) => format!("<ruby>{}<rt>{}</rt></ruby>", escape(&s.text), escape(r)),
            None => escape(&s.text),
        })
        .collect()
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn show(segs: &[Segment]) -> String {
        render_anki(segs)
    }

    #[test]
    fn aligns_okurigana() {
        assert_eq!(show(&align("食べる", "たべる")), " 食[た]べる");
        assert_eq!(show(&align("お茶", "おちゃ")), "お 茶[ちゃ]");
        assert_eq!(show(&align("取り扱い", "とりあつかい")), " 取[と]り 扱[あつか]い");
    }

    #[test]
    fn whole_word_when_all_kanji() {
        assert_eq!(show(&align("今日", "きょう")), " 今日[きょう]");
    }

    #[test]
    fn falls_back_when_kana_mismatch() {
        // Reading doesn't contain the surface kana: give up gracefully.
        assert_eq!(show(&align("行く", "ゆかない")), " 行く[ゆかない]");
    }

    #[test]
    fn katakana_in_surface_matches_hiragana_reading() {
        assert_eq!(show(&align("ヶ月", "かげつ")), " ヶ月[かげつ]");
    }
}
