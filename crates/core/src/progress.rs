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
use crate::review::Deck;

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
    /// Spaced-review cards: quiz questions and mock-exam tasks.
    pub deck: Deck,
}

/// How ready the learner is for one CKAD domain.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Readiness {
    pub domain: String,
    pub title: String,
    pub weight: u32,
    pub lessons_done: usize,
    pub lessons_total: usize,
    /// This domain's score in the learner's latest fresh (first-attempt) mocks: the mean of the
    /// last two, in percent. Retakes are left out: a retaken mock measures memory, not skill.
    pub recent_exam: Option<f32>,
}

impl Readiness {
    /// Share of the domain's lessons completed, 0..=1.
    pub fn coverage(&self) -> f32 {
        if self.lessons_total == 0 { 0.0 } else { self.lessons_done as f32 / self.lessons_total as f32 }
    }

    /// Weak: scored under the pass mark in recent fresh mocks.
    pub fn weak(&self) -> bool {
        self.recent_exam.is_some_and(|e| e < PASS_MARK)
    }
}

/// The CKAD pass mark, in percent.
pub const PASS_MARK: f32 = 66.0;
/// The recent fresh-mock average that, with full coverage and no weak domain, says "book it".
pub const READY_MARK: f32 = 80.0;

/// The honest answer to "would I pass?", from evidence rather than a blended number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Verdict {
    /// Less than 60% of the CKAD curriculum done and no mock yet.
    KeepStudying,
    /// Most of the curriculum done but no timed mock yet: no evidence about exam performance.
    TakeAMock,
    /// Recent fresh mocks under the pass mark.
    NotYet,
    /// Passing recent mocks, but without margin (under 80%, a weak domain, gaps or one mock only).
    Close,
    /// Two or more fresh mocks averaging 80%+, no weak domain and the curriculum done.
    Ready,
}

impl Verdict {
    pub fn title(self) -> &'static str {
        match self {
            Verdict::KeepStudying => "Keep studying",
            Verdict::TakeAMock => "Time for a timed mock",
            Verdict::NotYet => "Not yet",
            Verdict::Close => "Close",
            Verdict::Ready => "Ready to book",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ExamReadiness {
    pub verdict: Verdict,
    /// Weighted share of the CKAD lessons completed, 0..=100.
    pub coverage: f32,
    /// Fresh (first) attempts of each mock, oldest first: (exam id, percent, finished at).
    pub fresh: Vec<(String, f32, i64)>,
    /// Mean of the last two fresh attempts, in percent.
    pub recent: Option<f32>,
    /// Domain ids under the pass mark in recent fresh mocks, biggest exam weight first.
    pub weak: Vec<String>,
    /// What to do next, in one sentence.
    pub advice: String,
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

    /// Record a finished mock. Every task joins the review deck: a task without full marks is due
    /// for a drill right away, a solved one comes back later.
    pub fn record_exam(&mut self, attempt: ExamAttempt, catalog: &Catalog) {
        self.touch_day();
        self.active_exam = None;
        if let Some(exam) = catalog.exams.iter().find(|e| e.id == attempt.exam_id) {
            for r in &attempt.tasks {
                if let Some(task) = exam.tasks.iter().find(|t| t.id == r.id) {
                    let solved = r.possible > 0.0 && r.earned + f32::EPSILON >= r.possible;
                    self.deck.answer(&crate::review::task_key(exam, task), solved, attempt.finished_at);
                }
            }
        }
        self.exams.push(attempt);
    }

    /// Record a lesson quiz: the best score for completion and every question for spaced review.
    pub fn record_quiz_answers(&mut self, lesson: &Lesson, answers: &[Option<usize>], now: i64) -> QuizScore {
        let correct = lesson.quiz.iter().zip(answers).filter(|(q, a)| **a == Some(q.answer)).count() as u32;
        let score = QuizScore { correct, total: lesson.quiz.len() as u32 };
        self.deck.record_quiz(lesson, answers, now);
        self.record_quiz(lesson, score, now);
        score
    }

    /// Record one spaced-review answer (a quiz card or an exam-task drill).
    pub fn record_review(&mut self, key: &str, correct: bool, now: i64) {
        self.deck.answer(key, correct, now);
        self.touch_day();
    }

    /// First attempts of each mock, oldest first. Later attempts of the same mock are retakes.
    pub fn fresh_attempts(&self) -> Vec<&ExamAttempt> {
        let mut seen = BTreeSet::new();
        let mut fresh: Vec<&ExamAttempt> = Vec::new();
        let mut all: Vec<&ExamAttempt> = self.exams.iter().collect();
        all.sort_by_key(|a| a.finished_at);
        for a in all {
            if seen.insert(a.exam_id.as_str()) {
                fresh.push(a);
            }
        }
        fresh
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
        let fresh = self.fresh_attempts();
        let recent: Vec<&ExamAttempt> = fresh.iter().rev().take(2).copied().collect();
        DOMAINS
            .iter()
            .map(|d| {
                let lessons: Vec<&Lesson> =
                    catalog.lessons.values().filter(|l| l.domain.as_deref() == Some(d.id)).collect();
                let scores: Vec<f32> = recent
                    .iter()
                    .filter_map(|a| a.domains.get(d.id))
                    .filter(|s| s.possible > 0.0)
                    .map(|s| s.percent())
                    .collect();
                Readiness {
                    domain: d.id.to_string(),
                    title: d.title.to_string(),
                    weight: d.weight,
                    lessons_done: lessons.iter().filter(|l| self.is_done(l)).count(),
                    lessons_total: lessons.len(),
                    recent_exam: (!scores.is_empty()).then(|| scores.iter().sum::<f32>() / scores.len() as f32),
                }
            })
            .collect()
    }

    /// Weighted share of the CKAD curriculum completed, 0..=100.
    pub fn coverage(&self, catalog: &Catalog) -> f32 {
        self.readiness(catalog).iter().map(|r| r.coverage() * r.weight as f32).sum()
    }

    /// Would the learner pass today? Coverage says what was studied; only fresh timed mocks say how
    /// it goes under exam conditions, so the verdict leans on them and says what is missing.
    pub fn exam_readiness(&self, catalog: &Catalog) -> ExamReadiness {
        let domains = self.readiness(catalog);
        let coverage: f32 = domains.iter().map(|r| r.coverage() * r.weight as f32).sum();
        let fresh: Vec<(String, f32, i64)> =
            self.fresh_attempts().iter().map(|a| (a.exam_id.clone(), a.percent, a.finished_at)).collect();
        let last: Vec<f32> = fresh.iter().rev().take(2).map(|f| f.1).collect();
        let recent = (!last.is_empty()).then(|| last.iter().sum::<f32>() / last.len() as f32);
        let mut weak: Vec<&Readiness> = domains.iter().filter(|r| r.weak()).collect();
        weak.sort_by_key(|r| std::cmp::Reverse(r.weight));
        let weak_titles = weak
            .iter()
            .map(|r| DOMAINS.iter().find(|d| d.id == r.domain).map_or(r.title.as_str(), |d| d.short))
            .collect::<Vec<_>>()
            .join(" and ");
        let untaken = catalog.exams.len().saturating_sub(fresh.len());
        let (verdict, advice) = match recent {
            None if coverage < 60.0 => (
                Verdict::KeepStudying,
                "Work through the CKAD lessons and their labs; take the first timed mock at about 60%.".to_string(),
            ),
            None => (
                Verdict::TakeAMock,
                "Take a timed mock exam: only a fresh, timed attempt shows how you do under exam conditions."
                    .to_string(),
            ),
            Some(s) if s < PASS_MARK => (
                Verdict::NotYet,
                if weak.is_empty() {
                    "Redo the labs of the tasks you missed (Review › Drills), then take another fresh mock.".to_string()
                } else {
                    format!(
                        "Focus on {weak_titles}: drill the missed tasks and redo those labs, then take a fresh mock."
                    )
                },
            ),
            Some(s) if s >= READY_MARK && fresh.len() >= 2 && weak.is_empty() && coverage >= 90.0 => (
                Verdict::Ready,
                "Book the exam, and do the two killer.sh sessions that come with it in the last week.".to_string(),
            ),
            Some(_) => (
                Verdict::Close,
                if !weak.is_empty() {
                    format!("Passing, but {weak_titles} is under the pass mark: drill it before the next mock.")
                } else if coverage < 90.0 {
                    "Passing mocks: finish the remaining CKAD lessons to close the gaps.".to_string()
                } else if fresh.len() < 2 {
                    "One passing mock: confirm it with another fresh mock.".to_string()
                } else if untaken > 0 {
                    "Passing, but without margin: aim for 80%+ on a fresh mock.".to_string()
                } else {
                    "Passing without margin and no fresh mock left: drill every missed task until it is quick."
                        .to_string()
                },
            ),
        };
        ExamReadiness {
            verdict,
            coverage,
            fresh,
            recent,
            weak: weak.iter().map(|r| r.domain.clone()).collect(),
            advice,
        }
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
            requires: vec![],
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
                        negatives: vec![],
                        alternatives: vec![],
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

    fn attempt(exam: &str, percent: f32, finished_at: i64, domain: &str) -> ExamAttempt {
        let mut domains = BTreeMap::new();
        domains.insert(domain.to_string(), crate::exam::DomainScore { earned: percent, possible: 100.0 });
        ExamAttempt { exam_id: exam.into(), percent, finished_at, domains, ..Default::default() }
    }

    #[test]
    fn readiness_counts_fresh_mocks_only() {
        let mut c = Catalog::default();
        for e in ["m1", "m2", "m3"] {
            c.exams.push(crate::content::Exam {
                id: e.into(),
                title: e.into(),
                summary: String::new(),
                minutes: 120,
                pass_percent: 66,
                requires: vec![],
                setup: vec![],
                cleanup: vec![],
                tasks: vec![],
            });
        }
        let mut p = Progress::default();
        assert_eq!(p.exam_readiness(&c).verdict, Verdict::KeepStudying);
        p.exams.push(attempt("m1", 50.0, 1, "deployment"));
        p.exams.push(attempt("m1", 95.0, 2, "deployment"));
        let r = p.exam_readiness(&c);
        assert_eq!(r.verdict, Verdict::NotYet, "a retake must not count");
        assert_eq!(r.recent, Some(50.0));
        assert_eq!(r.weak, vec!["deployment".to_string()]);
        p.exams.push(attempt("m2", 90.0, 3, "deployment"));
        let r = p.exam_readiness(&c);
        assert_eq!(r.recent, Some(70.0));
        assert_eq!(r.verdict, Verdict::Close);
        p.exams.push(attempt("m3", 90.0, 4, "deployment"));
        let r = p.exam_readiness(&c);
        assert_eq!(r.recent, Some(90.0));
        assert!(r.weak.is_empty());
        assert_eq!(r.verdict, Verdict::Close, "no lessons done: not ready to book yet");
    }

    #[test]
    fn missed_exam_tasks_become_due_drills() {
        let task = |id: &str| crate::content::ExamTask {
            id: id.into(),
            domain: "deployment".into(),
            weight: 10,
            title: id.into(),
            namespace: None,
            task: String::new(),
            setup: vec![],
            solution: String::new(),
            checks: vec![Check::default()],
            negatives: vec![],
            alternatives: vec![],
        };
        let mut c = Catalog::default();
        c.exams.push(crate::content::Exam {
            id: "m1".into(),
            title: "M1".into(),
            summary: String::new(),
            minutes: 120,
            pass_percent: 66,
            requires: vec![],
            setup: vec![],
            cleanup: vec![],
            tasks: vec![task("q1"), task("q2")],
        });
        let mut a = attempt("m1", 50.0, 100, "deployment");
        a.tasks = vec![
            crate::exam::TaskResult {
                id: "q1".into(),
                domain: "deployment".into(),
                earned: 10.0,
                possible: 10.0,
                checks: vec![],
            },
            crate::exam::TaskResult {
                id: "q2".into(),
                domain: "deployment".into(),
                earned: 5.0,
                possible: 10.0,
                checks: vec![],
            },
        ];
        let mut p = Progress::default();
        p.record_exam(a, &c);
        let due = p.deck.due(&c, 100 + 3600);
        assert_eq!(due.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>(), vec!["t:m1/q2"]);
        assert_eq!(p.deck.stats(&c, 100 + 3600).due_tasks, 1);
    }

    #[test]
    fn quiz_answers_feed_the_deck() {
        let l = lesson("a", true, 0);
        let mut c = Catalog::default();
        c.lessons.insert("a".into(), l.clone());
        let mut p = Progress::default();
        let score = p.record_quiz_answers(&l, &[Some(1)], 0);
        assert_eq!(score, QuizScore { correct: 0, total: 1 });
        assert_eq!(p.deck.stats(&c, crate::review::RELEARN_SECS).due_questions, 1);
        let key = p.deck.due(&c, crate::review::RELEARN_SECS)[0].0.clone();
        p.record_review(&key, true, crate::review::RELEARN_SECS);
        assert_eq!(p.deck.stats(&c, crate::review::RELEARN_SECS).due_questions, 0);
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
