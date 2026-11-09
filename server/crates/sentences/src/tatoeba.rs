//! Downloads the per-language Tatoeba exports and loads them into a [`Store`].

use std::io::{BufRead, BufReader, Read};

use bzip2::read::BzDecoder;
use jp_core::Analyzer;

use crate::{Result, Store};

const BASE: &str = "https://downloads.tatoeba.org/exports/per_language";

async fn fetch_bz2(url: &str) -> Result<Vec<String>> {
    let bytes = reqwest::get(url).await?.error_for_status()?.bytes().await?;
    let mut text = String::new();
    BzDecoder::new(&bytes[..]).read_to_string(&mut text)?;
    Ok(text.lines().map(str::to_owned).collect())
}

/// Parses `id \t lang \t text` lines.
pub fn parse_sentences(lines: impl IntoIterator<Item = String>) -> impl Iterator<Item = (i64, String, String)> {
    lines.into_iter().filter_map(|l| {
        let mut f = l.splitn(3, '\t');
        Some((f.next()?.parse().ok()?, f.next()?.to_string(), f.next()?.to_string()))
    })
}

/// Parses `id_a \t id_b` lines.
pub fn parse_links(lines: impl IntoIterator<Item = String>) -> impl Iterator<Item = (i64, i64)> {
    lines.into_iter().filter_map(|l| {
        let mut f = l.split('\t');
        Some((f.next()?.parse().ok()?, f.next()?.parse().ok()?))
    })
}

/// Reads a local `.tsv` or `.tsv.bz2` file.
pub fn read_lines(path: &std::path::Path) -> Result<Vec<String>> {
    let file = std::fs::File::open(path)?;
    let reader: Box<dyn Read> = if path.extension().is_some_and(|e| e == "bz2") {
        Box::new(BzDecoder::new(file))
    } else {
        Box::new(file)
    };
    Ok(BufReader::new(reader).lines().collect::<std::io::Result<_>>()?)
}

/// Downloads Japanese sentences plus translations in `langs` (ISO 639-3,
/// e.g. "por", "eng") and builds the lemma index.
/// `log` receives human-readable progress messages.
pub async fn download_and_import(
    store: &std::sync::Mutex<Store>,
    analyzer: &Analyzer,
    langs: &[String],
    log: impl Fn(String),
) -> Result<()> {
    log("Baixando frases em japonês...".into());
    let jpn = fetch_bz2(&format!("{BASE}/jpn/jpn_sentences.tsv.bz2")).await?;
    let n = store.lock().unwrap().insert_sentences(parse_sentences(jpn), None)?;
    log(format!("{n} frases em japonês importadas."));

    for lang in langs {
        log(format!("Baixando links jpn-{lang}..."));
        let links = fetch_bz2(&format!("{BASE}/jpn/jpn-{lang}_links.tsv.bz2")).await?;
        let targets = store.lock().unwrap().insert_links(parse_links(links))?;

        log(format!("Baixando frases em {lang} ({} traduções ligadas)...", targets.len()));
        let trans = fetch_bz2(&format!("{BASE}/{lang}/{lang}_sentences.tsv.bz2")).await?;
        let n = store.lock().unwrap().insert_sentences(parse_sentences(trans), Some(&targets))?;
        log(format!("{n} traduções em {lang} importadas."));
    }

    log("Indexando lemas (pode levar alguns minutos)...".into());
    store.lock().unwrap().build_index(analyzer, |done, total| {
        log(format!("Indexando: {done}/{total}"));
    })?;
    log("Pronto.".into());
    Ok(())
}
