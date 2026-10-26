# Roadmap

**Stage:** v0.1, usable end to end. 39 lessons, 155 auto-graded lab steps, 184 quiz questions,
3 CKAD mock exams (51 tasks), all verified on a real kind cluster (Kubernetes v1.37) with
`dojo verify`.

## Done
- Profiles: a welcome screen ("Who's training today?"), several learners per computer, guest
  mode, resumable lab sessions (prepared state, hints, last checks), labs in progress on Home.
- Workspace: lab steps beside a terminal pane; exam workspace with timer, flags and terminal.
  Draggable dividers (widths remembered); the exam task list folds into a drawer below 1200 pt.
- Built-in terminal: a real interactive bash on a pseudo-terminal (pty-process + vt100), with
  colours, scrollback, selection and copy, paste, Ctrl-C, `vim`, `watch` and `kubectl exec -it`.
- Polished UI: icons drawn in code, quiz answer cards, progress rings, searchable lessons,
  shortcuts, light and dark themes; macOS app bundle (`cargo xtask bundle --install`).
- Doctor warns when kubectl is more than one minor version from the lab cluster.
- Machines: every Docker context (local, ssh://, tcp://), engine probe, per-machine labs.
- Lab cluster: kind on the chosen machine, own kubeconfig + guard, own Docker network.
- Learn: concepts with ▶ Run snippets, quizzes with explanations, labs with hints, solutions and
  checks; resume where you left off; study time, streak.
- Exams: timed, flags, partial credit, per-domain scores, history, knowledge drill.
- Explorers: Docker containers/images/layers/volumes/networks, Kubernetes objects by kind.
- `dojo` CLI on the same progress (status --json for AI tutors), `dojo verify` for content.

## Next
1. Spaced repetition: resurface quiz questions you missed, on a schedule.
2. Exam realism: per-task "context" switching between two lab clusters, a vim-only terminal mode.
3. More mock exams and "speed drills" (one task, a stopwatch, a target time).
4. Docker track extras: BuildKit secrets and cache mounts, image scanning and SBOMs.
5. Terminal extras: tabs, find in scrollback, mouse reporting for full-screen programs.
6. Packaging: signed macOS .app bundle and a Linux AppImage.
