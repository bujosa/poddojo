//! Scoring a mock exam the way the CKAD is scored: every task carries a weight, partial credit is
//! given per passing check, and the result rolls up per curriculum domain.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::check::CheckResult;
use crate::content::Exam;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TaskResult {
    pub id: String,
    pub domain: String,
    pub earned: f32,
    pub possible: f32,
    pub checks: Vec<CheckResult>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct DomainScore {
    pub earned: f32,
    pub possible: f32,
}

impl DomainScore {
    pub fn percent(&self) -> f32 {
        if self.possible > 0.0 { 100.0 * self.earned / self.possible } else { 0.0 }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ExamAttempt {
    pub exam_id: String,
    pub title: String,
    pub started_at: i64,
    pub finished_at: i64,
    pub percent: f32,
    pub passed: bool,
    pub domains: BTreeMap<String, DomainScore>,
    pub tasks: Vec<TaskResult>,
}

impl ExamAttempt {
    pub fn minutes_used(&self) -> i64 {
        (self.finished_at - self.started_at).max(0) / 60
    }
}

/// Score `exam` from each task's check results (`results[task_id]`); a task without results
/// earns nothing.
pub fn grade(
    exam: &Exam,
    results: &BTreeMap<String, Vec<CheckResult>>,
    started_at: i64,
    finished_at: i64,
) -> ExamAttempt {
    let mut domains: BTreeMap<String, DomainScore> = BTreeMap::new();
    let mut tasks = Vec::new();
    let (mut earned, mut possible) = (0.0f32, 0.0f32);
    for task in &exam.tasks {
        let checks = results.get(&task.id).cloned().unwrap_or_default();
        let weight = task.weight as f32;
        let total = task.checks.len().max(1) as f32;
        let passed = checks.iter().filter(|c| c.passed).count() as f32;
        let got = weight * passed.min(total) / total;
        earned += got;
        possible += weight;
        let d = domains.entry(task.domain.clone()).or_default();
        d.earned += got;
        d.possible += weight;
        tasks.push(TaskResult {
            id: task.id.clone(),
            domain: task.domain.clone(),
            earned: got,
            possible: weight,
            checks,
        });
    }
    let percent = if possible > 0.0 { 100.0 * earned / possible } else { 0.0 };
    ExamAttempt {
        exam_id: exam.id.clone(),
        title: exam.title.clone(),
        started_at,
        finished_at,
        percent,
        passed: percent + f32::EPSILON >= exam.pass_percent as f32,
        domains,
        tasks,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::check::Check;
    use crate::content::ExamTask;

    fn task(id: &str, domain: &str, weight: u32, checks: usize) -> ExamTask {
        ExamTask {
            id: id.into(),
            domain: domain.into(),
            weight,
            title: id.into(),
            namespace: None,
            task: String::new(),
            setup: vec![],
            solution: String::new(),
            checks: (0..checks)
                .map(|i| Check { name: format!("c{i}"), run: "true".into(), ..Default::default() })
                .collect(),
        }
    }

    fn res(passed: &[bool]) -> Vec<CheckResult> {
        passed.iter().map(|p| CheckResult { name: String::new(), passed: *p, detail: String::new() }).collect()
    }

    #[test]
    fn partial_credit_and_domains() {
        let exam = Exam {
            id: "e".into(),
            title: "E".into(),
            summary: String::new(),
            minutes: 120,
            pass_percent: 66,
            setup: vec![],
            cleanup: vec![],
            tasks: vec![task("a", "deployment", 10, 2), task("b", "networking", 10, 1)],
        };
        let mut r = BTreeMap::new();
        r.insert("a".to_string(), res(&[true, false]));
        r.insert("b".to_string(), res(&[true]));
        let g = grade(&exam, &r, 0, 600);
        assert!((g.percent - 75.0).abs() < 0.01);
        assert!(g.passed);
        assert!((g.domains["deployment"].percent() - 50.0).abs() < 0.01);
        assert_eq!(g.minutes_used(), 10);
        let g = grade(&exam, &BTreeMap::new(), 0, 0);
        assert!(!g.passed);
        assert_eq!(g.percent, 0.0);
    }
}
