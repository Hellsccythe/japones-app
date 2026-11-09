//! PDF export with furigana (ruby text), typeset with Typst.
//! Japanese text uses an embedded subset of Noto Serif JP (SIL OFL 1.1, see
//! `fonts/OFL.txt` and `scripts/fonts/subset.py`), so PDFs look the same on
//! Windows and Android. Characters outside the subset fall back to system fonts.

use jp_core::Segment;
use serde::{Deserialize, Serialize};
use typst_layout::PagedDocument;
use typst_as_lib::typst_kit_options::TypstKitFontOptions;
use typst_as_lib::TypstEngine;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("typst: {0}")]
    Typst(String),
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub segments: Vec<Segment>,
    #[serde(default)]
    pub translation: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Section {
    pub heading: String,
    pub entries: Vec<Entry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub title: String,
    pub sections: Vec<Section>,
    /// When false, the Japanese is printed without readings (for self-testing).
    #[serde(default = "yes")]
    pub furigana: bool,
}

fn yes() -> bool {
    true
}

const PREAMBLE: &str = r#"
#set document(title: TITLE)
#set page(paper: "a4", margin: (x: 2cm, y: 2.2cm), numbering: "1")
#set text(font: ("Noto Serif JP", "Noto Serif CJK JP", "Yu Mincho", "MS Mincho", "Noto Sans CJK JP", "Yu Gothic"), lang: "ja", size: 11pt)
#set par(leading: 1.5em)
#show heading: set block(below: 1.2em)

// Ruby: the reading is placed above the base text without changing its width.
#let rb(base, rt) = if rt == none or not FURIGANA { base } else {
  box[#base#place(top + center, dy: -0.82em, text(size: 0.5em, rt))]
}
#let jp(segs) = text(size: 16pt, segs.map(((b, r)) => rb(b, r)).join())
#let entry(n, segs, tr, note) = block(above: 2.4em, breakable: false, grid(
  columns: (1.6em, 1fr), row-gutter: 0.5em,
  text(fill: luma(140), str(n) + "."), jp(segs),
  [], if tr != none { text(fill: luma(70), font: ("Segoe UI", "Roboto", "Noto Sans", "Noto Serif JP"), tr) },
  [], if note != none { text(size: 9pt, fill: luma(120), font: ("Segoe UI", "Roboto", "Noto Sans", "Noto Serif JP"), note) },
))

#align(center, text(size: 22pt, weight: "bold", TITLE))
"#;

/// Typst string literal.
fn lit(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn opt(s: &Option<String>) -> String {
    s.as_deref().filter(|s| !s.is_empty()).map(lit).unwrap_or_else(|| "none".into())
}

/// Builds the Typst source for `doc`. Kept separate from compilation so it can be tested.
pub fn source(doc: &Document) -> String {
    let mut src = PREAMBLE
        .replace("TITLE", &lit(&doc.title))
        .replace("FURIGANA", if doc.furigana { "true" } else { "false" });

    for section in &doc.sections {
        src.push_str(&format!("\n= #{}\n", lit(&section.heading)));
        for (i, e) in section.entries.iter().enumerate() {
            let segs: Vec<String> = e
                .segments
                .iter()
                .map(|s| format!("({}, {})", lit(&s.text), opt(&s.reading)))
                .collect();
            // Trailing comma keeps a one-element array an array in Typst.
            src.push_str(&format!(
                "#entry({}, ({},), {}, {})\n",
                i + 1,
                segs.join(", "),
                opt(&e.translation),
                opt(&e.note)
            ));
        }
    }
    src
}

static NOTO_SERIF_JP: [&[u8]; 2] = [
    include_bytes!("../fonts/NotoSerifJP-Regular.otf"),
    include_bytes!("../fonts/NotoSerifJP-Bold.otf"),
];

/// Lays out `doc` (shared by [`render`] and the PNG preview example).
pub fn compile(doc: &Document) -> Result<PagedDocument> {
    let engine = TypstEngine::builder()
        .main_file(source(doc))
        .fonts(NOTO_SERIF_JP)
        .search_fonts_with(TypstKitFontOptions::default().include_system_fonts(true))
        .build();
    engine.compile().output.map_err(|e| Error::Typst(format!("{e:?}")))
}

/// Renders `doc` to PDF bytes.
pub fn render(doc: &Document) -> Result<Vec<u8>> {
    typst_pdf::pdf(&compile(doc)?, &Default::default()).map_err(|e| Error::Typst(format!("{e:?}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Document {
        Document {
            title: "食べる".into(),
            furigana: true,
            sections: vec![Section {
                heading: "食べる \"taberu\"".into(),
                entries: vec![Entry {
                    segments: vec![
                        Segment { text: "寿司".into(), reading: Some("すし".into()) },
                        Segment { text: "を".into(), reading: None },
                        Segment { text: "食".into(), reading: Some("た".into()) },
                        Segment { text: "べた。".into(), reading: None },
                    ],
                    translation: Some("Comi sushi.".into()),
                    note: Some("Passado simples (た形).".into()),
                }],
            }],
        }
    }

    #[test]
    fn escapes_strings() {
        assert!(source(&sample()).contains(r#""食べる \"taberu\"""#));
    }

    #[test]
    fn renders_pdf() {
        let pdf = render(&sample()).unwrap();
        assert!(pdf.starts_with(b"%PDF"));
    }
}
