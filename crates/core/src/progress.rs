//! What the learner has done: lessons read, quiz scores, lab steps passed, exam attempts and the
//! exam in progress. Saved as JSON in the data folder so every session picks up where the last
//! one stopped, in the app and in the `dojo` CLI alike.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::check::CheckResult;
use crate::content::{Catalog, Lesson};
use crate::domains::DOMAINS;
use crate::exam::ExamAttempt;

pub const VERSION: u32 = 1;
/// A quiz counts as passed at this share of correct answers.
pub const QUIZ_PASS: f32 = 0.7;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuizScore {
    pub correct: u32,
    pub total: u32,
}

impl QuizScore {
    pub fn passed(&self) -> bool {
        self.total == 0 || self.correct as f32 >= QUIZ_PASS * self.total as f32
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LessonProgress {
    pub read: bool,
    /// Best quiz score so far.
    pub quiz: Option<QuizScore>,
    /// Lab steps (by index) whose checks all passed.
    pub steps: BTreeSet<usize>,
    pub completed_at: Option<i64>,
    pub last_seen: i64,
    /// Minutes spent with the lesson open (approximate).
    pub minutes: u32,
    /// The lab session, so a lab can be resumed exactly where it was left: when it was prepared,
    /// the last check results per step and how many hints were revealed per step.
    pub prepared_at: Option<i64>,
    pub results: BTreeMap<usize, Vec<CheckResult>>,
    pub hints: BTreeMap<usize, usize>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ActiveExam {
    pub exam_id: String,
    pub started_at: i64,
    pub flagged: BTreeSet<String>,
    pub current_task: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LessonStatus {
    NotStarted,
    InProgress,
    Done,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Progress {
    pub version: u32,
    pub last_lesson: Option<String>,
    pub lessons: BTreeMap<String, LessonProgress>,
    pub exams: Vec<ExamAttempt>,
    pub active_exam: Option<ActiveExam>,
    /// Days (`YYYY-MM-DD`, local time) with any study activity, for the streak.
    pub active_days: BTreeSet<String>,
}

/// How ready the learner is for one CKAD domain.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Readiness {
    pub domain: String,
    pub title: String,
    pub weight: u32,
    pub lessons_done: usize,
    pub lessons_total: usize,
    /// Best mock-exam score in this domain, in percent.
    pub best_exam: Option<f32>,
}

impl Readiness {
    /// 0..=1: half from lessons done, half from the best exam score (lessons only until an exam).
    pub fn score(&self) -> f32 {
        let lessons = if self.lessons_total == 0 { 0.0 } else { self.lessons_done as f32 / self.lessons_total as f32 };
        match self.best_exam {
            Some(e) => 0.5 * lessons + 0.5 * (e / 100.0).clamp(0.0, 1.0),
            None => lessons,
        }
    }
}

impl Progress {
    pub fn load(path: &Path) -> std::io::Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(text) => {
                serde_json::from_str(&text).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self { version: VERSION, ..Self::default() }),
            Err(e) => Err(e),
        }
    }

    /// Write atomically (temp file + rename) so a crash never leaves half a file.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text =
            serde_json::to_string_pretty(self).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let tmp = path.with_extension(format!("json.{}.tmp", std::process::id()));
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, path)
    }

    pub fn lesson(&self, id: &str) -> Option<&LessonProgress> {
        self.lessons.get(id)
    }

    fn entry(&mut self, id: &str) -> &mut LessonProgress {
        self.lessons.entry(id.to_string()).or_default()
    }

    fn touch_day(&mut self) {
        self.active_days.insert(chrono::Local::now().format("%Y-%m-%d").to_string());
    }

    pub fn open_lesson(&mut self, lesson: &Lesson, now: i64) {
        self.last_lesson = Some(lesson.id.clone());
        self.entry(&lesson.id).last_seen = now;
    }

    pub fn add_minutes(&mut self, id: &str, minutes: u32) {
        let e = self.entry(id);
        e.minutes = e.minutes.saturating_add(minutes);
    }

    pub fn mark_read(&mut self, lesson: &Lesson, now: i64) {
        self.entry(&lesson.id).read = true;
        self.touch_day();
        self.refresh_completion(lesson, now);
    }

    pub fn record_quiz(&mut self, lesson: &Lesson, score: QuizScore, now: i64) {
        let e = self.entry(&lesson.id);
        if e.quiz.is_none_or(|best| score.correct * best.total.max(1) >= best.correct * score.total.max(1)) {
            e.quiz = Some(score);
        }
        self.touch_day();
        self.refresh_completion(lesson, now);
    }

    pub fn record_step(&mut self, lesson: &Lesson, step: usize, passed: bool, now: i64) {
        self.entry(&lesson.id);
        // A pass is kept until the lab is reset: later steps often undo an earlier step's state on
        // purpose (remove the container you created), so re-checking must not take credit away.
        if passed {
            self.entry(&lesson.id).steps.insert(step);
        }
        self.touch_day();
        self.refresh_completion(lesson, now);
    }

    pub fn record_exam(&mut self, attempt: ExamAttempt) {
        self.touch_day();
        self.active_exam = None;
        self.exams.push(attempt);
    }

    /// Clear a lesson's lab progress (after "Reset lab").
    pub fn reset_lab(&mut self, lesson: &Lesson) {
        if let Some(e) = self.lessons.get_mut(&lesson.id) {
            e.steps.clear();
            e.completed_at = None;
            e.prepared_at = None;
            e.results.clear();
            e.hints.clear();
        }
    }

    pub fn record_prepared(&mut self, lesson: &Lesson, now: i64) {
        self.entry(&lesson.id).prepared_at = Some(now);
        self.touch_day();
    }

    pub fn record_results(&mut self, lesson: &Lesson, step: usize, results: Vec<CheckResult>) {
        self.entry(&lesson.id).results.insert(step, results);
    }

    pub fn record_hints(&mut self, lesson: &Lesson, step: usize, shown: usize) {
        self.entry(&lesson.id).hints.insert(step, shown);
    }

    /// Labs started but not finished, most recent first.
    pub fn labs_in_progress<'a>(&self, catalog: &'a Catalog) -> Vec<&'a Lesson> {
        let mut labs: Vec<&Lesson> = catalog
            .lessons
            .values()
            .filter(|l| l.has_lab() && !self.is_done(l))
            .filter(|l| {
                self.lessons
                    .get(&l.id)
                    .is_some_and(|p| p.prepared_at.is_some() || !p.steps.is_empty() || !p.results.is_empty())
            })
            .collect();
        labs.sort_by_key(|l| std::cmp::Reverse(self.lessons.get(&l.id).map_or(0, |p| p.last_seen)));
        labs
    }

    /// When this learner last did anything (lesson opened or exam finished).
    pub fn last_active(&self) -> Option<i64> {
        let lessons = self.lessons.values().map(|p| p.last_seen).max();
        let exams = self.exams.iter().map(|a| a.finished_at).max();
        lessons.max(exams).filter(|t| *t > 0)
    }

    fn refresh_completion(&mut self, lesson: &Lesson, now: i64) {
        let done = self.requirements_met(lesson);
        let e = self.entry(&lesson.id);
        match (done, e.completed_at) {
            (true, None) => e.completed_at = Some(now),
            (false, Some(_)) => e.completed_at = None,
            _ => {}
        }
    }

    fn requirements_met(&self, lesson: &Lesson) -> bool {
        let Some(p) = self.lessons.get(&lesson.id) else { return false };
        let quiz_ok = lesson.quiz.is_empty() || p.quiz.is_some_and(|q| q.passed());
        let steps = lesson.lab.as_ref().map_or(0, |l| l.steps.len());
        let lab_ok = (0..steps).all(|i| p.steps.contains(&i));
        p.read && quiz_ok && lab_ok
    }

    pub fn status(&self, lesson: &Lesson) -> LessonStatus {
        match self.lessons.get(&lesson.id) {
            Some(p) if p.completed_at.is_some() => LessonStatus::Done,
            Some(p) if p.read || p.quiz.is_some() || !p.steps.is_empty() || p.last_seen > 0 => LessonStatus::InProgress,
            _ => LessonStatus::NotStarted,
        }
    }

    pub fn is_done(&self, lesson: &Lesson) -> bool {
        self.status(lesson) == LessonStatus::Done
    }

    /// Where to continue: the last lesson if it is unfinished, else the first unfinished lesson
    /// after it in study order, else the first unfinished one anywhere.
    pub fn resume<'a>(&self, catalog: &'a Catalog) -> Option<&'a Lesson> {
        if let Some(last) = self.last_lesson.as_deref().and_then(|id| catalog.lesson(id)) {
            if !self.is_done(last) {
                return Some(last);
            }
            if let Some(next) = catalog.ordered_lessons().skip_while(|l| l.id != last.id).find(|l| !self.is_done(l)) {
                return Some(next);
            }
        }
        catalog.ordered_lessons().find(|l| !self.is_done(l))
    }

    pub fn done_count(&self, catalog: &Catalog) -> (usize, usize) {
        let total = catalog.lessons.len();
        let done = catalog.lessons.values().filter(|l| self.is_done(l)).count();
        (done, total)
    }

    pub fn track_counts(&self, catalog: &Catalog, track: &str) -> (usize, usize) {
        let lessons: Vec<&Lesson> = catalog.lessons.values().filter(|l| l.track == track).collect();
        (lessons.iter().filter(|l| self.is_done(l)).count(), lessons.len())
    }

    pub fn readiness(&self, catalog: &Catalog) -> Vec<Readiness> {
        DOMAINS
            .iter()
            .map(|d| {
                let lessons: Vec<&Lesson> =
                    catalog.lessons.values().filter(|l| l.domain.as_deref() == Some(d.id)).collect();
                let best_exam = self
                    .exams
                    .iter()
                    .filter_map(|a| a.domains.get(d.id))
                    .filter(|s| s.possible > 0.0)
                    .map(|s| s.percent())
                    .fold(None, |acc: Option<f32>, p| Some(acc.map_or(p, |a| a.max(p))));
                Readiness {
                    domain: d.id.to_string(),
                    title: d.title.to_string(),
                    weight: d.weight,
                    lessons_done: lessons.iter().filter(|l| self.is_done(l)).count(),
                    lessons_total: lessons.len(),
                    best_exam,
                }
            })
            .collect()
    }

    /// Weighted CKAD readiness, 0..=100.
    pub fn overall_readiness(&self, catalog: &Catalog) -> f32 {
        self.readiness(catalog).iter().map(|r| r.score() * r.weight as f32).sum::<f32>()
    }

    /// Consecutive days with activity, ending today or yesterday.
    pub fn streak(&self, today: chrono::NaiveDate) -> u32 {
        let has = |d: chrono::NaiveDate| self.active_days.contains(&d.format("%Y-%m-%d").to_string());
        let mut day = if has(today) {
            today
        } else {
            match today.pred_opt() {
                Some(y) if has(y) => y,
                _ => return 0,
            }
        };
        let mut n = 0;
        while has(day) {
            n += 1;
            match day.pred_opt() {
                Some(p) => day = p,
                None => break,
            }
        }
        n
    }

    pub fn best_attempt(&self, exam_id: &str) -> Option<&ExamAttempt> {
        self.exams.iter().filter(|a| a.exam_id == exam_id).max_by(|a, b| a.percent.total_cmp(&b.percent))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::check::Check;
    use crate::content::{Lab, LabStep, Needs, QuizQuestion};

    fn lesson(id: &str, quiz: bool, steps: usize) -> Lesson {
        Lesson {
            id: id.into(),
            track: "t".into(),
            module: "m".into(),
            title: id.into(),
            minutes: 10,
            domain: Some("deployment".into()),
            needs: Needs::Docker,
            objectives: vec![],
            body: String::new(),
            quiz: if quiz {
                vec![QuizQuestion { q: "?".into(), choices: vec!["a".into(), "b".into()], answer: 0, why: "x".into() }]
            } else {
                vec![]
            },
            lab: Some(Lab {
                steps: (0..steps)
                    .map(|i| LabStep {
                        title: format!("s{i}"),
                        task: String::new(),
                        hints: vec![],
                        solution: String::new(),
                        checks: vec![Check::default()],
                    })
                    .collect(),
                ..Default::default()
            }),
            path: format!("t/{id}.md"),
        }
    }

    #[test]
    fn completion_needs_read_quiz_and_lab() {
        let l = lesson("a", true, 2);
        let mut p = Progress::default();
        assert_eq!(p.status(&l), LessonStatus::NotStarted);
        p.mark_read(&l, 1);
        assert_eq!(p.status(&l), LessonStatus::InProgress);
        p.record_quiz(&l, QuizScore { correct: 1, total: 1 }, 2);
        p.record_step(&l, 0, true, 3);
        assert!(!p.is_done(&l));
        p.record_step(&l, 1, true, 4);
        assert!(p.is_done(&l));
        p.record_step(&l, 0, false, 5);
        assert!(p.is_done(&l), "a later failed re-check keeps an earned pass");
        p.record_prepared(&l, 6);
        p.record_hints(&l, 1, 2);
        assert_eq!(p.lesson("a").unwrap().completed_at, Some(4));
        p.reset_lab(&l);
        assert!(!p.is_done(&l));
        let e = p.lesson("a").unwrap();
        assert!(e.prepared_at.is_none() && e.hints.is_empty() && e.steps.is_empty());
    }

    #[test]
    fn best_quiz_is_kept() {
        let l = lesson("a", true, 0);
        let mut p = Progress::default();
        p.record_quiz(&l, QuizScore { correct: 3, total: 4 }, 1);
        p.record_quiz(&l, QuizScore { correct: 1, total: 4 }, 2);
        assert_eq!(p.lesson("a").unwrap().quiz, Some(QuizScore { correct: 3, total: 4 }));
    }

    #[test]
    fn resume_picks_last_unfinished_then_next() {
        let mut c = Catalog::default();
        for id in ["a", "b", "c"] {
            c.lessons.insert(id.into(), lesson(id, false, 0));
        }
        c.tracks.push(crate::content::Track {
            id: "t".into(),
            title: "T".into(),
            summary: String::new(),
            order: 1,
            modules: vec![],
            lessons: vec!["a".into(), "b".into(), "c".into()],
        });
        let mut p = Progress::default();
        assert_eq!(p.resume(&c).map(|l| l.id.as_str()), Some("a"));
        let b = c.lesson("b").unwrap().clone();
        p.open_lesson(&b, 1);
        assert_eq!(p.resume(&c).map(|l| l.id.as_str()), Some("b"));
        p.mark_read(&b, 2);
        assert_eq!(p.resume(&c).map(|l| l.id.as_str()), Some("c"));
    }

    #[test]
    fn streak_counts_back_from_today_or_yesterday() {
        let mut p = Progress::default();
        let d = |s: &str| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap();
        for day in ["2026-10-22", "2026-10-23", "2026-10-24"] {
            p.active_days.insert(day.into());
        }
        assert_eq!(p.streak(d("2026-10-24")), 3);
        assert_eq!(p.streak(d("2026-10-25")), 3);
        assert_eq!(p.streak(d("2026-10-27")), 0);
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = std::env::temp_dir().join(format!("poddojo-test-{}", std::process::id()));
        let path = dir.join("progress.json");
        let mut p = Progress { version: VERSION, last_lesson: Some("a".into()), ..Default::default() };
        p.active_days.insert("2026-10-25".into());
        p.save(&path).unwrap();
        assert_eq!(Progress::load(&path).unwrap(), p);
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(Progress::load(&dir.join("missing.json")).unwrap().version, VERSION);
    }
}
