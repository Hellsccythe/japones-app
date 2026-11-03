//! Reads notes out of an Anki `.apkg`. Notes of this app's own note type keep
//! all their fields; other note types use the first field as the sentence and
//! the second as the translation. Media and Anki's scheduling aren't imported.

use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

use rusqlite::Connection;
use serde::Deserialize;

use crate::{Error, NoteInput, Result};

pub type ImportedNote = NoteInput;

#[derive(Deserialize)]
struct Model {
    flds: Vec<Field>,
}

#[derive(Deserialize)]
struct Field {
    name: String,
}

pub fn import_apkg(path: &Path) -> Result<Vec<ImportedNote>> {
    let file = std::fs::File::open(path)?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| Error::Invalid(format!("não é um .apkg válido: {e}")))?;
    // anki21 is newer than anki2 when both are present (the anki2 then only holds a notice).
    let name = ["collection.anki21", "collection.anki2"]
        .into_iter()
        .find(|n| zip.by_name(n).is_ok())
        .ok_or_else(|| {
            Error::Invalid(if zip.by_name("collection.anki21b").is_ok() {
                "este .apkg usa o formato novo do Anki. No Anki, exporte de novo marcando \
                 “Suporte a versões antigas do Anki”."
                    .into()
            } else {
                "o .apkg não tem coleção do Anki".into()
            })
        })?;

    let tmp = std::env::temp_dir().join(format!("japones-import-{}.db", std::process::id()));
    {
        let mut entry = zip.by_name(name).map_err(|e| Error::Invalid(e.to_string()))?;
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes)?;
        std::fs::write(&tmp, bytes)?;
    }
    let result = read_collection(&tmp);
    let _ = std::fs::remove_file(&tmp);
    result
}

fn read_collection(path: &Path) -> Result<Vec<ImportedNote>> {
    let conn = Connection::open(path)?;
    let models_json: String = conn.query_row("SELECT models FROM col", [], |r| r.get(0))?;
    let models: HashMap<String, Model> =
        serde_json::from_str(&models_json).map_err(|e| Error::Invalid(format!("tipos de nota ilegíveis: {e}")))?;

    let mut stmt = conn.prepare("SELECT mid, flds FROM notes ORDER BY id")?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
    let mut out = Vec::new();
    for row in rows {
        let (mid, flds) = row?;
        let values: Vec<&str> = flds.split('\x1f').collect();
        let names: Vec<&str> = models
            .get(&mid.to_string())
            .map(|m| m.flds.iter().map(|f| f.name.as_str()).collect())
            .unwrap_or_default();
        let field = |name: &str| names.iter().position(|n| *n == name).and_then(|i| values.get(i)).map(|v| clean(v));
        let note = if field("Sentence").is_some() {
            NoteInput {
                word: field("Word").unwrap_or_default(),
                sentence: field("Sentence").unwrap_or_default(),
                translation: field("Translation").unwrap_or_default(),
                note: field("Note").unwrap_or_default(),
            }
        } else {
            NoteInput {
                word: String::new(),
                sentence: values.first().map(|v| clean(v)).unwrap_or_default(),
                translation: values.get(1).map(|v| clean(v)).unwrap_or_default(),
                note: String::new(),
            }
        };
        if !note.sentence.is_empty() {
            out.push(note);
        }
    }
    Ok(out)
}

/// Field text without HTML tags, sound tags or furigana brackets.
fn clean(field: &str) -> String {
    let mut out = String::with_capacity(field.len());
    let mut in_tag = false;
    for c in field.replace("<br>", " ").replace("&nbsp;", " ").chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    // [sound:x.mp3] and Anki furigana readings like 食[た]べる.
    let mut s = String::with_capacity(out.len());
    let mut depth = 0;
    for c in out.chars() {
        match c {
            '[' => depth += 1,
            ']' if depth > 0 => depth -= 1,
            _ if depth == 0 => s.push(c),
            _ => {}
        }
    }
    s.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::clean;

    #[test]
    fn cleans_html_sound_and_readings() {
        assert_eq!(clean("<b>食[た]べる</b>[sound:a.mp3]<br>ok &amp; done"), "食べる ok & done");
    }
}
