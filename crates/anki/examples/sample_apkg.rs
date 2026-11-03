//! Writes a sample .apkg (a review card with history, a learning card and a new
//! card, with audio) to check imports in Anki: cargo run -p anki --example sample_apkg -- out.apkg
use anki::{Card, CardKind, Review, Schedule};

fn main() {
    let out = std::env::args().nth(1).unwrap_or("sample.apkg".into());
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
    let day = 86_400;
    let card = |sentence: &str, furigana: &str, schedule| Card {
        word: "食べる".into(),
        sentence: sentence.into(),
        sentence_furigana: furigana.into(),
        translation: "Tradução.".into(),
        note: "Nota.".into(),
        audio: Some(std::fs::read(std::env::args().nth(2).unwrap_or_default()).unwrap_or(b"RIFF".to_vec())),
        audio_credit: Some("VOICEVOX:ずんだもん".into()),
        schedule,
    };
    let review = Schedule {
        kind: CardKind::Review,
        due: now + 3 * day,
        last_review: Some(now - 2 * day),
        stability: Some(5.2),
        difficulty: Some(4.8),
        reps: 2,
        lapses: 0,
        reviews: vec![
            Review { time_ms: (now - 2 * day - 700) * 1000, ease: 3, kind: CardKind::New, interval_secs: 600, took_ms: 5000 },
            Review { time_ms: (now - 2 * day) * 1000, ease: 3, kind: CardKind::Learning, interval_secs: 5 * day, took_ms: 3000 },
        ],
    };
    let learning = Schedule {
        kind: CardKind::Learning,
        due: now + 300,
        last_review: Some(now - 60),
        stability: Some(0.4),
        difficulty: Some(7.0),
        reps: 1,
        lapses: 0,
        reviews: vec![Review { time_ms: (now - 60) * 1000, ease: 1, kind: CardKind::New, interval_secs: 60, took_ms: 2000 }],
    };
    let cards = [
        card("寿司を食べた。", " 寿司[すし]を 食[た]べた。", Some(review)),
        card("パンを食べる。", "パンを 食[た]べる。", Some(learning)),
        card("何を食べたい？", " 何[なに]を 食[た]べたい？", None),
    ];
    let day_start = now - now.rem_euclid(day);
    anki::apkg::write(std::path::Path::new(&out), "Japonês::Amostra", &cards, &["japones-app".into()], day_start).unwrap();
    println!("{out}");
}
