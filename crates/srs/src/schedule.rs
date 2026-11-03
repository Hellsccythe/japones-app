//! What happens to a card when it's answered: FSRS for the memory state and
//! intervals, Anki-style short steps for new and lapsed cards.

use fsrs::{MemoryState, FSRS};
use serde::{Deserialize, Serialize};

use crate::Result;

const MINUTE: i64 = 60;
const DAY: i64 = 86_400;
/// New card: Again repeats in 1 min, Good goes to a 10 min step, then graduates.
const LEARN_AGAIN: i64 = MINUTE;
const LEARN_HARD: i64 = 6 * MINUTE;
const LEARN_GOOD: i64 = 10 * MINUTE;
/// Forgotten review card: back in 10 min before returning to day intervals.
const RELEARN: i64 = 10 * MINUTE;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    New,
    Learning,
    Review,
    Relearning,
}

impl State {
    pub(crate) fn to_i64(self) -> i64 {
        match self {
            State::New => 0,
            State::Learning => 1,
            State::Review => 2,
            State::Relearning => 3,
        }
    }

    pub(crate) fn from_i64(v: i64) -> State {
        match v {
            1 => State::Learning,
            2 => State::Review,
            3 => State::Relearning,
            _ => State::New,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Rating {
    Again = 1,
    Hard = 2,
    Good = 3,
    Easy = 4,
}

impl Rating {
    pub fn from_u8(v: u8) -> Option<Rating> {
        Some(match v {
            1 => Rating::Again,
            2 => Rating::Hard,
            3 => Rating::Good,
            4 => Rating::Easy,
            _ => return None,
        })
    }
}

/// Scheduling fields of a card.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sched {
    pub state: State,
    /// Unix seconds.
    pub due: i64,
    pub memory: Option<MemoryState>,
    pub last_review: Option<i64>,
    pub lapses: i64,
}

impl Sched {
    pub fn new_card(now: i64) -> Sched {
        Sched { state: State::New, due: now, memory: None, last_review: None, lapses: 0 }
    }
}

/// The card's scheduling after answering `rating` at `now`.
pub fn answer(card: &Sched, rating: Rating, now: i64, retention: f32) -> Result<Sched> {
    let fsrs = FSRS::new(&[])?;
    let days_elapsed = card.last_review.map_or(0, |t| ((now - t).max(0) / DAY) as u32);
    let next = fsrs.next_states(card.memory, retention, days_elapsed)?;
    let chosen = match rating {
        Rating::Again => &next.again,
        Rating::Hard => &next.hard,
        Rating::Good => &next.good,
        Rating::Easy => &next.easy,
    };

    // Day intervals, kept in order so a better answer never schedules sooner.
    let days = |s: &fsrs::ItemState| (s.interval.round() as i64).max(1);
    let hard = days(&next.hard);
    let good = days(&next.good).max(hard + 1);
    let easy = days(&next.easy).max(good + 1);
    let in_days = |d: i64| now + d * DAY;

    let (state, due) = match (card.state, rating) {
        (State::New | State::Learning, Rating::Again) => (State::Learning, now + LEARN_AGAIN),
        (State::New | State::Learning, Rating::Hard) => (State::Learning, now + LEARN_HARD),
        (State::New, Rating::Good) => (State::Learning, now + LEARN_GOOD),
        (State::Learning, Rating::Good) => (State::Review, in_days(good)),
        (State::New | State::Learning, Rating::Easy) => (State::Review, in_days(easy)),

        (State::Review, Rating::Again) => (State::Relearning, now + RELEARN),
        (State::Review, Rating::Hard) => (State::Review, in_days(hard)),
        (State::Review, Rating::Good) => (State::Review, in_days(good)),
        (State::Review, Rating::Easy) => (State::Review, in_days(easy)),

        (State::Relearning, Rating::Again | Rating::Hard) => (State::Relearning, now + RELEARN),
        (State::Relearning, Rating::Good) => (State::Review, in_days(hard.max(1))),
        (State::Relearning, Rating::Easy) => (State::Review, in_days(good)),
    };

    Ok(Sched {
        state,
        due,
        memory: Some(chosen.memory),
        last_review: Some(now),
        lapses: card.lapses + i64::from(card.state == State::Review && rating == Rating::Again),
    })
}

/// Seconds until the card comes back for each rating (Again, Hard, Good, Easy),
/// shown on the answer buttons.
pub fn preview(card: &Sched, now: i64, retention: f32) -> Result<[i64; 4]> {
    let mut out = [0; 4];
    for (i, r) in [Rating::Again, Rating::Hard, Rating::Good, Rating::Easy].into_iter().enumerate() {
        out[i] = answer(card, r, now, retention)?.due - now;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_800_000_000;

    #[test]
    fn new_card_goes_through_learning_steps() {
        let c = Sched::new_card(NOW);
        let again = answer(&c, Rating::Again, NOW, 0.9).unwrap();
        assert_eq!((again.state, again.due - NOW), (State::Learning, 60));
        let good = answer(&c, Rating::Good, NOW, 0.9).unwrap();
        assert_eq!((good.state, good.due - NOW), (State::Learning, 600));
        let graduated = answer(&good, Rating::Good, NOW + 600, 0.9).unwrap();
        assert_eq!(graduated.state, State::Review);
        assert!(graduated.due - (NOW + 600) >= DAY);
    }

    #[test]
    fn intervals_are_ordered_and_grow() {
        let mut c = answer(&Sched::new_card(NOW), Rating::Easy, NOW, 0.9).unwrap();
        let p = preview(&c, c.due, 0.9).unwrap();
        assert!(p[0] < p[1] && p[1] < p[2] && p[2] < p[3], "{p:?}");
        let first = c.due - NOW;
        let t = c.due;
        c = answer(&c, Rating::Good, t, 0.9).unwrap();
        assert!(c.due - t > first, "second interval should be longer");
    }

    #[test]
    fn lapse_relearns_and_counts() {
        let c = answer(&Sched::new_card(NOW), Rating::Easy, NOW, 0.9).unwrap();
        let lapsed = answer(&c, Rating::Again, c.due, 0.9).unwrap();
        assert_eq!((lapsed.state, lapsed.lapses, lapsed.due - c.due), (State::Relearning, 1, 600));
        let back = answer(&lapsed, Rating::Good, lapsed.due, 0.9).unwrap();
        assert_eq!(back.state, State::Review);
    }
}
