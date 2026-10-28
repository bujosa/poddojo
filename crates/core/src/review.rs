//! Spaced review: every quiz question the learner answers and every mock-exam task they attempt
//! becomes a card in a Leitner system. A miss sends the card back to the first box (due again
//! today), a success moves it up a box and pushes the next review further out, so weak spots keep
//! coming back until they stick and known material stops taking time.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::content::{Catalog, Exam, ExamTask, Lesson, QuizQuestion};

/// Days until the next review for a card in each box (box 0 is "missed": due again soon).
pub const INTERVAL_DAYS: [i64; 6] = [0, 1, 3, 7, 16, 35];
/// A missed card comes back after this many seconds, so it is reviewed again the same day.
pub const RELEARN_SECS: i64 = 10 * 60;
/// From this box up a card counts as learned.
pub const LEARNED_BOX: u8 = 3;
const DAY: i64 = 86_400;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Card {
    /// Leitner box, `0..INTERVAL_DAYS.len()`.
    pub level: u8,
    /// When the card is next due (Unix seconds).
    pub due: i64,
    pub reviews: u32,
    pub lapses: u32,
    pub last: i64,
}

impl Card {
    /// Record one answer at `now`.
    pub fn answer(&mut self, correct: bool, now: i64) {
        self.reviews += 1;
        self.last = now;
        if correct {
            let max = (INTERVAL_DAYS.len() - 1) as u8;
            self.level = (self.level + 1).min(max);
            self.due = now + INTERVAL_DAYS[self.level as usize] * DAY;
        } else {
            if self.reviews > 1 || self.level > 0 {
                self.lapses += 1;
            }
            self.level = 0;
            self.due = now + RELEARN_SECS;
        }
    }

    pub fn is_due(&self, now: i64) -> bool {
        self.due <= now
    }

    pub fn learned(&self) -> bool {
        self.level >= LEARNED_BOX
    }
}

/// What a card points at.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Item<'a> {
    Question { lesson: &'a Lesson, index: usize, question: &'a QuizQuestion },
    Task { exam: &'a Exam, task: &'a ExamTask },
}

impl Item<'_> {
    pub fn domain(&self) -> Option<&str> {
        match self {
            Item::Question { lesson, .. } => lesson.domain.as_deref(),
            Item::Task { task, .. } => Some(task.domain.as_str()),
        }
    }
}

/// The card key of a quiz question: lesson id + a hash of the question text, so reordering or
/// adding questions keeps the learner's history (an edited question starts fresh).
pub fn question_key(lesson: &Lesson, q: &QuizQuestion) -> String {
    let mut h: u32 = 0x811c9dc5;
    for b in q.q.bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(0x01000193);
    }
    format!("q:{}:{h:08x}", lesson.id)
}

pub fn task_key(exam: &Exam, task: &ExamTask) -> String {
    format!("t:{}/{}", exam.id, task.id)
}

/// Resolve a card key against the catalog (cards for removed content resolve to `None`).
pub fn resolve<'a>(catalog: &'a Catalog, key: &str) -> Option<Item<'a>> {
    if let Some(rest) = key.strip_prefix("q:") {
        let (lesson_id, _) = rest.rsplit_once(':')?;
        let lesson = catalog.lesson(lesson_id)?;
        let (index, question) = lesson.quiz.iter().enumerate().find(|(_, q)| question_key(lesson, q) == key)?;
        return Some(Item::Question { lesson, index, question });
    }
    let (exam_id, task_id) = key.strip_prefix("t:")?.split_once('/')?;
    let exam = catalog.exams.iter().find(|e| e.id == exam_id)?;
    let task = exam.tasks.iter().find(|t| t.id == task_id)?;
    Some(Item::Task { exam, task })
}

/// The learner's cards, keyed by [`question_key`] / [`task_key`].
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Deck {
    pub cards: BTreeMap<String, Card>,
}

/// Counts for the review dashboard.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct DeckStats {
    pub due_questions: usize,
    pub due_tasks: usize,
    pub learning: usize,
    pub learned: usize,
    /// Cards that will be due within the next 24 hours (not counting the ones due now).
    pub due_tomorrow: usize,
}

impl Deck {
    pub fn answer(&mut self, key: &str, correct: bool, now: i64) {
        self.cards.entry(key.to_string()).or_default().answer(correct, now);
    }

    /// Record a whole lesson quiz: every question becomes (or updates) a card.
    pub fn record_quiz(&mut self, lesson: &Lesson, answers: &[Option<usize>], now: i64) {
        for (q, a) in lesson.quiz.iter().zip(answers) {
            self.answer(&question_key(lesson, q), *a == Some(q.answer), now);
        }
    }

    /// Due cards that still exist in the catalog, most overdue first; tasks after questions on a tie.
    pub fn due<'a>(&self, catalog: &'a Catalog, now: i64) -> Vec<(String, Item<'a>)> {
        let mut due: Vec<(&String, &Card)> = self.cards.iter().filter(|(_, c)| c.is_due(now)).collect();
        due.sort_by_key(|(k, c)| (c.due, k.starts_with("t:")));
        due.into_iter().filter_map(|(k, _)| resolve(catalog, k).map(|item| (k.clone(), item))).collect()
    }

    pub fn stats(&self, catalog: &Catalog, now: i64) -> DeckStats {
        let mut s = DeckStats::default();
        for (key, card) in &self.cards {
            let Some(item) = resolve(catalog, key) else { continue };
            if card.learned() {
                s.learned += 1;
            } else {
                s.learning += 1;
            }
            if card.is_due(now) {
                match item {
                    Item::Question { .. } => s.due_questions += 1,
                    Item::Task { .. } => s.due_tasks += 1,
                }
            } else if card.due <= now + DAY {
                s.due_tomorrow += 1;
            }
        }
        s
    }

    /// Questions answered wrong most often (at least one lapse or still in box 0), worst first.
    pub fn weakest<'a>(&self, catalog: &'a Catalog, limit: usize) -> Vec<(Item<'a>, Card)> {
        let mut weak: Vec<(Item, Card)> = self
            .cards
            .iter()
            .filter(|(_, c)| c.level == 0 || c.lapses > 0)
            .filter_map(|(k, c)| resolve(catalog, k).map(|i| (i, *c)))
            .collect();
        weak.sort_by_key(|(_, c)| (std::cmp::Reverse(c.lapses), c.level));
        weak.truncate(limit);
        weak
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leitner_moves_up_on_success_and_back_on_a_miss() {
        let mut c = Card::default();
        c.answer(true, 0);
        assert_eq!((c.level, c.due), (1, DAY));
        c.answer(true, DAY);
        assert_eq!((c.level, c.due), (2, 4 * DAY));
        c.answer(false, 4 * DAY);
        assert_eq!((c.level, c.lapses, c.due), (0, 1, 4 * DAY + RELEARN_SECS));
        for _ in 0..10 {
            c.answer(true, 0);
        }
        assert_eq!(c.level as usize, INTERVAL_DAYS.len() - 1);
        assert!(c.learned());
    }

    #[test]
    fn first_miss_is_not_a_lapse() {
        let mut c = Card::default();
        c.answer(false, 0);
        assert_eq!((c.level, c.lapses), (0, 0));
        assert!(c.is_due(RELEARN_SECS));
        assert!(!c.is_due(RELEARN_SECS - 1));
    }
}
