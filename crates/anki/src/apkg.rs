//! Writes cards to a standalone `.apkg` file (import with File > Import in Anki).
//!
//! An `.apkg` is a zip with `collection.anki2` (an Anki collection in the legacy
//! schema 11, which every Anki version imports), a `media` JSON map and the
//! media files, stored as `0`, `1`, `2`... Cards may carry their schedule and
//! review log, so studying continues in Anki where it stopped in the app.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;

use rusqlite::{params, Connection};
use serde_json::json;
use zip::write::SimpleFileOptions;

use crate::{Card, Result, BACK, CSS, FIELDS, FRONT, MODEL_ID, MODEL_NAME};

/// Card state, as Anki sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardKind {
    New,
    Learning,
    Review,
    Relearning,
}

/// One answer from the review log.
#[derive(Debug, Clone)]
pub struct Review {
    /// Unix milliseconds.
    pub time_ms: i64,
    /// 1 = Again ... 4 = Easy.
    pub ease: u8,
    /// State of the card before this answer.
    pub kind: CardKind,
    /// Time until the next due date that this answer set.
    pub interval_secs: i64,
    pub took_ms: i64,
}

/// Where a card stands in its study, plus how it got there.
#[derive(Debug, Clone)]
pub struct Schedule {
    pub kind: CardKind,
    /// Unix seconds (ignored for new cards).
    pub due: i64,
    pub last_review: Option<i64>,
    /// FSRS memory state. Anki with FSRS turned on continues from it.
    pub stability: Option<f64>,
    pub difficulty: Option<f64>,
    pub reps: i64,
    pub lapses: i64,
    pub reviews: Vec<Review>,
}

/// Deck ids must be stable per deck name so re-imports merge into the same deck.
fn deck_id(name: &str) -> i64 {
    (crate::fnv1a(name.as_bytes()) >> 1) as i64
}

const DAY: i64 = 86_400;
/// Retention the app schedules with (stored with the FSRS memory state).
const DESIRED_RETENTION: f64 = 0.9;

const SCHEMA: &str = "
CREATE TABLE col (id integer primary key, crt integer not null, mod integer not null, scm integer not null,
  ver integer not null, dty integer not null, usn integer not null, ls integer not null, conf text not null,
  models text not null, decks text not null, dconf text not null, tags text not null);
CREATE TABLE notes (id integer primary key, guid text not null, mid integer not null, mod integer not null,
  usn integer not null, tags text not null, flds text not null, sfld integer not null, csum integer not null,
  flags integer not null, data text not null);
CREATE TABLE cards (id integer primary key, nid integer not null, did integer not null, ord integer not null,
  mod integer not null, usn integer not null, type integer not null, queue integer not null, due integer not null,
  ivl integer not null, factor integer not null, reps integer not null, lapses integer not null, left integer not null,
  odue integer not null, odid integer not null, flags integer not null, data text not null);
CREATE TABLE revlog (id integer primary key, cid integer not null, usn integer not null, ease integer not null,
  ivl integer not null, lastIvl integer not null, factor integer not null, time integer not null, type integer not null);
CREATE TABLE graves (usn integer not null, oid integer not null, type integer not null);
CREATE INDEX ix_notes_usn on notes (usn);
CREATE INDEX ix_cards_usn on cards (usn);
CREATE INDEX ix_revlog_usn on revlog (usn);
CREATE INDEX ix_cards_nid on cards (nid);
CREATE INDEX ix_cards_sched on cards (did, queue, due);
CREATE INDEX ix_revlog_cid on revlog (cid);
CREATE INDEX ix_notes_csum on notes (csum);
";

/// Writes `cards` into deck `deck_name`.
///
/// `day_start` is the unix time when the current study day began (the app's
/// 4 AM rollover). Review due dates are stored as days counted from it.
pub fn write(path: &Path, deck_name: &str, cards: &[Card], tags: &[String], day_start: i64) -> Result<()> {
    // Built next to the output: the system temp dir isn't writable on Android.
    let db_path = path.with_extension("apkg-db.tmp");
    let _ = std::fs::remove_file(&db_path);
    let result = build(&db_path, path, deck_name, cards, tags, day_start);
    let _ = std::fs::remove_file(&db_path);
    result
}

fn build(db_path: &Path, path: &Path, deck_name: &str, cards: &[Card], tags: &[String], day_start: i64) -> Result<()> {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default();
    let now = now_ms / 1000;
    let did = deck_id(deck_name);

    let mut conn = Connection::open(db_path)?;
    conn.execute_batch(SCHEMA)?;
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO col VALUES (1, ?1, ?2, ?2, 11, 0, 0, 0, ?3, ?4, ?5, ?6, '{}')",
        params![
            day_start,
            now_ms,
            collection_conf(did).to_string(),
            json!({ MODEL_ID.to_string(): model(did, now) }).to_string(),
            json!({ "1": deck_json(1, "Default", now), did.to_string(): deck_json(did, deck_name, now) }).to_string(),
            json!({ "1": deck_options() }).to_string(),
        ],
    )?;

    // name -> zip entry number
    let mut media: BTreeMap<String, (usize, &[u8])> = BTreeMap::new();
    let tag_str = if tags.is_empty() { String::new() } else { format!(" {} ", tags.join(" ")) };
    for (i, card) in cards.iter().enumerate() {
        let sound = match &card.audio {
            Some(wav) => {
                let name = card.audio_filename();
                let n = media.len();
                media.entry(name.clone()).or_insert((n, wav));
                format!("[sound:{name}]")
            }
            None => String::new(),
        };
        let fields = card.fields(&sound);
        // Ids are timestamps in Anki; consecutive values keep them unique.
        let id = now_ms + i as i64;
        tx.execute(
            "INSERT INTO notes VALUES (?1, ?2, ?3, ?4, -1, ?5, ?6, ?7, ?8, 0, '')",
            params![
                id,
                // Same sentence -> same guid, so re-importing updates instead of duplicating.
                format!("{:x}", crate::fnv1a(card.sentence.as_bytes())),
                MODEL_ID,
                now,
                tag_str,
                fields.join("\x1f"),
                fields[0],
                checksum(&fields[0]),
            ],
        )?;
        insert_card(&tx, id, did, i as i64, now, day_start, card.schedule.as_ref())?;
    }
    tx.commit()?;
    drop(conn);

    let file = std::fs::File::create(path)?;
    let mut zip = zip::ZipWriter::new(file);
    let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    zip.start_file("collection.anki2", opts)?;
    zip.write_all(&std::fs::read(db_path)?)?;
    let names: BTreeMap<String, &str> = media.iter().map(|(name, (n, _))| (n.to_string(), name.as_str())).collect();
    zip.start_file("media", opts)?;
    zip.write_all(serde_json::to_string(&names)?.as_bytes())?;
    for (n, bytes) in media.values() {
        // WAV barely compresses; store it as is.
        zip.start_file(n.to_string(), SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored))?;
        zip.write_all(bytes)?;
    }
    zip.finish()?;
    Ok(())
}

fn insert_card(
    tx: &rusqlite::Transaction,
    id: i64,
    did: i64,
    position: i64,
    now: i64,
    day_start: i64,
    sched: Option<&Schedule>,
) -> Result<()> {
    let Some(s) = sched.filter(|s| s.kind != CardKind::New) else {
        // type 0 / queue 0: new, shown in `due` order.
        tx.execute(
            "INSERT INTO cards VALUES (?1, ?1, ?2, 0, ?3, -1, 0, 0, ?4, 0, 0, 0, 0, 0, 0, 0, 0, '')",
            params![id, did, now, position + 1],
        )?;
        insert_reviews(tx, id, sched)?;
        return Ok(());
    };
    let ivl_days = match (s.kind, s.last_review) {
        (CardKind::Review, Some(last)) => (((s.due - last) as f64 / DAY as f64).round() as i64).max(1),
        (CardKind::Review, None) => 1,
        _ => 0,
    };
    // Review cards are due on a day (counted from the collection's creation day);
    // (re)learning cards at a time of day.
    let (ctype, queue, due, left) = match s.kind {
        CardKind::Review => (2, 2, (s.due - day_start).div_euclid(DAY), 0),
        CardKind::Learning => (1, 1, s.due, 1001),
        CardKind::Relearning => (3, 1, s.due, 1001),
        CardKind::New => unreachable!(),
    };
    let factor = if s.kind == CardKind::Learning { 0 } else { 2500 };
    let mut data = serde_json::Map::new();
    if let (Some(st), Some(d)) = (s.stability, s.difficulty) {
        data.insert("s".into(), json!(round3(st)));
        data.insert("d".into(), json!(round3(d)));
        data.insert("dr".into(), json!(DESIRED_RETENTION));
    }
    if let Some(last) = s.last_review {
        data.insert("lrt".into(), json!(last));
    }
    let data = if data.is_empty() { String::new() } else { serde_json::Value::Object(data).to_string() };
    tx.execute(
        "INSERT INTO cards VALUES (?1, ?1, ?2, 0, ?3, -1, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 0, 0, 0, ?12)",
        params![id, did, now, ctype, queue, due, ivl_days, factor, s.reps, s.lapses, left, data],
    )?;
    insert_reviews(tx, id, sched)
}

fn insert_reviews(tx: &rusqlite::Transaction, cid: i64, sched: Option<&Schedule>) -> Result<()> {
    let mut last_ivl = 0;
    for r in sched.map(|s| s.reviews.as_slice()).unwrap_or_default() {
        // Positive = days, negative = seconds.
        let ivl = if r.interval_secs >= DAY { (r.interval_secs as f64 / DAY as f64).round() as i64 } else { -r.interval_secs };
        let (kind, factor) = match r.kind {
            CardKind::New | CardKind::Learning => (0, 0),
            CardKind::Review => (1, 2500),
            CardKind::Relearning => (2, 2500),
        };
        tx.execute(
            "INSERT OR IGNORE INTO revlog VALUES (?1, ?2, -1, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![r.time_ms, cid, r.ease, ivl, last_ivl, factor, r.took_ms.clamp(0, 60_000), kind],
        )?;
        last_ivl = ivl;
    }
    Ok(())
}

fn round3(x: f64) -> f64 {
    (x * 1000.0).round() / 1000.0
}

/// First 8 hex digits of the SHA-1 of the sort field, as Anki computes it for
/// duplicate checks.
fn checksum(field: &str) -> i64 {
    let digest = sha1_smol::Sha1::from(field).digest().bytes();
    u32::from_be_bytes([digest[0], digest[1], digest[2], digest[3]]) as i64
}

fn collection_conf(did: i64) -> serde_json::Value {
    json!({
        "activeDecks": [did], "curDeck": did, "newSpread": 0, "collapseTime": 1200, "timeLim": 0,
        "estTimes": true, "dueCounts": true, "curModel": MODEL_ID, "nextPos": 1,
        "sortType": "noteFld", "sortBackwards": false, "addToCur": true,
        "schedVer": 2, "rollover": 4, "creationOffset": 0,
    })
}

fn model(did: i64, now: i64) -> serde_json::Value {
    let flds: Vec<_> = FIELDS
        .iter()
        .enumerate()
        .map(|(ord, name)| json!({ "name": name, "ord": ord, "sticky": false, "rtl": false, "font": "Arial", "size": 20, "media": [] }))
        .collect();
    json!({
        "id": MODEL_ID, "name": MODEL_NAME, "type": 0, "mod": now, "usn": -1, "sortf": 0, "did": did,
        "tmpls": [{ "name": "Leitura", "ord": 0, "qfmt": FRONT, "afmt": BACK, "bqfmt": "", "bafmt": "",
                    "did": null, "bfont": "", "bsize": 0 }],
        "flds": flds, "css": CSS,
        "latexPre": "\\documentclass[12pt]{article}\n\\special{papersize=3in,5in}\n\\usepackage[utf8]{inputenc}\n\\usepackage{amssymb,amsmath}\n\\pagestyle{empty}\n\\setlength{\\parindent}{0in}\n\\begin{document}\n",
        "latexPost": "\\end{document}", "latexsvg": false,
        // The front uses Word and Sentence.
        "req": [[0, "any", [0, 1]]],
        "tags": [], "vers": [],
    })
}

fn deck_json(id: i64, name: &str, now: i64) -> serde_json::Value {
    json!({
        "id": id, "name": name, "mod": now, "usn": -1, "desc": if id == 1 { "" } else { "Gerado pelo Japones App" },
        "lrnToday": [0, 0], "revToday": [0, 0], "newToday": [0, 0], "timeToday": [0, 0],
        "collapsed": false, "browserCollapsed": false, "dyn": 0, "conf": 1, "extendNew": 0, "extendRev": 0,
    })
}

fn deck_options() -> serde_json::Value {
    json!({
        "id": 1, "name": "Default", "mod": 0, "usn": 0, "maxTaken": 60, "autoplay": true, "timer": 0,
        "replayq": true, "dyn": false,
        "new": { "delays": [1.0, 10.0], "ints": [1, 4, 0], "initialFactor": 2500, "order": 1, "perDay": 20, "bury": false },
        "lapse": { "delays": [10.0], "mult": 0.0, "minInt": 1, "leechFails": 8, "leechAction": 1 },
        "rev": { "perDay": 200, "ease4": 1.3, "ivlFct": 1.0, "maxIvl": 36500, "bury": false, "hardFactor": 1.2 },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    fn card(sentence: &str, schedule: Option<Schedule>) -> Card {
        Card {
            word: "食べる".into(),
            sentence: sentence.into(),
            sentence_furigana: " 寿司[すし]を 食[た]べた。".into(),
            translation: "Comi sushi.".into(),
            note: String::new(),
            audio: Some(b"RIFF....WAVE".to_vec()),
            audio_credit: Some("VOICEVOX:ずんだもん".into()),
            schedule,
        }
    }

    #[test]
    fn writes_collection_media_and_history() {
        let dir = std::env::temp_dir().join(format!("apkg-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("t.apkg");
        let day_start = 1_800_000_000;
        let review = Schedule {
            kind: CardKind::Review,
            due: day_start + 5 * DAY + 3600,
            last_review: Some(day_start - DAY),
            stability: Some(6.1234),
            difficulty: Some(5.0),
            reps: 3,
            lapses: 0,
            reviews: vec![
                Review { time_ms: 1_799_000_000_000, ease: 3, kind: CardKind::New, interval_secs: 600, took_ms: 4000 },
                Review { time_ms: 1_799_000_700_000, ease: 3, kind: CardKind::Learning, interval_secs: 3 * DAY, took_ms: 3000 },
            ],
        };
        let audio = &card("寿司を食べた。", None).fields("[sound:a.wav]")[5];
        assert_eq!(audio, r#"[sound:a.wav]<div class="credit">VOICEVOX:ずんだもん</div>"#);
        write(&path, "Japonês::Teste", &[card("寿司を食べた。", Some(review)), card("パンを食べる。", None)], &["japones-app".into()], day_start)
            .unwrap();

        let mut zip = zip::ZipArchive::new(std::fs::File::open(&path).unwrap()).unwrap();
        let mut media = String::new();
        zip.by_name("media").unwrap().read_to_string(&mut media).unwrap();
        let media: BTreeMap<String, String> = serde_json::from_str(&media).unwrap();
        assert_eq!(media.len(), 2);
        assert_eq!(zip.by_name("0").unwrap().size(), 12);
        let db = dir.join("check.anki2");
        std::io::copy(&mut zip.by_name("collection.anki2").unwrap(), &mut std::fs::File::create(&db).unwrap()).unwrap();
        assert!(!dir.join("t.apkg-db.tmp").exists());

        let conn = Connection::open(&db).unwrap();
        let (ctype, queue, due, ivl, data): (i64, i64, i64, i64, String) = conn
            .query_row("SELECT type, queue, due, ivl, data FROM cards ORDER BY id LIMIT 1", [], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
            })
            .unwrap();
        assert_eq!((ctype, queue, due, ivl), (2, 2, 5, 6));
        assert!(data.contains("\"s\":6.123"), "{data}");
        let (new_type, new_due): (i64, i64) =
            conn.query_row("SELECT type, due FROM cards ORDER BY id DESC LIMIT 1", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        assert_eq!((new_type, new_due), (0, 2));
        let log: Vec<(i64, i64, i64)> = conn
            .prepare("SELECT ivl, lastIvl, type FROM revlog ORDER BY id")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert_eq!(log, vec![(-600, 0, 0), (3, -600, 0)]);
        let (flds, csum): (String, i64) =
            conn.query_row("SELECT flds, csum FROM notes ORDER BY id LIMIT 1", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        assert!(flds.starts_with("食べる\x1f寿司を食べた。"));
        assert_eq!(csum, checksum("食べる"));
        drop(conn);
        let _ = std::fs::remove_dir_all(dir);
    }
}
