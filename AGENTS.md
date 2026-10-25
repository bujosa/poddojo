# PodDojo — instructions for agents

PodDojo is an ultralight, pure-Rust (egui) desktop dojo for learning Docker and passing the **CKAD**
(Certified Kubernetes Application Developer, curriculum v1.37). Learners pick the machine that runs
Docker (any Docker context: this computer or a server over SSH), study lessons, do auto-graded
hands-on labs, take timed mock exams, and always resume where they stopped. Everything is in English.

## Start every session here
1. `cargo run -q -p dojo -- status` shows the learner's state (progress lives outside the repo).
2. Read [`ROADMAP.md`](ROADMAP.md) for what is done and what is next.
3. Content work: read [`docs/content-authoring.md`](docs/content-authoring.md) first. Code work:
   [`docs/architecture.md`](docs/architecture.md).

## Layout
- `crates/core` — curriculum model + validation, progress/resume, grading. Pure, no processes.
- `crates/runner` — machines (Docker contexts), command execution with the lab environment, the
  kind lab cluster, explorers, the doctor, the lab terminal.
- `crates/ui` — the egui app (thin: reads core/runner state, runs work on background threads).
- `apps/poddojo` — desktop binary. `apps/dojo` — CLI (same progress file as the app).
- `content/` — tracks, lessons and exams (embedded at build time).
- `xtask` — `cargo xtask ci`, `cargo xtask icon`.

## Non-negotiables

### Safety: the learner's machines and clusters are real
- PodDojo only ever talks to the lab cluster through **its own kubeconfig**
  (`<data>/kubeconfig`, context `kind-poddojo`); `cluster::guard` refuses anything else. Never read,
  write or switch the user's `~/.kube/config` or Docker context.
- Content commands must pass `poddojo_core::check::unsafe_command`: no prune, no "all containers",
  no `sudo`, no `--context`/`--kubeconfig`. Docker resources are named `dojo-…`.
- Never delete user data outside PodDojo's data folder.

### Never crash
- No `unwrap()`, `expect()`, `panic!`, `unreachable!`, `todo!`, `unimplemented!` in shipped code
  (workspace clippy lints deny them; tests may use them). `unsafe_code = "forbid"`.
- The UI never blocks: anything that runs a command goes through `ui::jobs` on a thread.

### Ultralight
- Pure Rust, egui + glow, no web view, no async runtime, few dependencies. Talk to Docker and
  Kubernetes through their CLIs (`docker`, `kubectl`, `kind`, `helm`): that keeps SSH contexts, auth
  plugins and versions working exactly like in the learner's terminal (and like the exam).

### Assets
- Every image or icon is original work (MIT OR Apache-2.0) and listed in `ATTRIBUTION.md`. Never use
  Docker's, Kubernetes' or the Linux Foundation's logos or artwork.

## Quality gates (before every commit)
`cargo xtask ci` = fmt check, clippy `-D warnings`, tests, content validation. For content also run
`dojo verify` on what you changed (see docs/content-authoring.md). Conventional Commits.

## Running
- App: `cargo run -p poddojo --release`. CLI: `cargo run -q -p dojo -- help`.
- `PODDOJO_HOME=<dir>` isolates progress/settings (use it in tests and agents);
  `PODDOJO_CONTENT_DIR=content` loads lessons from disk without rebuilding.
