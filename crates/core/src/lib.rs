//! PodDojo core: the curriculum (tracks, lessons, quizzes, labs, exams), the learner's progress and
//! the grading rules. Pure data and logic: running commands against a Docker host or the lab cluster
//! lives in `poddojo-runner`, drawing it in `poddojo-ui`.

pub mod check;
pub mod content;
pub mod domains;
pub mod exam;
pub mod progress;
pub mod review;

mod embedded {
    include!(concat!(env!("OUT_DIR"), "/embedded.rs"));
}

pub use check::{Check, CheckResult, CommandOutput};
pub use content::{
    Case, Catalog, ContentError, Exam, ExamTask, Lab, LabStep, Lesson, Module, Needs, QuizQuestion, RefPage,
    Requirement, Track,
};
pub use domains::{CKAD_CURRICULUM_VERSION, DOMAINS, Domain};
pub use exam::{DomainScore, ExamAttempt, TaskResult};
pub use progress::{
    ActiveExam, ExamReadiness, LessonProgress, LessonStatus, PASS_MARK, Progress, QuizScore, READY_MARK, Readiness,
    Verdict,
};
pub use review::{Card, Deck, DeckStats, Item};

/// The curriculum compiled into this binary (`content/`).
pub fn embedded_files() -> &'static [(&'static str, &'static str)] {
    embedded::FILES
}

/// Seconds since the Unix epoch, now.
pub fn now() -> i64 {
    chrono::Utc::now().timestamp()
}
