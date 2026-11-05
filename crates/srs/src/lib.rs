//! Flashcard study inside the app: decks, one card per sentence, FSRS
//! scheduling and a review log, in its own SQLite file (`study.db`) so it can
//! be backed up and moved between devices on its own.

mod import;
pub mod schedule;

use std::path::Path;

use chrono::{Duration, Local, TimeZone};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

pub use import::{import_apkg, ImportedNote};
pub use schedule::{Rating, Sched, State};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("banco de estudo: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("FSRS: {0}")]
    Fsrs(#[from] fsrs::FSRSError),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Invalid(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Probability of remembering a card when it comes due.
pub const DESIRED_RETENTION: f32 = 0.9;
/// The study day starts at 4 a.m. local time, like Anki.
const DAY_START_HOUR: i64 = 4;
/// When nothing else is due, learning cards due this soon are shown early.
const LEARN_AHEAD: i64 = 20 * 60;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS decks (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL UNIQUE,
  new_per_day INTEGER NOT NULL DEFAULT 20,
  reviews_per_day INTEGER NOT NULL DEFAULT 200
);
CREATE TABLE IF NOT EXISTS notes (
  id INTEGER PRIMARY KEY,
  deck_id INTEGER NOT NULL REFERENCES decks(id) ON DELETE CASCADE,
  word TEXT NOT NULL DEFAULT '',
  sentence TEXT NOT NULL,
  translation TEXT NOT NULL DEFAULT '',
  note TEXT NOT NULL DEFAULT '',
  created INTEGER NOT NULL,
  UNIQUE (deck_id, sentence)
);
CREATE TABLE IF NOT EXISTS cards (
  id INTEGER PRIMARY KEY,
  note_id INTEGER NOT NULL UNIQUE REFERENCES notes(id) ON DELETE CASCADE,
  state INTEGER NOT NULL DEFAULT 0,
  due INTEGER NOT NULL,
  stability REAL,
  difficulty REAL,
  last_review INTEGER,
  reps INTEGER NOT NULL DEFAULT 0,
  lapses INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS cards_due ON cards(state, due);
CREATE TABLE IF NOT EXISTS revlog (
  id INTEGER PRIMARY KEY,          -- unix milliseconds
  card_id INTEGER NOT NULL REFERENCES cards(id) ON DELETE CASCADE,
  rating INTEGER NOT NULL,
  state_before INTEGER NOT NULL,
  interval_secs INTEGER NOT NULL,
  stability REAL,
  difficulty REAL,
  took_ms INTEGER NOT NULL
);
";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Deck {
    pub id: i64,
    pub name: String,
    pub new_per_day: i64,
    pub reviews_per_day: i64,
}

/// Cards left to study today, as shown on the deck list.
#[derive(Debug, Clone, Copy, Default, Serialize)]
pub struct Counts {
    pub new: i64,
    pub learning: i64,
    pub review: i64,
    /// All cards in the deck.
    pub total: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeckView {
    #[serde(flatten)]
    pub deck: Deck,
    pub counts: Counts,
}

/// A note's content, for adding and exporting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteInput {
    #[serde(default)]
    pub word: String,
    pub sentence: String,
    #[serde(default)]
    pub translation: String,
    #[serde(default)]
    pub note: String,
}

/// The card to study now.
#[derive(Debug, Clone, Serialize)]
pub struct StudyCard {
    pub card_id: i64,
    pub state: State,
    #[serde(flatten)]
    pub note: NoteInput,
    /// Seconds until it comes back for Again, Hard, Good, Easy.
    pub intervals: [i64; 4],
}

/// One answer from the review log, for exporting.
#[derive(Debug, Clone)]
pub struct LoggedReview {
    /// Unix milliseconds (also the revlog id).
    pub time_ms: i64,
    pub rating: u8,
    pub state_before: State,
    /// Time until the next due date that this answer set.
    pub interval_secs: i64,
    pub took_ms: i64,
}

/// A note with its card's schedule and review log, for exporting to Anki.
#[derive(Debug, Clone)]
pub struct ExportedNote {
    pub note: NoteInput,
    pub state: State,
    /// Unix seconds (for new cards: the order they were added in).
    pub due: i64,
    pub stability: Option<f64>,
    pub difficulty: Option<f64>,
    pub last_review: Option<i64>,
    pub reps: i64,
    pub lapses: i64,
    pub reviews: Vec<LoggedReview>,
}

pub struct Srs {
    conn: Connection,
}

fn now_secs() -> i64 {
    chrono::Utc::now().timestamp()
}

/// Start of the study day containing `now` (4 a.m. local time).
pub fn day_start(now: i64) -> i64 {
    let local = Local.timestamp_opt(now, 0).single().unwrap_or_else(Local::now);
    let date = (local - Duration::hours(DAY_START_HOUR)).date_naive();
    let start = date.and_hms_opt(DAY_START_HOUR as u32, 0, 0).unwrap();
    Local
        .from_local_datetime(&start)
        .earliest()
        .map_or(now - (now % 86_400), |t| t.timestamp())
}

impl Srs {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;")?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self { conn })
    }

    // ---- decks ----

    pub fn decks(&self) -> Result<Vec<DeckView>> {
        let mut stmt = self.conn.prepare("SELECT id, name, new_per_day, reviews_per_day FROM decks ORDER BY name")?;
        let decks = stmt
            .query_map([], |r| {
                Ok(Deck { id: r.get(0)?, name: r.get(1)?, new_per_day: r.get(2)?, reviews_per_day: r.get(3)? })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let now = now_secs();
        decks.into_iter().map(|d| Ok(DeckView { counts: self.counts_at(&d, now)?, deck: d })).collect()
    }

    /// The deck called `name`, created if missing.
    pub fn deck_id(&self, name: &str) -> Result<i64> {
        let name = name.trim();
        if name.is_empty() {
            return Err(Error::Invalid("dê um nome ao deck".into()));
        }
        self.conn.execute("INSERT OR IGNORE INTO decks (name) VALUES (?1)", [name])?;
        Ok(self.conn.query_row("SELECT id FROM decks WHERE name = ?1", [name], |r| r.get(0))?)
    }

    pub fn deck(&self, id: i64) -> Result<Deck> {
        self.conn
            .query_row("SELECT id, name, new_per_day, reviews_per_day FROM decks WHERE id = ?1", [id], |r| {
                Ok(Deck { id: r.get(0)?, name: r.get(1)?, new_per_day: r.get(2)?, reviews_per_day: r.get(3)? })
            })
            .optional()?
            .ok_or_else(|| Error::Invalid("deck não encontrado".into()))
    }

    pub fn update_deck(&self, d: &Deck) -> Result<()> {
        self.conn.execute(
            "UPDATE decks SET name = ?2, new_per_day = ?3, reviews_per_day = ?4 WHERE id = ?1",
            params![d.id, d.name.trim(), d.new_per_day.max(0), d.reviews_per_day.max(0)],
        )?;
        Ok(())
    }

    pub fn delete_deck(&self, id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM decks WHERE id = ?1", [id])?;
        Ok(())
    }

    // ---- notes ----

    /// Adds notes (one card each) to a deck. Sentences already in the deck
    /// are skipped. Returns how many were added.
    pub fn add_notes(&mut self, deck_id: i64, notes: &[NoteInput]) -> Result<usize> {
        let now = now_secs();
        let tx = self.conn.transaction()?;
        let mut added = 0;
        for n in notes {
            let sentence = n.sentence.trim();
            if sentence.is_empty() {
                continue;
            }
            let inserted = tx.execute(
                "INSERT OR IGNORE INTO notes (deck_id, word, sentence, translation, note, created)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![deck_id, n.word, sentence, n.translation, n.note, now],
            )?;
            if inserted == 1 {
                tx.execute("INSERT INTO cards (note_id, due) VALUES (?1, ?2)", params![tx.last_insert_rowid(), now])?;
                added += 1;
            }
        }
        tx.commit()?;
        Ok(added)
    }

    pub fn notes(&self, deck_id: i64) -> Result<Vec<NoteInput>> {
        let mut stmt =
            self.conn.prepare("SELECT word, sentence, translation, note FROM notes WHERE deck_id = ?1 ORDER BY id")?;
        let notes = stmt
            .query_map([deck_id], |r| {
                Ok(NoteInput { word: r.get(0)?, sentence: r.get(1)?, translation: r.get(2)?, note: r.get(3)? })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(notes)
    }

    /// Every note of a deck with its schedule and review history, oldest first.
    pub fn export(&self, deck_id: i64) -> Result<Vec<ExportedNote>> {
        let mut stmt = self.conn.prepare(
            "SELECT c.id, n.word, n.sentence, n.translation, n.note, c.state, c.due, c.stability, c.difficulty,
                    c.last_review, c.reps, c.lapses
             FROM notes n JOIN cards c ON c.note_id = n.id WHERE n.deck_id = ?1 ORDER BY n.id",
        )?;
        let rows = stmt
            .query_map([deck_id], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    ExportedNote {
                        note: NoteInput { word: r.get(1)?, sentence: r.get(2)?, translation: r.get(3)?, note: r.get(4)? },
                        state: State::from_i64(r.get(5)?),
                        due: r.get(6)?,
                        stability: r.get(7)?,
                        difficulty: r.get(8)?,
                        last_review: r.get(9)?,
                        reps: r.get(10)?,
                        lapses: r.get(11)?,
                        reviews: Vec::new(),
                    },
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let mut log = self.conn.prepare(
            "SELECT id, rating, state_before, interval_secs, took_ms FROM revlog WHERE card_id = ?1 ORDER BY id",
        )?;
        rows.into_iter()
            .map(|(card_id, mut n)| {
                n.reviews = log
                    .query_map([card_id], |r| {
                        Ok(LoggedReview {
                            time_ms: r.get(0)?,
                            rating: r.get::<_, i64>(1)? as u8,
                            state_before: State::from_i64(r.get(2)?),
                            interval_secs: r.get(3)?,
                            took_ms: r.get(4)?,
                        })
                    })?
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                Ok(n)
            })
            .collect()
    }

    // ---- studying ----

    pub fn counts(&self, deck_id: i64) -> Result<Counts> {
        self.counts_at(&self.deck(deck_id)?, now_secs())
    }

    /// (new cards introduced, reviews done) today in a deck.
    fn done_today(&self, deck_id: i64, now: i64) -> Result<(i64, i64)> {
        let since_ms = day_start(now) * 1000;
        Ok(self.conn.query_row(
            "SELECT count(DISTINCT CASE WHEN r.state_before = 0 THEN r.card_id END),
                    count(CASE WHEN r.state_before = 2 THEN 1 END)
             FROM revlog r JOIN cards c ON c.id = r.card_id JOIN notes n ON n.id = c.note_id
             WHERE n.deck_id = ?1 AND r.id >= ?2",
            params![deck_id, since_ms],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?)
    }

    fn counts_at(&self, deck: &Deck, now: i64) -> Result<Counts> {
        let day_end = day_start(now) + 86_400;
        let (new_done, reviews_done) = self.done_today(deck.id, now)?;
        let (new, learning, review, total): (i64, i64, i64, i64) = self.conn.query_row(
            "SELECT count(CASE WHEN c.state = 0 THEN 1 END),
                    count(CASE WHEN c.state IN (1, 3) AND c.due < ?2 THEN 1 END),
                    count(CASE WHEN c.state = 2 AND c.due < ?2 THEN 1 END),
                    count(*)
             FROM cards c JOIN notes n ON n.id = c.note_id WHERE n.deck_id = ?1",
            params![deck.id, day_end],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )?;
        Ok(Counts {
            new: new.min((deck.new_per_day - new_done).max(0)),
            learning,
            review: review.min((deck.reviews_per_day - reviews_done).max(0)),
            total,
        })
    }

    /// The next card to study in a deck, or `None` when done for today.
    /// Order: learning cards that are due, then reviews, then new cards.
    pub fn next_card(&self, deck_id: i64) -> Result<Option<StudyCard>> {
        let now = now_secs();
        let deck = self.deck(deck_id)?;
        let counts = self.counts_at(&deck, now)?;
        let day_end = day_start(now) + 86_400;

        let pick = |sql: &str, p: &[&dyn rusqlite::ToSql]| -> Result<Option<i64>> {
            Ok(self.conn.query_row(sql, p, |r| r.get(0)).optional()?)
        };
        const BASE: &str = "SELECT c.id FROM cards c JOIN notes n ON n.id = c.note_id WHERE n.deck_id = ?1 AND ";
        let mut id = pick(&format!("{BASE} c.state IN (1, 3) AND c.due <= ?2 ORDER BY c.due LIMIT 1"), &[&deck_id, &now])?;
        if id.is_none() && counts.review > 0 {
            id = pick(&format!("{BASE} c.state = 2 AND c.due < ?2 ORDER BY c.due LIMIT 1"), &[&deck_id, &day_end])?;
        }
        if id.is_none() && counts.new > 0 {
            id = pick(&format!("{BASE} c.state = 0 ORDER BY c.id LIMIT 1"), &[&deck_id])?;
        }
        if id.is_none() {
            let ahead = now + LEARN_AHEAD;
            id = pick(&format!("{BASE} c.state IN (1, 3) AND c.due <= ?2 ORDER BY c.due LIMIT 1"), &[&deck_id, &ahead])?;
        }
        id.map(|id| self.study_card(id, now)).transpose()
    }

    fn sched(&self, card_id: i64) -> Result<Sched> {
        Ok(self.conn.query_row(
            "SELECT state, due, stability, difficulty, last_review, lapses FROM cards WHERE id = ?1",
            [card_id],
            |r| {
                let (s, d): (Option<f32>, Option<f32>) = (r.get(2)?, r.get(3)?);
                Ok(Sched {
                    state: State::from_i64(r.get(0)?),
                    due: r.get(1)?,
                    memory: s.zip(d).map(|(stability, difficulty)| fsrs::MemoryState { stability, difficulty }),
                    last_review: r.get(4)?,
                    lapses: r.get(5)?,
                })
            },
        )?)
    }

    fn study_card(&self, card_id: i64, now: i64) -> Result<StudyCard> {
        let sched = self.sched(card_id)?;
        let note = self.conn.query_row(
            "SELECT n.word, n.sentence, n.translation, n.note FROM cards c JOIN notes n ON n.id = c.note_id WHERE c.id = ?1",
            [card_id],
            |r| Ok(NoteInput { word: r.get(0)?, sentence: r.get(1)?, translation: r.get(2)?, note: r.get(3)? }),
        )?;
        Ok(StudyCard {
            card_id,
            state: sched.state,
            note,
            intervals: schedule::preview(&sched, now, DESIRED_RETENTION)?,
        })
    }

    /// Records an answer and reschedules the card.
    pub fn answer(&mut self, card_id: i64, rating: Rating, took_ms: i64) -> Result<()> {
        let now_ms = chrono::Utc::now().timestamp_millis();
        let now = now_ms / 1000;
        let before = self.sched(card_id)?;
        let after = schedule::answer(&before, rating, now, DESIRED_RETENTION)?;
        let (stability, difficulty) = after.memory.map_or((None, None), |m| (Some(m.stability), Some(m.difficulty)));

        let tx = self.conn.transaction()?;
        tx.execute(
            "UPDATE cards SET state = ?2, due = ?3, stability = ?4, difficulty = ?5, last_review = ?6,
                              reps = reps + 1, lapses = ?7 WHERE id = ?1",
            params![card_id, after.state.to_i64(), after.due, stability, difficulty, now, after.lapses],
        )?;
        // Two answers in the same millisecond still get distinct ids.
        let last: i64 = tx.query_row("SELECT coalesce(max(id), 0) FROM revlog", [], |r| r.get(0))?;
        tx.execute(
            "INSERT INTO revlog (id, card_id, rating, state_before, interval_secs, stability, difficulty, took_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                now_ms.max(last + 1),
                card_id,
                rating as i64,
                before.state.to_i64(),
                after.due - now,
                stability,
                difficulty,
                took_ms.clamp(0, 60_000 * 10)
            ],
        )?;
        tx.commit()?;
        Ok(())
    }

    // ---- backup ----

    /// Writes a consistent copy of the study database to `path`.
    pub fn backup_to(&self, path: &Path) -> Result<()> {
        self.conn.backup(rusqlite::DatabaseName::Main, path, None)?;
        Ok(())
    }

    /// Replaces all study data with a backup made by [`Srs::backup_to`].
    pub fn restore_from(&mut self, path: &Path) -> Result<()> {
        let check = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let ok: i64 = check
            .query_row("SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name IN ('decks', 'notes', 'cards', 'revlog')", [], |r| r.get(0))
            .unwrap_or(0);
        if ok != 4 {
            return Err(Error::Invalid("este arquivo não é um backup do Japonês App".into()));
        }
        drop(check);
        self.conn.restore(rusqlite::DatabaseName::Main, path, None::<fn(rusqlite::backup::Progress)>)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_srs(name: &str) -> (Srs, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("srs-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        (Srs::open(&dir.join("study.db")).unwrap(), dir)
    }

    fn note(s: &str) -> NoteInput {
        NoteInput { word: "食べる".into(), sentence: s.into(), translation: "t".into(), note: String::new() }
    }

    #[test]
    fn study_flow_counts_and_limits() {
        let (mut srs, dir) = temp_srs("flow");
        let deck = srs.deck_id("Teste").unwrap();
        let added = srs.add_notes(deck, &[note("パンを食べる。"), note("ご飯を食べた。"), note("パンを食べる。")]).unwrap();
        assert_eq!(added, 2, "duplicate sentence skipped");

        let mut d = srs.deck(deck).unwrap();
        d.new_per_day = 1;
        srs.update_deck(&d).unwrap();
        assert_eq!(srs.counts(deck).unwrap().new, 1, "daily new limit");

        let card = srs.next_card(deck).unwrap().unwrap();
        assert_eq!(card.note.sentence, "パンを食べる。");
        assert_eq!(card.intervals[0], 60);
        srs.answer(card.card_id, Rating::Good, 3000).unwrap();
        assert_eq!(srs.counts(deck).unwrap().new, 0, "new limit used up");
        // Counted when it comes due: seen from now, a step that crosses 4 a.m. belongs to tomorrow.
        let due = srs.sched(card.card_id).unwrap().due;
        assert_eq!(srs.counts_at(&d, due).unwrap().learning, 1, "card now learning");

        // The 10-minute step isn't due yet but is within the learn-ahead window.
        let again = srs.next_card(deck).unwrap().unwrap();
        assert_eq!(again.card_id, card.card_id);
        srs.answer(again.card_id, Rating::Easy, 1000).unwrap();
        assert!(srs.next_card(deck).unwrap().is_none(), "done for today");

        let backup = dir.join("backup.db");
        srs.backup_to(&backup).unwrap();
        srs.delete_deck(deck).unwrap();
        assert!(srs.decks().unwrap().is_empty());
        srs.restore_from(&backup).unwrap();
        assert_eq!(srs.decks().unwrap()[0].counts.total, 2);
        drop(srs);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn day_starts_at_4am() {
        let now = now_secs();
        let start = day_start(now);
        assert!(start <= now && now - start < 86_400);
        assert_eq!(Local.timestamp_opt(start, 0).unwrap().format("%H:%M").to_string(), "04:00");
    }
}
