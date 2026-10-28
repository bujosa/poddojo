# Roadmap

**Stage:** v0.3, everything needed to prepare for the CKAD in the app. 40 lessons, 159
auto-graded lab steps, 190 quiz questions, 4 CKAD mock exams (68 tasks), 8 reference pages, all
verified on a real kind cluster (Kubernetes v1.37) with `dojo verify`.

## Done
- v0.3.0: spaced review of quiz questions and timed drills of missed mock tasks; an honest
  readiness verdict from fresh mocks; in-app reference (page and ⌘K window); mock exam 4; a
  technical proofreading pass over every lesson and a zero-level terminal/YAML/vim lesson;
  progress heatmap and score trend; accessibility labels and focus rings; signed bundles and
  `cargo xtask release`; Linux build checked.
- v0.2.0: grader audit (negative and alternative verification cases), a working Ingress
  controller, behavioural NetworkPolicy grading.
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
1. Exam realism: per-task "context" switching between two lab clusters, a vim-only terminal mode.
2. More mock exams generated as variations of existing tasks (new names and values), so retakes
   stay fresh.
3. Docker track extras: BuildKit secrets and cache mounts, image scanning and SBOMs.
4. Terminal extras: tabs, find in scrollback, mouse reporting for full-screen programs.
5. Packaging: a Developer ID signature (needs an Apple Developer account) and a Linux AppImage.
