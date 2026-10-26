//! `dojo`: PodDojo from the terminal. The same progress file as the app, so you can switch between
//! them at any time. `dojo help` lists the commands.

use std::collections::BTreeMap;
use std::io::{BufRead, Write};
use std::path::Path;
use std::process::ExitCode;
use std::time::Duration;

use poddojo_core::{
    Case, Catalog, Check, CheckResult, Exam, Lesson, LessonStatus, Needs, Progress, QuizScore, exam, now,
};
use poddojo_runner::profiles::{self, Profile, Profiles};
use poddojo_runner::{LabEnv, Paths, Settings, Target, cluster, doctor, lab, targets, terminal};

const HELP: &str = "\
dojo: PodDojo from the terminal (Docker + CKAD hands-on dojo)

Study
  status [--json]          Where you are: resume point, completion, CKAD readiness
  next                     The lesson to continue with
  lessons                  Every lesson with its status
  show <lesson>            Print a lesson (concepts, objectives, lab tasks)
  quiz <lesson>            Take the lesson's quiz here
  prepare <lesson>         Write the lab files and run its setup
  check <lesson> [step]    Grade the lab (all steps, or one, numbered from 1)
  reset <lesson>           Clean up what the lab created and clear its lab progress
  read <lesson>            Mark the concepts as read

Exams (timed, performance-based, like the real CKAD)
  exam list                Mock exams and your best scores
  exam start <id>          Run the exam setup and start the clock
  exam status              Time left and the task list of the running exam
  exam grade               Stop the clock and grade every task
  exam abandon             Stop without grading

Profiles (the app's \"Who's training today?\")
  profiles                 List profiles; the current one is marked
  profile new <name>       Create a profile and switch to it
  profile use <id>         Study as another profile (or set PODDOJO_PROFILE; `guest` for guest mode)

Environment
  targets                  Machines that can run Docker (your Docker contexts)
  use <machine>            Run labs on that machine
  env [lesson]             Shell exports for the lab: eval \"$(dojo env)\"
  shell [lesson]           Open a terminal window with the lab environment
  cluster up|down|status   The isolated kind lab cluster on the chosen machine
  doctor                   Check docker, kubectl, kind and helm

Authoring
  validate [dir]           Validate content (default: ./content)
  verify <lesson|all|exam:ID[/TASK]> [--keep]
                           Prepare, try the negative and alternative cases, apply the
                           worked solutions, grade, then clean up
";

/// The profile the CLI acts as: `PODDOJO_PROFILE` (id or name), else the one used last in the app,
/// else a new profile named after the person logged in.
fn active_profile(base: &Paths) -> Result<(Paths, Profile), String> {
    let mut profiles = Profiles::load(base);
    let chosen = match std::env::var("PODDOJO_PROFILE") {
        Ok(who) if who == profiles::GUEST => return Ok((profiles.start_guest(), Profile::guest())),
        Ok(who) => {
            Some(profiles.find(&who).cloned().ok_or_else(|| format!("no profile `{who}` (see `dojo profiles`)"))?)
        }
        Err(_) => profiles.last().or_else(|| profiles.list().first()).cloned(),
    };
    let profile = match chosen {
        Some(p) => p,
        None => profiles.create(&profiles::system_name()).map_err(|e| e.to_string())?,
    };
    Ok((profiles.paths_for(&profile), profile))
}

fn profiles_cmd(args: &[&str]) -> Result<(), String> {
    let base = Paths::discover();
    let mut profiles = Profiles::load(&base);
    match args {
        [] => {
            let current = active_profile(&base).map(|(_, p)| p.id).unwrap_or_default();
            for p in profiles.list() {
                let progress = Progress::load(&profiles.paths_for(p).progress()).unwrap_or_default();
                let mark = if p.id == current { "▶" } else { " " };
                println!(
                    "{mark} {:<20} {:<24} last active {}",
                    p.id,
                    p.name,
                    progress.last_active().map_or("never".into(), |t| chrono::DateTime::from_timestamp(t, 0)
                        .map_or(String::new(), |d| d.format("%Y-%m-%d").to_string()))
                );
            }
            println!(
                "
Switch with `dojo profile use <id>`, create with `dojo profile new <name>`, or set PODDOJO_PROFILE."
            );
            Ok(())
        }
        ["new", name @ ..] if !name.is_empty() => {
            let p = profiles.create(&name.join(" ")).map_err(|e| e.to_string())?;
            profiles.set_last(&p.id).map_err(|e| e.to_string())?;
            println!("created `{}` ({}) and made it the current profile", p.name, p.id);
            Ok(())
        }
        ["use", who] => {
            let p = profiles.find(who).cloned().ok_or_else(|| format!("no profile `{who}`"))?;
            profiles.set_last(&p.id).map_err(|e| e.to_string())?;
            println!("now studying as {}", p.name);
            Ok(())
        }
        _ => Err("usage: dojo profiles | dojo profile new <name> | dojo profile use <id>".into()),
    }
}

struct Ctx {
    paths: Paths,
    settings: Settings,
    catalog: Catalog,
    progress: Progress,
}

impl Ctx {
    fn load() -> Result<Self, String> {
        let base = Paths::discover();
        let (paths, _) = active_profile(&base)?;
        let settings = Settings::load(&base.settings());
        let catalog =
            Catalog::load().map_err(|errs| errs.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("\n"))?;
        let progress = Progress::load(&paths.progress()).map_err(|e| format!("could not read progress: {e}"))?;
        Ok(Self { paths, settings, catalog, progress })
    }

    fn save(&self) -> Result<(), String> {
        self.progress.save(&self.paths.progress()).map_err(|e| format!("could not save progress: {e}"))
    }

    fn target(&self) -> Result<Target, String> {
        let all = targets::list(&self.paths)?;
        targets::choose(&all, &self.settings.target)
            .ok_or_else(|| "no Docker contexts found: is Docker installed?".to_string())
    }

    fn env_for(&self, id: &str) -> Result<LabEnv, String> {
        let workdir = self.paths.workdir(id);
        self.paths.ensure(&workdir).map_err(|e| e.to_string())?;
        Ok(self.target()?.env(&self.paths, workdir))
    }

    fn lesson(&self, id: &str) -> Result<&Lesson, String> {
        self.catalog.lesson(id).ok_or_else(|| format!("no lesson `{id}` (see `dojo lessons`)"))
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let words: Vec<&str> = args.iter().map(String::as_str).collect();
    match dispatch(&words) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("dojo: {e}");
            ExitCode::FAILURE
        }
    }
}

fn dispatch(args: &[&str]) -> Result<(), String> {
    match args {
        [] | ["help" | "-h" | "--help"] => {
            print!("{HELP}");
            Ok(())
        }
        ["profiles"] => profiles_cmd(&[]),
        ["profile", rest @ ..] => profiles_cmd(rest),
        ["validate"] => validate(Path::new("content")),
        ["validate", dir] => validate(Path::new(dir)),
        ["status"] => status(&Ctx::load()?, false),
        ["status", "--json"] => status(&Ctx::load()?, true),
        ["next" | "resume"] => next(&Ctx::load()?),
        ["lessons"] => lessons(&Ctx::load()?),
        ["show", id] => show(&Ctx::load()?, id),
        ["read", id] => {
            let mut c = Ctx::load()?;
            let l = c.lesson(id)?.clone();
            c.progress.open_lesson(&l, now());
            c.progress.mark_read(&l, now());
            c.save()?;
            println!("marked `{id}` as read");
            Ok(())
        }
        ["quiz", id] => quiz(&mut Ctx::load()?, id),
        ["prepare", id] => prepare(&mut Ctx::load()?, id),
        ["check", id] => check(&mut Ctx::load()?, id, None),
        ["check", id, step] => {
            check(&mut Ctx::load()?, id, Some(step.parse::<usize>().map_err(|_| "step must be a number")?))
        }
        ["reset", id] => reset(&mut Ctx::load()?, id),
        ["targets"] => list_targets(&Ctx::load()?),
        ["use", name] => use_target(&mut Ctx::load()?, name),
        ["env"] => env(&Ctx::load()?, "shell"),
        ["env", id] => env(&Ctx::load()?, id),
        ["shell"] => shell(&Ctx::load()?, "shell"),
        ["shell", id] => shell(&Ctx::load()?, id),
        ["doctor"] => run_doctor(&Ctx::load()?),
        ["cluster", "status"] => cluster_status(&Ctx::load()?),
        ["cluster", "up"] => cluster_up(&Ctx::load()?),
        ["cluster", "down"] => cluster_down(&Ctx::load()?),
        ["exam", "list"] => exam_list(&Ctx::load()?),
        ["exam", "start", id] => exam_start(&mut Ctx::load()?, id),
        ["exam", "status"] => exam_status(&Ctx::load()?),
        ["exam", "grade"] => exam_grade(&mut Ctx::load()?),
        ["exam", "abandon"] => {
            let mut c = Ctx::load()?;
            c.progress.active_exam = None;
            c.save()?;
            println!("exam abandoned");
            Ok(())
        }
        ["verify", rest @ ..] if !rest.is_empty() => {
            let keep = rest.contains(&"--keep");
            let what: Vec<&str> = rest.iter().copied().filter(|a| *a != "--keep").collect();
            match what.as_slice() {
                [w] => verify(&Ctx::load()?, w, keep),
                _ => Err("usage: dojo verify <lesson|all|docker|ckad|exam:ID[/TASK]> [--keep]".into()),
            }
        }
        _ => Err(format!("unknown command `{}` (try `dojo help`)", args.join(" "))),
    }
}

fn validate(dir: &Path) -> Result<(), String> {
    match Catalog::from_dir(dir) {
        Ok(c) => {
            let steps: usize = c.lessons.values().filter_map(|l| l.lab.as_ref()).map(|l| l.steps.len()).sum();
            let quiz: usize = c.lessons.values().map(|l| l.quiz.len()).sum();
            let tasks: usize = c.exams.iter().map(|e| e.tasks.len()).sum();
            println!(
                "content OK: {} tracks, {} lessons, {} quiz questions, {} lab steps, {} exams ({} tasks)",
                c.tracks.len(),
                c.lessons.len(),
                quiz,
                steps,
                c.exams.len(),
                tasks
            );
            Ok(())
        }
        Err(errors) => {
            for e in &errors {
                eprintln!("✗ {e}");
            }
            Err(format!("{} problem(s) in {}", errors.len(), dir.display()))
        }
    }
}

fn mark(status: LessonStatus) -> &'static str {
    match status {
        LessonStatus::Done => "✓",
        LessonStatus::InProgress => "◐",
        LessonStatus::NotStarted => "·",
    }
}

fn bar(fraction: f32, width: usize) -> String {
    let filled = ((fraction.clamp(0.0, 1.0) * width as f32).round() as usize).min(width);
    format!("{}{}", "█".repeat(filled), "░".repeat(width - filled))
}

fn status(c: &Ctx, json: bool) -> Result<(), String> {
    let (done, total) = c.progress.done_count(&c.catalog);
    let resume = c.progress.resume(&c.catalog);
    let readiness = c.progress.readiness(&c.catalog);
    let overall = c.progress.overall_readiness(&c.catalog);
    let streak = c.progress.streak(chrono::Local::now().date_naive());
    if json {
        let v = serde_json::json!({
            "lessons_done": done,
            "lessons_total": total,
            "resume": resume.map(|l| serde_json::json!({"id": l.id, "title": l.title, "track": l.track})),
            "ckad_readiness_percent": overall,
            "domains": readiness,
            "streak_days": streak,
            "machine": c.settings.target,
            "active_exam": c.progress.active_exam.as_ref().map(|e| &e.exam_id),
            "exam_attempts": c.progress.exams.iter().map(|a| serde_json::json!({"exam": a.exam_id, "percent": a.percent, "passed": a.passed, "finished_at": a.finished_at})).collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string_pretty(&v).map_err(|e| e.to_string())?);
        return Ok(());
    }
    println!("PodDojo · {done}/{total} lessons · streak {streak} day(s)");
    for t in &c.catalog.tracks {
        let (d, n) = c.progress.track_counts(&c.catalog, &t.id);
        let f = if n == 0 { 0.0 } else { d as f32 / n as f32 };
        println!("  {} {:>3}/{:<3} {}", bar(f, 20), d, n, t.title);
    }
    println!("\nCKAD readiness {:.0}%", overall);
    for r in &readiness {
        let exam = r.best_exam.map_or("no exam yet".to_string(), |e| format!("best exam {e:.0}%"));
        println!(
            "  {} {:>3}% {} ({}%, {}/{} lessons, {exam})",
            bar(r.score(), 20),
            (r.score() * 100.0) as u32,
            r.title,
            r.weight,
            r.lessons_done,
            r.lessons_total
        );
    }
    if let Some(e) = &c.progress.active_exam {
        println!("\n⏱  exam `{}` in progress: `dojo exam status`", e.exam_id);
    }
    match resume {
        Some(l) => println!("\nContinue: {} — {}  (`dojo show {}`)", l.id, l.title, l.id),
        None if total > 0 => println!("\nEvery lesson is done. Take a mock exam: `dojo exam list`"),
        None => {}
    }
    Ok(())
}

fn next(c: &Ctx) -> Result<(), String> {
    match c.progress.resume(&c.catalog) {
        Some(l) => {
            let p = c.progress.lesson(&l.id).cloned().unwrap_or_default();
            println!("{} — {}", l.id, l.title);
            println!("  module: {} · ~{} min", c.catalog.module_title(l), l.minutes);
            println!("  concepts: {}", if p.read { "read" } else { "to read (`dojo show`, then `dojo read`)" });
            if !l.quiz.is_empty() {
                println!(
                    "  quiz: {}",
                    p.quiz.map_or("not taken (`dojo quiz`)".into(), |q| format!("{}/{}", q.correct, q.total))
                );
            }
            if let Some(lab) = &l.lab {
                println!("  lab: {}/{} steps (`dojo prepare`, then `dojo check`)", p.steps.len(), lab.steps.len());
            }
            Ok(())
        }
        None => {
            println!("Nothing left to study. Take a mock exam: `dojo exam list`");
            Ok(())
        }
    }
}

fn lessons(c: &Ctx) -> Result<(), String> {
    for t in &c.catalog.tracks {
        println!("{}", t.title);
        let mut module = String::new();
        for id in &t.lessons {
            let Some(l) = c.catalog.lesson(id) else { continue };
            if l.module != module {
                module = l.module.clone();
                println!("  {}", c.catalog.module_title(l));
            }
            println!("    {} {:<44} {}", mark(c.progress.status(l)), l.id, l.title);
        }
    }
    Ok(())
}

fn show(c: &Ctx, id: &str) -> Result<(), String> {
    let l = c.lesson(id)?;
    println!("# {}\n", l.title);
    println!(
        "{} · ~{} min{}\n",
        c.catalog.module_title(l),
        l.minutes,
        l.domain.as_deref().map(|d| format!(" · CKAD domain: {d}")).unwrap_or_default()
    );
    if !l.objectives.is_empty() {
        println!("You will be able to:");
        for o in &l.objectives {
            println!("  - {o}");
        }
        println!();
    }
    println!("{}\n", l.body);
    if let Some(lab) = &l.lab {
        println!("## Lab\n\n{}\n", lab.summary);
        for (i, s) in lab.steps.iter().enumerate() {
            println!("### Step {}: {}\n\n{}\n", i + 1, s.title, s.task);
        }
        println!("Run `dojo prepare {id}` first, work in `eval \"$(dojo env {id})\"`, then `dojo check {id}`.");
    }
    Ok(())
}

fn quiz(c: &mut Ctx, id: &str) -> Result<(), String> {
    let l = c.lesson(id)?.clone();
    if l.quiz.is_empty() {
        return Err(format!("`{id}` has no quiz"));
    }
    let stdin = std::io::stdin();
    let mut correct = 0;
    for (i, q) in l.quiz.iter().enumerate() {
        println!("\nQ{}. {}", i + 1, q.q);
        let order = q.order(&l.id);
        for (pos, j) in order.iter().enumerate() {
            println!("   {}) {}", (b'a' + pos as u8) as char, q.choices.get(*j).map_or("", String::as_str));
        }
        print!("> ");
        let _ = std::io::stdout().flush();
        let mut line = String::new();
        stdin.lock().read_line(&mut line).map_err(|e| e.to_string())?;
        let pick = line
            .trim()
            .chars()
            .next()
            .map(|ch| (ch.to_ascii_lowercase() as u8).wrapping_sub(b'a') as usize)
            .and_then(|pos| order.get(pos).copied());
        if pick == Some(q.answer) {
            correct += 1;
            println!("✓ correct. {}", q.why);
        } else {
            let right = q.choices.get(q.answer).map_or("", String::as_str);
            println!("✗ the answer is: {right}\n  {}", q.why);
        }
    }
    let score = QuizScore { correct, total: l.quiz.len() as u32 };
    c.progress.open_lesson(&l, now());
    c.progress.record_quiz(&l, score, now());
    c.save()?;
    println!(
        "\nScore {}/{} {}",
        score.correct,
        score.total,
        if score.passed() { "· passed" } else { "· 70% needed to pass" }
    );
    Ok(())
}

fn prepare(c: &mut Ctx, id: &str) -> Result<(), String> {
    let l = c.lesson(id)?.clone();
    let env = c.env_for(id)?;
    let (ok, log) = lab::prepare(&l, &env);
    print!("{log}");
    c.progress.open_lesson(&l, now());
    c.save()?;
    if ok {
        println!("✓ lab ready in {}. Work with: eval \"$(dojo env {id})\"", env.workdir.display());
        Ok(())
    } else {
        Err("lab setup failed".into())
    }
}

fn print_results(results: &[CheckResult]) {
    for r in results {
        if r.passed {
            println!("    ✓ {}", r.name);
        } else {
            println!("    ✗ {}: {}", r.name, r.detail);
        }
    }
}

fn check(c: &mut Ctx, id: &str, only: Option<usize>) -> Result<(), String> {
    let l = c.lesson(id)?.clone();
    let lab = l.lab.clone().ok_or_else(|| format!("`{id}` has no lab"))?;
    let env = c.env_for(id)?;
    lab::preflight(l.needs, &env)?;
    let mut all_ok = true;
    for (i, step) in lab.steps.iter().enumerate() {
        if only.is_some_and(|n| n != i + 1) {
            continue;
        }
        let results = lab::run_checks(&step.checks, &env);
        let ok = results.iter().all(|r| r.passed);
        all_ok &= ok;
        println!("{} Step {}: {}", if ok { "✓" } else { "✗" }, i + 1, step.title);
        print_results(&results);
        c.progress.record_step(&l, i, ok, now());
    }
    c.progress.open_lesson(&l, now());
    c.save()?;
    if c.progress.is_done(&l) {
        println!("\n🥋 lesson complete");
    }
    if all_ok { Ok(()) } else { Err("some checks failed".into()) }
}

fn reset(c: &mut Ctx, id: &str) -> Result<(), String> {
    let l = c.lesson(id)?.clone();
    let env = c.env_for(id)?;
    let (ok, log) = lab::reset(&l, &env);
    print!("{log}");
    if !ok {
        return Err("lab reset refused; progress kept".into());
    }
    c.progress.reset_lab(&l);
    c.save()?;
    println!("✓ lab reset");
    Ok(())
}

fn list_targets(c: &Ctx) -> Result<(), String> {
    let all = targets::list(&c.paths)?;
    let chosen = targets::choose(&all, &c.settings.target).map(|t| t.name);
    for t in &all {
        let star = if Some(&t.name) == chosen.as_ref() { "▶" } else { " " };
        println!(
            "{star} {:<16} {:<5} {}{}",
            t.name,
            format!("{:?}", t.kind).to_lowercase(),
            t.endpoint,
            if t.error.is_empty() { String::new() } else { format!("  ({})", t.error) }
        );
    }
    println!("\nChoose with `dojo use <machine>`. Add one: docker context create NAME --docker host=ssh://user@host");
    Ok(())
}

fn use_target(c: &mut Ctx, name: &str) -> Result<(), String> {
    let all = targets::list(&c.paths)?;
    let t = all
        .iter()
        .find(|t| t.name == name)
        .ok_or_else(|| format!("no Docker context `{name}` (see `dojo targets`)"))?;
    let env = t.env(&c.paths, c.paths.root.clone());
    match targets::probe(&env) {
        Ok(info) => {
            println!("✓ {name}: Docker {} on {} ({}, {} CPUs)", info.server_version, info.os, info.arch, info.cpus)
        }
        Err(e) => println!("! {name} is not answering right now: {e}"),
    }
    c.settings.target = name.to_string();
    c.settings.save(&c.paths.settings()).map_err(|e| e.to_string())?;
    if let Some(rec) = cluster::ClusterRecord::load(&c.paths)
        && rec.target != name
    {
        println!(
            "! the lab cluster lives on `{}`: recreate it here with `dojo cluster down` then `dojo cluster up`",
            rec.target
        );
    }
    println!("labs now run on `{name}`");
    Ok(())
}

fn env(c: &Ctx, id: &str) -> Result<(), String> {
    let env = c.env_for(id)?;
    print!("{}", env.export_script());
    println!("cd {}", poddojo_runner::exec::shell_quote(&env.workdir.display().to_string()));
    Ok(())
}

fn shell(c: &Ctx, id: &str) -> Result<(), String> {
    let env = c.env_for(id)?;
    terminal::open(&env, &c.target()?.name, &c.paths.root)
}

fn run_doctor(c: &Ctx) -> Result<(), String> {
    let env = LabEnv {
        docker_context: String::new(),
        kubeconfig: c.paths.kubeconfig(),
        workdir: c.paths.root.clone(),
        host: "127.0.0.1".into(),
        cluster: "poddojo".into(),
    };
    let tools = doctor::check(&env);
    for t in &tools {
        match &t.version {
            Some(v) => println!("✓ {:<8} {v}", t.name),
            None => {
                println!("{} {:<8} missing: {}  → {}", if t.required { "✗" } else { "!" }, t.name, t.why, t.install)
            }
        }
    }
    match c.target() {
        Ok(t) => {
            let env = t.env(&c.paths, c.paths.root.clone());
            match targets::probe(&env) {
                Ok(i) => println!("✓ machine  {} → Docker {} on {} ({})", t.name, i.server_version, i.os, i.arch),
                Err(e) => println!("✗ machine  {}: {e}", t.name),
            }
            match cluster::status(&env) {
                cluster::Status::Running { nodes } => {
                    let server = nodes.first().map_or("", |n| n.version.as_str());
                    println!("✓ cluster  kind-poddojo, {} node(s), {server}", nodes.len());
                    let client =
                        tools.iter().find(|t| t.name == "kubectl").and_then(|t| t.version.clone()).unwrap_or_default();
                    if let Some(w) = doctor::kubectl_skew(&client, server) {
                        println!("! kubectl  {w}");
                    }
                }
                cluster::Status::Missing => println!("· cluster  not created yet: `dojo cluster up`"),
                cluster::Status::Unreachable(e) => println!("✗ cluster  {e}"),
            }
        }
        Err(e) => println!("✗ machine  {e}"),
    }
    println!("· data     {}", c.paths.root.display());
    if doctor::missing_required(&tools).is_empty() { Ok(()) } else { Err("install the missing tools above".into()) }
}

fn cluster_status(c: &Ctx) -> Result<(), String> {
    let env = c.env_for("cluster")?;
    let rec = cluster::ClusterRecord::load(&c.paths);
    match cluster::status(&env) {
        cluster::Status::Running { nodes } => {
            println!("kind-poddojo on `{}`", rec.map(|r| r.target).unwrap_or_default());
            for n in nodes {
                println!("  {} {:<28} {:<14} {}", if n.ready { "✓" } else { "✗" }, n.name, n.role, n.version);
            }
            Ok(())
        }
        cluster::Status::Missing => {
            println!("no lab cluster: `dojo cluster up` creates one on `{}`", c.target()?.name);
            Ok(())
        }
        cluster::Status::Unreachable(e) => Err(e),
    }
}

fn cluster_up(c: &Ctx) -> Result<(), String> {
    let target = c.target()?;
    let env = target.env(&c.paths, c.paths.root.clone());
    if cluster::status(&env).is_running() {
        println!("lab cluster already running");
        return Ok(());
    }
    let cmd = cluster::create_command(&c.paths, &c.settings, &target).map_err(|e| e.to_string())?;
    println!("$ {cmd}");
    let out = poddojo_runner::exec::run_live(&cmd, &env, Duration::from_secs(900), None);
    print!("{}", out.combined());
    if !out.success() {
        return Err("kind create cluster failed".into());
    }
    cluster::ClusterRecord { target: target.name.clone(), created_at: now() }
        .save(&c.paths)
        .map_err(|e| e.to_string())?;
    println!("\n✓ lab cluster ready on `{}`", target.name);
    cluster_status(c)
}

fn cluster_down(c: &Ctx) -> Result<(), String> {
    let rec = cluster::ClusterRecord::load(&c.paths);
    let all = targets::list(&c.paths)?;
    let selected = targets::choose(&all, &c.settings.target).map(|t| t.name).unwrap_or_default();
    let target = cluster::delete_target(&all, rec.as_ref(), &selected)?;
    let env = target.env(&c.paths, c.paths.root.clone());
    let cmd = cluster::delete_command(&c.paths);
    println!("$ {cmd}");
    let out = poddojo_runner::run(&cmd, &env, Duration::from_secs(300));
    print!("{}", out.combined());
    if out.success() {
        cluster::ClusterRecord::clear(&c.paths);
        let _ = std::fs::remove_file(c.paths.kubeconfig());
        println!("✓ lab cluster deleted");
        Ok(())
    } else {
        Err("kind delete cluster failed".into())
    }
}

fn exam_list(c: &Ctx) -> Result<(), String> {
    for e in &c.catalog.exams {
        let best = c.progress.best_attempt(&e.id).map_or("not taken".to_string(), |a| {
            format!("best {:.0}% {}", a.percent, if a.passed { "PASS" } else { "fail" })
        });
        println!(
            "{:<16} {:<36} {} tasks · {} min · pass {}% · {best}",
            e.id,
            e.title,
            e.tasks.len(),
            e.minutes,
            e.pass_percent
        );
    }
    Ok(())
}

fn exam_env(c: &Ctx, e: &Exam) -> Result<LabEnv, String> {
    c.env_for(&format!("exam-{}", e.id))
}

fn exam_start(c: &mut Ctx, id: &str) -> Result<(), String> {
    let e = c.catalog.exam(id).ok_or_else(|| format!("no exam `{id}`"))?.clone();
    let env = exam_env(c, &e)?;
    cluster::guard(&env)?;
    println!("Preparing the exam environment…");
    print!("{}", lab::provide(&e.requires, &env)?);
    // A retake starts clean: remove whatever the previous attempt left behind.
    let _ = lab::run_all(&e.cleanup, &env, Duration::from_secs(300));
    let setup: Vec<String> = e.setup.iter().chain(e.tasks.iter().flat_map(|t| &t.setup)).cloned().collect();
    let (ok, log) = lab::run_all(&setup, &env, Duration::from_secs(300));
    if !ok {
        print!("{log}");
        return Err("exam setup failed".into());
    }
    c.progress.active_exam =
        Some(poddojo_core::ActiveExam { exam_id: e.id.clone(), started_at: now(), ..Default::default() });
    c.save()?;
    println!("⏱  {} started: {} minutes, {} tasks, pass {}%.", e.title, e.minutes, e.tasks.len(), e.pass_percent);
    println!("Work in: eval \"$(dojo env exam-{})\"   Then: dojo exam grade", e.id);
    exam_status(c)
}

fn exam_status(c: &Ctx) -> Result<(), String> {
    let a = c.progress.active_exam.as_ref().ok_or("no exam in progress (`dojo exam start <id>`)")?;
    let e = c.catalog.exam(&a.exam_id).ok_or("the running exam is no longer in the catalog")?;
    let left = e.minutes as i64 * 60 - (now() - a.started_at);
    if left > 0 {
        println!("⏱  {}:{:02} left", left / 60, left % 60);
    } else {
        println!("⏱  time is up ({} min over): grade now with `dojo exam grade`", -left / 60);
    }
    for (i, t) in e.tasks.iter().enumerate() {
        println!("\n## Task {} · {} · {}% · {}", i + 1, t.title, t.weight, t.domain);
        if let Some(ns) = &t.namespace {
            println!("namespace: {ns}");
        }
        println!("{}", t.task);
    }
    Ok(())
}

fn grade_exam(e: &Exam, env: &LabEnv) -> BTreeMap<String, Vec<CheckResult>> {
    e.tasks.iter().map(|t| (t.id.clone(), lab::run_checks(&t.checks, env))).collect()
}

fn print_attempt(e: &Exam, a: &poddojo_core::ExamAttempt) {
    println!("\n{} — {:.1}% {}", e.title, a.percent, if a.passed { "PASS ✓" } else { "FAIL ✗" });
    for (d, s) in &a.domains {
        println!("  {:<18} {:>5.1}%  ({:.1}/{:.0})", d, s.percent(), s.earned, s.possible);
    }
    for t in &a.tasks {
        println!("  {} {:<10} {:.1}/{:.0}", if t.earned >= t.possible { "✓" } else { "✗" }, t.id, t.earned, t.possible);
        for r in t.checks.iter().filter(|r| !r.passed) {
            println!("      ✗ {}: {}", r.name, r.detail);
        }
    }
}

fn exam_grade(c: &mut Ctx) -> Result<(), String> {
    let a = c.progress.active_exam.clone().ok_or("no exam in progress")?;
    let e = c.catalog.exam(&a.exam_id).ok_or("the running exam is no longer in the catalog")?.clone();
    let env = exam_env(c, &e)?;
    cluster::guard(&env)?;
    println!("Grading…");
    let results = grade_exam(&e, &env);
    let attempt = exam::grade(&e, &results, a.started_at, now());
    print_attempt(&e, &attempt);
    c.progress.record_exam(attempt);
    c.save()
}

/// Bash blocks of a solution, in order: the commands a worked solution runs.
fn solution_blocks(markdown: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut current: Option<String> = None;
    for line in markdown.lines() {
        let t = line.trim_start();
        match &mut current {
            None if t.starts_with("```") => {
                let lang = t.trim_start_matches('`').trim();
                if matches!(lang, "bash" | "sh" | "shell" | "console") {
                    current = Some(String::new());
                } else {
                    current = Some("\u{0}skip".into());
                }
            }
            Some(buf) if t.starts_with("```") => {
                if !buf.starts_with('\u{0}') {
                    blocks.push(std::mem::take(buf));
                }
                current = None;
            }
            Some(buf) => {
                buf.push_str(line);
                buf.push('\n');
            }
            None => {}
        }
    }
    blocks
}

fn run_solution(markdown: &str, env: &LabEnv) -> Result<(), String> {
    for block in solution_blocks(markdown) {
        let script = format!("set -e\n{block}");
        let out = poddojo_runner::run(&script, env, Duration::from_secs(600));
        if !out.success() {
            return Err(format!("solution failed:\n{block}\n{}", out.combined()));
        }
    }
    Ok(())
}

/// Cleanup deletes namespaces with `--wait=false`; setup right after it would race the deletion
/// ("being terminated"). Wait until no namespace the setup mentions is still terminating.
fn wait_for_namespaces(setup: &[String], env: &LabEnv) {
    let text = setup.join("\n");
    for _ in 0..60 {
        let out = poddojo_runner::run(
            "kubectl get namespaces --field-selector status.phase=Terminating -o name",
            env,
            Duration::from_secs(15),
        );
        let busy = out
            .stdout
            .lines()
            .filter_map(|l| l.trim().strip_prefix("namespace/"))
            .any(|ns| text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-')).any(|word| word == ns));
        if !out.success() || !busy {
            return;
        }
        std::thread::sleep(Duration::from_secs(2));
    }
}

/// Run a case's commands (or its undo) as one `set -e` script.
fn run_case_script(script: &str, env: &LabEnv) -> Result<(), String> {
    if script.trim().is_empty() {
        return Ok(());
    }
    let out = poddojo_runner::run(&format!("set -e\n{script}"), env, Duration::from_secs(600));
    if out.success() { Ok(()) } else { Err(out.combined()) }
}

/// Run every negative and alternative case of a step or task from the state it starts in:
/// negatives must fail at least one check, alternatives must pass them all. Returns the problems.
fn verify_cases(what: &str, checks: &[Check], negatives: &[Case], alternatives: &[Case], env: &LabEnv) -> Vec<String> {
    let mut problems = Vec::new();
    let cases = negatives.iter().map(|c| (true, c)).chain(alternatives.iter().map(|c| (false, c)));
    for (negative, case) in cases {
        let kind = if negative { "negative" } else { "alternative" };
        if let Err(e) = run_case_script(&case.run, env) {
            problems.push(format!("{what}: {kind} `{}` could not run:\n{e}", case.name));
        } else {
            let results = lab::run_checks(checks, env);
            let failed: Vec<&CheckResult> = results.iter().filter(|r| !r.passed).collect();
            let as_expected = failed.is_empty() != negative;
            println!("    {} {kind}: {}", if as_expected { "✓" } else { "✗" }, case.name);
            if negative && failed.is_empty() {
                problems.push(format!("{what}: negative `{}` passes every check (the grader accepts it)", case.name));
            }
            if !negative {
                for r in failed {
                    problems
                        .push(format!("{what}: alternative `{}` fails check `{}`: {}", case.name, r.name, r.detail));
                }
            }
        }
        if let Err(e) = run_case_script(&case.undo, env) {
            problems.push(format!("{what}: undo of {kind} `{}` failed:\n{e}", case.name));
        }
    }
    problems
}

fn verify_lesson(c: &Ctx, l: &Lesson, keep: bool) -> Result<(), String> {
    let Some(labdef) = &l.lab else {
        println!("· {} (no lab)", l.id);
        return Ok(());
    };
    let env = c.env_for(&l.id)?;
    let _ = lab::reset(l, &env);
    if l.needs == Needs::Cluster {
        wait_for_namespaces(&labdef.setup, &env);
    }
    let (ok, log) = lab::prepare(l, &env);
    if !ok {
        return Err(format!("{}: setup failed\n{log}", l.id));
    }
    let mut problems = Vec::new();
    let mut cases = 0;
    for (i, step) in labdef.steps.iter().enumerate() {
        let before = lab::run_checks(&step.checks, &env);
        if before.iter().all(|r| r.passed) && step.checks.iter().all(|c| c.exit.is_none()) {
            problems.push(format!("step {} passes before the solution runs (checks prove nothing)", i + 1));
        }
        cases += step.negatives.len() + step.alternatives.len();
        problems.extend(verify_cases(
            &format!("step {}", i + 1),
            &step.checks,
            &step.negatives,
            &step.alternatives,
            &env,
        ));
        if let Err(e) = run_solution(&step.solution, &env) {
            problems.push(format!("step {}: {e}", i + 1));
            break;
        }
        let after = lab::run_checks(&step.checks, &env);
        for r in after.iter().filter(|r| !r.passed) {
            problems.push(format!("step {} check `{}` fails after the solution: {}", i + 1, r.name, r.detail));
        }
    }
    if !keep {
        let _ = lab::reset(l, &env);
    }
    if problems.is_empty() {
        println!("✓ {} ({} steps, {cases} extra case(s))", l.id, labdef.steps.len());
        Ok(())
    } else {
        Err(format!("{}:\n  {}", l.id, problems.join("\n  ")))
    }
}

fn verify_exam(c: &Ctx, full: &Exam, only: Option<&str>, keep: bool) -> Result<(), String> {
    let env = exam_env(c, full)?;
    // The whole exam is set up and cleaned up, but only the chosen task is cased and graded.
    let mut picked_exam = full.clone();
    picked_exam.tasks.retain(|t| only.is_none_or(|o| o == t.id));
    let e = &picked_exam;
    cluster::guard(&env)?;
    print!("{}", lab::provide(&e.requires, &env).map_err(|err| format!("exam {}: {err}", e.id))?);
    let _ = lab::run_all(&e.cleanup, &env, Duration::from_secs(300));
    let setup: Vec<String> = e.setup.iter().chain(full.tasks.iter().flat_map(|t| &t.setup)).cloned().collect();
    wait_for_namespaces(&setup, &env);
    let (ok, log) = lab::run_all(&setup, &env, Duration::from_secs(600));
    if !ok {
        return Err(format!("exam {}: setup failed\n{log}", e.id));
    }
    let before = exam::grade(e, &grade_exam(e, &env), 0, 0);
    let mut problems = Vec::new();
    for t in &before.tasks {
        if t.earned >= t.possible {
            problems.push(format!("task {} is already solved before the solution runs", t.id));
        }
    }
    let mut cases = 0;
    for t in &e.tasks {
        cases += t.negatives.len() + t.alternatives.len();
        problems.extend(verify_cases(&format!("task {}", t.id), &t.checks, &t.negatives, &t.alternatives, &env));
        if let Err(err) = run_solution(&t.solution, &env) {
            problems.push(format!("task {}: {err}", t.id));
        }
    }
    let after = exam::grade(e, &grade_exam(e, &env), 0, 0);
    for t in &after.tasks {
        for r in t.checks.iter().filter(|r| !r.passed) {
            problems.push(format!("task {} check `{}` fails after the solution: {}", t.id, r.name, r.detail));
        }
    }
    if !keep {
        let _ = lab::run_all(&e.cleanup, &env, Duration::from_secs(300));
    }
    if problems.is_empty() {
        println!(
            "✓ exam {} ({} task(s), {cases} extra case(s), solutions score {:.0}%)",
            e.id,
            e.tasks.len(),
            after.percent
        );
        Ok(())
    } else {
        Err(format!("exam {}:\n  {}", e.id, problems.join("\n  ")))
    }
}

fn verify(c: &Ctx, what: &str, keep: bool) -> Result<(), String> {
    if let Some(id) = what.strip_prefix("exam:") {
        let (id, only) = id.split_once('/').map_or((id, None), |(e, t)| (e, Some(t)));
        let e = c.catalog.exam(id).ok_or_else(|| format!("no exam `{id}`"))?;
        if let Some(t) = only
            && !e.tasks.iter().any(|x| x.id == t)
        {
            return Err(format!("exam `{id}` has no task `{t}`"));
        }
        return verify_exam(c, e, only, keep);
    }
    let lessons: Vec<&Lesson> = match what {
        "all" => c.catalog.ordered_lessons().collect(),
        track if c.catalog.track(track).is_some() => c.catalog.ordered_lessons().filter(|l| l.track == track).collect(),
        id => vec![c.lesson(id)?],
    };
    let mut failures = Vec::new();
    for l in lessons {
        if l.needs == Needs::Cluster && cluster::guard(&c.env_for(&l.id)?).is_err() {
            println!("· {} skipped: no lab cluster (`dojo cluster up`)", l.id);
            continue;
        }
        if let Err(e) = verify_lesson(c, l, keep) {
            eprintln!("✗ {e}");
            failures.push(l.id.clone());
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(format!("{} lesson(s) failed: {}", failures.len(), failures.join(", ")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_bash_blocks_only() {
        let md = "Do this:\n```bash\necho one\n```\n```yaml\nkind: Pod\n```\n```sh\necho two\n```\n";
        assert_eq!(solution_blocks(md), vec!["echo one\n".to_string(), "echo two\n".to_string()]);
    }
}
