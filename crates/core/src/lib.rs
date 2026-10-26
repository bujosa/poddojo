//! PodDojo core: the curriculum (tracks, lessons, quizzes, labs, exams), the learner's progress and
//! the grading rules. Pure data and logic: running commands against a Docker host or the lab cluster
//! lives in `poddojo-runner`, drawing it in `poddojo-ui`.

pub mod check;
pub mod content;
pub mod domains;
pub mod exam;
pub mod progress;

mod embedded {
    include!(concat!(env!("OUT_DIR"), "/embedded.rs"));
}

pub use check::{Check, CheckResult, CommandOutput};
pub use content::{
    Case, Catalog, ContentError, Exam, ExamTask, Lab, LabStep, Lesson, Module, Needs, QuizQuestion, Requirement, Track,
};
pub use domains::{CKAD_CURRICULUM_VERSION, DOMAINS, Domain};
pub use exam::{DomainScore, ExamAttempt, TaskResult};
pub use progress::{ActiveExam, LessonProgress, LessonStatus, Progress, QuizScore, Readiness};

/// The curriculum compiled into this binary (`content/`).
pub fn embedded_files() -> &'static [(&'static str, &'static str)] {
    embedded::FILES
}

/// Seconds since the Unix epoch, now.
pub fn now() -> i64 {
    chrono::Utc::now().timestamp()
}
