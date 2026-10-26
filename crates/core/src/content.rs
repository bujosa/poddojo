//! The curriculum: tracks of lessons (concepts + quiz + hands-on lab) and timed mock exams, read
//! from `content/` (see docs/content-authoring.md for the format).
//!
//! ```text
//! content/<track>/track.toml        track metadata and its modules, in order
//! content/<track>/NN-<slug>.md      one lesson: TOML front matter between `+++` lines, then Markdown
//! content/exams/<id>.toml           one mock exam
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::check::Check;
use crate::domains;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Module {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TrackFile {
    id: String,
    title: String,
    #[serde(default)]
    summary: String,
    order: u32,
    modules: Vec<Module>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Track {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub order: u32,
    pub modules: Vec<Module>,
    /// Lesson ids in study order (modules in track order, files by name within a module).
    pub lessons: Vec<String>,
}

/// What a lesson's lab needs from the environment.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Needs {
    /// Nothing: a reading-only lesson.
    None,
    /// A Docker engine (the selected machine).
    #[default]
    Docker,
    /// The PodDojo lab cluster (kind on the selected machine).
    Cluster,
}

/// Something a lab or exam needs from the lab cluster beyond `needs = "cluster"`. Checked (or
/// installed) before setup, so the learner gets a clear message instead of a confusing failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Requirement {
    /// A worker node besides the control plane (scheduling, spreading, node failure drills).
    TwoNodes,
    /// PodDojo's Traefik Ingress controller in namespace `dojo-ingress`, installed on demand.
    IngressController,
}

/// An extra run of a step's (or task's) checks that `dojo verify` performs from the state the step
/// starts in, before the worked solution: a negative case (incomplete or wrong work the checks must
/// reject) or an alternative (different, equally correct work the checks must accept).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    /// What this work does, in plain words ("uninstalls but keeps the release history").
    pub name: String,
    /// Shell commands (`set -e`) that produce the case's state.
    pub run: String,
    /// Shell commands that bring back the state the step starts in, so the next case and the worked
    /// solution start clean. Empty when the solution overwrites the case's state anyway.
    #[serde(default)]
    pub undo: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuizQuestion {
    pub q: String,
    pub choices: Vec<String>,
    /// Index into `choices`.
    pub answer: usize,
    /// Why the answer is right (shown after answering).
    pub why: String,
}

impl QuizQuestion {
    /// The order to show the choices in: a fixed shuffle per question (salted with the lesson id),
    /// so the correct answer is not always in the position it was authored in.
    pub fn order(&self, salt: &str) -> Vec<usize> {
        // FNV-1a of salt + question, then a tiny LCG for a Fisher-Yates shuffle.
        let mut h: u64 = 0xcbf29ce484222325;
        for b in salt.bytes().chain(self.q.bytes()) {
            h ^= b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        let mut order: Vec<usize> = (0..self.choices.len()).collect();
        for i in (1..order.len()).rev() {
            h = h.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            order.swap(i, (h >> 33) as usize % (i + 1));
        }
        order
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LabStep {
    pub title: String,
    /// What to do (Markdown).
    pub task: String,
    #[serde(default)]
    pub hints: Vec<String>,
    /// A worked solution (Markdown, usually a ```bash block).
    #[serde(default)]
    pub solution: String,
    #[serde(default)]
    pub checks: Vec<Check>,
    /// Incomplete or wrong work the checks must reject (run by `dojo verify`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub negatives: Vec<Case>,
    /// Different correct work the checks must accept (run by `dojo verify`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alternatives: Vec<Case>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lab {
    #[serde(default)]
    pub summary: String,
    /// Commands that prepare the lab (idempotent; run on "Prepare lab").
    #[serde(default)]
    pub setup: Vec<String>,
    /// Commands that remove what the lab created (run on "Reset lab").
    #[serde(default)]
    pub cleanup: Vec<String>,
    /// Files written into the lesson's working directory before setup (name → contents).
    #[serde(default)]
    pub files: BTreeMap<String, String>,
    #[serde(default)]
    pub steps: Vec<LabStep>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FrontMatter {
    id: String,
    title: String,
    module: String,
    #[serde(default = "default_minutes")]
    minutes: u32,
    #[serde(default)]
    domain: Option<String>,
    #[serde(default)]
    needs: Needs,
    #[serde(default)]
    requires: Vec<Requirement>,
    #[serde(default)]
    objectives: Vec<String>,
    #[serde(default)]
    quiz: Vec<QuizQuestion>,
    #[serde(default)]
    lab: Option<Lab>,
}

fn default_minutes() -> u32 {
    20
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Lesson {
    pub id: String,
    pub track: String,
    pub module: String,
    pub title: String,
    pub minutes: u32,
    /// CKAD domain id (`domains::DOMAINS`), when the lesson maps to one.
    pub domain: Option<String>,
    pub needs: Needs,
    /// Extra lab cluster requirements (a worker node, the Ingress controller).
    pub requires: Vec<Requirement>,
    pub objectives: Vec<String>,
    /// The concepts, in Markdown.
    pub body: String,
    pub quiz: Vec<QuizQuestion>,
    pub lab: Option<Lab>,
    /// Source file, relative to `content/`.
    pub path: String,
}

impl Lesson {
    pub fn has_lab(&self) -> bool {
        self.lab.as_ref().is_some_and(|l| !l.steps.is_empty())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExamTask {
    pub id: String,
    pub domain: String,
    /// Points; the exam score is earned points over total points.
    pub weight: u32,
    pub title: String,
    /// The namespace the task works in, if any (shown as context, created by setup).
    #[serde(default)]
    pub namespace: Option<String>,
    /// What to do (Markdown).
    pub task: String,
    #[serde(default)]
    pub setup: Vec<String>,
    #[serde(default)]
    pub solution: String,
    pub checks: Vec<Check>,
    /// Incomplete or wrong work the checks must reject (run by `dojo verify`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub negatives: Vec<Case>,
    /// Different correct work the checks must accept (run by `dojo verify`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alternatives: Vec<Case>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Exam {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub summary: String,
    pub minutes: u32,
    pub pass_percent: u32,
    /// Extra lab cluster requirements, checked or installed before the exam setup.
    #[serde(default)]
    pub requires: Vec<Requirement>,
    #[serde(default)]
    pub setup: Vec<String>,
    #[serde(default)]
    pub cleanup: Vec<String>,
    pub tasks: Vec<ExamTask>,
}

impl Exam {
    pub fn total_weight(&self) -> u32 {
        self.tasks.iter().map(|t| t.weight).sum()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentError {
    pub path: String,
    pub message: String,
}

impl std::fmt::Display for ContentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.path, self.message)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Catalog {
    pub tracks: Vec<Track>,
    pub lessons: BTreeMap<String, Lesson>,
    pub exams: Vec<Exam>,
}

/// Split `+++\n<toml>\n+++\n<markdown>`.
fn split_front_matter(text: &str) -> Option<(&str, &str)> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let rest = text.strip_prefix("+++\n").or_else(|| text.strip_prefix("+++\r\n"))?;
    let mut offset = 0;
    for line in rest.split_inclusive('\n') {
        if line.trim_end() == "+++" {
            let fm = rest.get(..offset)?;
            let body = rest.get(offset + line.len()..)?;
            return Some((fm, body));
        }
        offset += line.len();
    }
    None
}

impl Catalog {
    /// The curriculum compiled into the binary, or the one in `PODDOJO_CONTENT_DIR` when set.
    pub fn load() -> Result<Self, Vec<ContentError>> {
        match std::env::var_os("PODDOJO_CONTENT_DIR") {
            Some(dir) if !dir.is_empty() => Self::from_dir(Path::new(&dir)),
            _ => Self::embedded(),
        }
    }

    pub fn embedded() -> Result<Self, Vec<ContentError>> {
        Self::from_files(crate::embedded_files().iter().map(|(p, t)| ((*p).to_string(), (*t).to_string())))
    }

    pub fn from_dir(dir: &Path) -> Result<Self, Vec<ContentError>> {
        let mut files = Vec::new();
        let mut errors = Vec::new();
        collect(dir, dir, &mut files, &mut errors);
        if !errors.is_empty() {
            return Err(errors);
        }
        Self::from_files(files)
    }

    /// Build and validate a catalog from `(path relative to content/, text)` pairs.
    pub fn from_files(files: impl IntoIterator<Item = (String, String)>) -> Result<Self, Vec<ContentError>> {
        let mut errors = Vec::new();
        let mut tracks: Vec<(String, TrackFile)> = Vec::new();
        let mut lessons: Vec<Lesson> = Vec::new();
        let mut exams: Vec<(String, Exam)> = Vec::new();
        let mut files: Vec<(String, String)> = files.into_iter().collect();
        files.sort();
        let err = |path: &str, message: String| ContentError { path: path.to_string(), message };

        for (path, text) in &files {
            let parts: Vec<&str> = path.split('/').collect();
            match parts.as_slice() {
                ["exams", name] if name.ends_with(".toml") => match toml::from_str::<Exam>(text) {
                    Ok(e) => exams.push((path.clone(), e)),
                    Err(e) => errors.push(err(path, e.to_string())),
                },
                [_, "track.toml"] => match toml::from_str::<TrackFile>(text) {
                    Ok(t) => tracks.push((path.clone(), t)),
                    Err(e) => errors.push(err(path, e.to_string())),
                },
                [track, name] if name.ends_with(".md") => {
                    let Some((fm, body)) = split_front_matter(text) else {
                        errors.push(err(path, "missing `+++` TOML front matter".into()));
                        continue;
                    };
                    match toml::from_str::<FrontMatter>(fm) {
                        Ok(f) => lessons.push(Lesson {
                            id: f.id,
                            track: (*track).to_string(),
                            module: f.module,
                            title: f.title,
                            minutes: f.minutes,
                            domain: f.domain,
                            needs: f.needs,
                            requires: f.requires,
                            objectives: f.objectives,
                            body: body.trim().to_string(),
                            quiz: f.quiz,
                            lab: f.lab,
                            path: path.clone(),
                        }),
                        Err(e) => errors.push(err(path, e.to_string())),
                    }
                }
                _ if path.ends_with("README.md") => {}
                _ => errors.push(err(path, "unexpected file (see docs/content-authoring.md)".into())),
            }
        }

        // Tracks.
        let mut catalog = Catalog::default();
        let mut track_dirs = BTreeSet::new();
        for (path, t) in tracks {
            let dir = path.split('/').next().unwrap_or_default().to_string();
            if dir != t.id {
                errors.push(err(&path, format!("track id `{}` must match its folder `{dir}`", t.id)));
            }
            track_dirs.insert(dir);
            let mut seen = BTreeSet::new();
            for m in &t.modules {
                if !seen.insert(m.id.clone()) {
                    errors.push(err(&path, format!("duplicate module id `{}`", m.id)));
                }
            }
            catalog.tracks.push(Track {
                id: t.id,
                title: t.title,
                summary: t.summary,
                order: t.order,
                modules: t.modules,
                lessons: Vec::new(),
            });
        }
        catalog.tracks.sort_by(|a, b| a.order.cmp(&b.order).then(a.id.cmp(&b.id)));

        // Lessons.
        for lesson in lessons {
            for problem in lesson_problems(&lesson) {
                errors.push(err(&lesson.path, problem));
            }
            let Some(track) = catalog.tracks.iter().find(|t| t.id == lesson.track) else {
                errors.push(err(&lesson.path, format!("no track.toml for folder `{}`", lesson.track)));
                continue;
            };
            if !track.modules.iter().any(|m| m.id == lesson.module) {
                errors.push(err(
                    &lesson.path,
                    format!("module `{}` is not listed in {}/track.toml", lesson.module, track.id),
                ));
            }
            if let Some(prev) = catalog.lessons.get(&lesson.id) {
                errors.push(err(&lesson.path, format!("duplicate lesson id `{}` (also {})", lesson.id, prev.path)));
                continue;
            }
            catalog.lessons.insert(lesson.id.clone(), lesson);
        }
        for track in &mut catalog.tracks {
            let mut ordered: Vec<&Lesson> = catalog.lessons.values().filter(|l| l.track == track.id).collect();
            let module_index = |m: &str| track.modules.iter().position(|x| x.id == m).unwrap_or(usize::MAX);
            ordered.sort_by(|a, b| module_index(&a.module).cmp(&module_index(&b.module)).then(a.path.cmp(&b.path)));
            track.lessons = ordered.iter().map(|l| l.id.clone()).collect();
        }

        // Exams.
        let mut exam_ids = BTreeSet::new();
        for (path, exam) in exams {
            for problem in exam_problems(&exam) {
                errors.push(err(&path, problem));
            }
            if !exam_ids.insert(exam.id.clone()) {
                errors.push(err(&path, format!("duplicate exam id `{}`", exam.id)));
            }
            catalog.exams.push(exam);
        }
        catalog.exams.sort_by(|a, b| a.id.cmp(&b.id));

        if errors.is_empty() { Ok(catalog) } else { Err(errors) }
    }

    pub fn lesson(&self, id: &str) -> Option<&Lesson> {
        self.lessons.get(id)
    }

    pub fn exam(&self, id: &str) -> Option<&Exam> {
        self.exams.iter().find(|e| e.id == id)
    }

    pub fn track(&self, id: &str) -> Option<&Track> {
        self.tracks.iter().find(|t| t.id == id)
    }

    /// Every lesson in study order: tracks by `order`, then each track's own order.
    pub fn ordered_lessons(&self) -> impl Iterator<Item = &Lesson> {
        self.tracks.iter().flat_map(|t| t.lessons.iter()).filter_map(|id| self.lessons.get(id))
    }

    pub fn module_title<'a>(&'a self, lesson: &'a Lesson) -> &'a str {
        self.track(&lesson.track)
            .and_then(|t| t.modules.iter().find(|m| m.id == lesson.module))
            .map_or(lesson.module.as_str(), |m| m.title.as_str())
    }

    /// The lesson after `id` in study order.
    pub fn next_lesson(&self, id: &str) -> Option<&Lesson> {
        let mut it = self.ordered_lessons().skip_while(|l| l.id != id);
        it.next()?;
        it.next()
    }

    pub fn previous_lesson(&self, id: &str) -> Option<&Lesson> {
        let mut prev = None;
        for l in self.ordered_lessons() {
            if l.id == id {
                return prev;
            }
            prev = Some(l);
        }
        None
    }
}

fn collect(root: &Path, dir: &Path, out: &mut Vec<(String, String)>, errors: &mut Vec<ContentError>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            errors.push(ContentError { path: dir.display().to_string(), message: e.to_string() });
            return;
        }
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(root, &path, out, errors);
            continue;
        }
        if !matches!(path.extension().and_then(|e| e.to_str()), Some("md" | "toml")) {
            continue;
        }
        let rel = path.strip_prefix(root).unwrap_or(&path).to_string_lossy().replace('\\', "/");
        match std::fs::read_to_string(&path) {
            Ok(text) => out.push((rel, text)),
            Err(e) => errors.push(ContentError { path: rel, message: e.to_string() }),
        }
    }
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn domain_problem(domain: &str) -> Option<String> {
    if domains::domain(domain).is_some() {
        return None;
    }
    let known: Vec<&str> = domains::DOMAINS.iter().map(|d| d.id).collect();
    Some(format!("unknown domain `{domain}` (one of: {})", known.join(", ")))
}

/// Problems with a step's or task's verification cases (`where_` says which step or task).
fn case_problems(where_: &str, negatives: &[Case], alternatives: &[Case]) -> Vec<String> {
    let mut out = Vec::new();
    for (kind, c) in negatives.iter().map(|c| ("negative", c)).chain(alternatives.iter().map(|c| ("alternative", c))) {
        if c.name.trim().is_empty() {
            out.push(format!("{where_}: a {kind} case has no name"));
        }
        if c.run.trim().is_empty() {
            out.push(format!("{where_}: {kind} case `{}` has nothing to run", c.name));
        }
        for cmd in [&c.run, &c.undo] {
            if let Some(why) = crate::check::unsafe_command(cmd) {
                out.push(format!("{where_}: {kind} case `{}` command {why}", c.name));
            }
        }
    }
    out
}

fn lesson_problems(l: &Lesson) -> Vec<String> {
    let mut out = Vec::new();
    if !valid_id(&l.id) {
        out.push(format!("lesson id `{}` must be lowercase letters, digits and dashes", l.id));
    }
    if l.title.trim().is_empty() {
        out.push("lesson has no title".into());
    }
    if l.body.trim().len() < 200 {
        out.push("lesson body is too short to teach anything (under 200 characters)".into());
    }
    if let Some(d) = &l.domain
        && let Some(p) = domain_problem(d)
    {
        out.push(p);
    }
    for (i, q) in l.quiz.iter().enumerate() {
        if q.choices.len() < 2 {
            out.push(format!("quiz question {} needs at least two choices", i + 1));
        }
        if q.answer >= q.choices.len() {
            out.push(format!("quiz question {} answer index {} is out of range", i + 1, q.answer));
        }
        if q.why.trim().is_empty() {
            out.push(format!("quiz question {} has no `why`", i + 1));
        }
    }
    if let Some(lab) = &l.lab {
        for cmd in lab.setup.iter().chain(&lab.cleanup) {
            if let Some(why) = crate::check::unsafe_command(cmd) {
                out.push(format!("lab command `{cmd}` {why}"));
            }
        }
        if !l.requires.is_empty() && l.needs != Needs::Cluster {
            out.push("`requires` only applies to lessons with `needs = \"cluster\"`".into());
        }
        if l.needs == Needs::None && !lab.steps.is_empty() {
            out.push("a lesson with lab steps cannot have `needs = \"none\"`".into());
        }
        for name in lab.files.keys() {
            if name.contains("..") || name.starts_with('/') || name.is_empty() {
                out.push(format!("lab file name `{name}` must be a relative path inside the lab folder"));
            }
        }
        for (i, s) in lab.steps.iter().enumerate() {
            if s.checks.is_empty() {
                out.push(format!("lab step {} (`{}`) has no checks", i + 1, s.title));
            }
            for c in &s.checks {
                out.extend(c.problems().into_iter().map(|p| format!("lab step {}: {p}", i + 1)));
            }
            out.extend(case_problems(&format!("lab step {}", i + 1), &s.negatives, &s.alternatives));
        }
    }
    out
}

fn exam_problems(e: &Exam) -> Vec<String> {
    let mut out = Vec::new();
    if !valid_id(&e.id) {
        out.push(format!("exam id `{}` must be lowercase letters, digits and dashes", e.id));
    }
    if e.tasks.is_empty() {
        out.push("exam has no tasks".into());
    }
    if e.pass_percent == 0 || e.pass_percent > 100 {
        out.push("pass_percent must be 1..=100".into());
    }
    for cmd in e.setup.iter().chain(&e.cleanup).chain(e.tasks.iter().flat_map(|t| &t.setup)) {
        if let Some(why) = crate::check::unsafe_command(cmd) {
            out.push(format!("command `{cmd}` {why}"));
        }
    }
    let mut ids = BTreeSet::new();
    for t in &e.tasks {
        if !ids.insert(t.id.clone()) {
            out.push(format!("duplicate task id `{}`", t.id));
        }
        if let Some(p) = domain_problem(&t.domain) {
            out.push(format!("task {}: {p}", t.id));
        }
        if t.weight == 0 {
            out.push(format!("task {} has weight 0", t.id));
        }
        if t.checks.is_empty() {
            out.push(format!("task {} has no checks", t.id));
        }
        for c in &t.checks {
            out.extend(c.problems().into_iter().map(|p| format!("task {}: {p}", t.id)));
        }
        out.extend(case_problems(&format!("task {}", t.id), &t.negatives, &t.alternatives));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const TRACK: &str = r#"
id = "docker"
title = "Docker"
order = 1
[[modules]]
id = "basics"
title = "Basics"
"#;

    fn lesson(id: &str) -> String {
        format!(
            r#"+++
id = "{id}"
title = "A lesson"
module = "basics"

[[quiz]]
q = "?"
choices = ["a", "b"]
answer = 1
why = "because"

[lab]
[[lab.steps]]
title = "Do it"
task = "Run it"
[[lab.steps.checks]]
name = "ran"
run = "true"
+++
# Concepts

{}
"#,
            "Containers are processes. ".repeat(20)
        )
    }

    #[test]
    fn embedded_content_is_valid() {
        let catalog = match Catalog::embedded() {
            Ok(c) => c,
            Err(errors) => {
                panic!("content errors:\n{}", errors.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("\n"))
            }
        };
        assert!(!catalog.tracks.is_empty());
    }

    #[test]
    fn parses_tracks_lessons_and_order() {
        let files = vec![
            ("docker/track.toml".to_string(), TRACK.to_string()),
            ("docker/02-b.md".to_string(), lesson("b")),
            ("docker/01-a.md".to_string(), lesson("a")),
        ];
        let c = Catalog::from_files(files).unwrap();
        assert_eq!(c.tracks[0].lessons, vec!["a", "b"]);
        assert_eq!(c.next_lesson("a").map(|l| l.id.as_str()), Some("b"));
        assert_eq!(c.previous_lesson("b").map(|l| l.id.as_str()), Some("a"));
        assert!(c.lesson("a").unwrap().has_lab());
    }

    #[test]
    fn reports_problems() {
        let bad = lesson("a").replace("answer = 1", "answer = 5").replace("module = \"basics\"", "module = \"nope\"");
        let files = vec![("docker/track.toml".to_string(), TRACK.to_string()), ("docker/01-a.md".to_string(), bad)];
        let errors = Catalog::from_files(files).unwrap_err();
        assert!(errors.iter().any(|e| e.message.contains("out of range")));
        assert!(errors.iter().any(|e| e.message.contains("module `nope`")));
    }

    #[test]
    fn quiz_order_is_a_stable_permutation_that_spreads_answers() {
        let q = QuizQuestion {
            q: "x".into(),
            choices: vec!["a".into(), "b".into(), "c".into(), "d".into()],
            answer: 1,
            why: String::new(),
        };
        let o = q.order("lesson");
        assert_eq!(o, q.order("lesson"));
        let mut sorted = o.clone();
        sorted.sort();
        assert_eq!(sorted, vec![0, 1, 2, 3]);
        let catalog = Catalog::embedded().unwrap();
        let mut at = [0usize; 4];
        for l in catalog.lessons.values() {
            for q in &l.quiz {
                if let Some(pos) = q.order(&l.id).iter().position(|&j| j == q.answer) {
                    at[pos.min(3)] += 1;
                }
            }
        }
        let total: usize = at.iter().sum();
        assert!(at.iter().all(|&n| n * 100 / total.max(1) < 40), "answers still bunched: {at:?}");
    }

    #[test]
    fn parses_requirements_and_verification_cases() {
        let text = lesson("a").replace("module = \"basics\"", "module = \"basics\"\nneeds = \"cluster\"\nrequires = [\"two-nodes\", \"ingress-controller\"]").replace(
            "run = \"true\"\n",
            "run = \"true\"\n[[lab.steps.negatives]]\nname = \"shortcut\"\nrun = \"echo half\"\n[[lab.steps.alternatives]]\nname = \"other way\"\nrun = \"echo other\"\nundo = \"echo back\"\n",
        );
        let files = vec![("docker/track.toml".to_string(), TRACK.to_string()), ("docker/01-a.md".to_string(), text)];
        let c = Catalog::from_files(files).unwrap();
        let l = c.lesson("a").unwrap();
        assert_eq!(l.requires, vec![Requirement::TwoNodes, Requirement::IngressController]);
        let step = &l.lab.as_ref().unwrap().steps[0];
        assert_eq!(step.negatives[0].name, "shortcut");
        assert_eq!(step.alternatives[0].undo, "echo back");
    }

    #[test]
    fn rejects_bad_requirements_and_cases() {
        let text =
            lesson("a").replace("module = \"basics\"", "module = \"basics\"\nrequires = [\"two-nodes\"]").replace(
                "run = \"true\"\n",
                "run = \"true\"\n[[lab.steps.negatives]]\nname = \"\"\nrun = \"docker system prune -f\"\n",
            );
        let files = vec![("docker/track.toml".to_string(), TRACK.to_string()), ("docker/01-a.md".to_string(), text)];
        let errors = Catalog::from_files(files).unwrap_err();
        assert!(errors.iter().any(|e| e.message.contains("`requires` only applies")));
        assert!(errors.iter().any(|e| e.message.contains("has no name")));
        assert!(errors.iter().any(|e| e.message.contains("must not prune")));
        let unknown = lesson("b").replace("module = \"basics\"", "module = \"basics\"\nrequires = [\"gpu\"]");
        let files = vec![("docker/track.toml".to_string(), TRACK.to_string()), ("docker/01-b.md".to_string(), unknown)];
        assert!(Catalog::from_files(files).is_err());
    }

    #[test]
    fn front_matter_needs_closing_fence() {
        assert!(split_front_matter("+++\nid = 1\n").is_none());
        assert_eq!(split_front_matter("+++\na = 1\n+++\nbody"), Some(("a = 1\n", "body")));
    }
}
