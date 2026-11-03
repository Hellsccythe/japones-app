//! Renders the first page of a sample document to PNG, to eyeball the ruby layout:
//! cargo run -p pdf --example preview -- out.png
use jp_core::Analyzer;
use pdf::{Document, Entry, Section};

fn main() {
    let a = Analyzer::new().unwrap();
    let lines = ["料理がおいしくてつい食べ過ぎた。", "明日、学校へ行きますか。", "お腹が空いたので、ご飯を食べに行こう。"];
    let doc = Document {
        title: "食べる・行く".into(),
        furigana: true,
        sections: vec![Section {
            heading: "食べる".into(),
            entries: lines
                .iter()
                .map(|l| Entry { segments: a.furigana(l).unwrap(), translation: Some("Tradução de exemplo.".into()), note: Some("Nota de gramática.".into()) })
                .collect(),
        }],
    };
    let compiled = pdf::compile(&doc).unwrap();
    let pixmap = typst_render::render(&compiled.pages()[0], &typst_render::RenderOptions { pixel_per_pt: 1.5.into(), ..Default::default() });
    let out = std::env::args().nth(1).unwrap_or("preview.png".into());
    pixmap.save_png(out).unwrap();
}
