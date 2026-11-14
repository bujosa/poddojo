# Changelog

## 0.3.2

- The app icon follows Apple's grid (the artwork inside the standard margin, with a soft shadow),
  so PodDojo sits at the same size as every other app in the Dock and in Keiko.

## 0.3.1

- Release binaries no longer contain the builder's home folder (crate paths in panic locations);
  `cargo xtask bundle` fails if they do.
- Installable from **Keiko**, the launcher for the dojo apps.

## 0.3.0

- **Review**: every quiz answer and mock-exam task joins a spaced-repetition deck (1, 3, 7, 16, 35
  days; misses come back the same day). One question at a time with keyboard answers, and missed
  mock tasks come back as **timed drills** set up fresh on the cluster. `dojo review` in the CLI.
- **Would you pass today?** replaces the blended readiness percentage: a verdict from your fresh
  first attempts of each mock, curriculum coverage and weak domains, with the next step to take.
- **Reference**: eight dense, copy-ready pages (kubectl, Pod YAML, workloads, config and secrets,
  networking, observability, packaging, API deprecations), searchable, and a floating window (⌘K)
  beside labs, drills and mocks.
- **CKAD Mock Exam 4 (exam day)**: 17 new tasks at the hardest real-exam level.
- **Proofread content**: every lesson checked against Kubernetes v1.37, Helm 4 and Docker 28;
  quiz ambiguities removed; the real exam's rules described accurately. New first lesson: the
  terminal, YAML and vim, for learners starting from zero.
- Progress page with an activity heatmap and a mock score trend; Docker explorer shows only
  PodDojo's resources by default and confirms stopping anything else; kubectl short names in the
  cluster explorer; inline code in quiz text.
- Accessibility: screen-reader labels and keyboard focus rings on every custom control.
- `cargo xtask release` (gates, signed bundle, zip, checksums); Developer ID signing and
  notarisation when configured.

## 0.2.1

- **Lessons use wide screens**: a centred reading column with larger text (16 px on 25 px lines)
  and, on wide windows, a side rail with your progress and the next action, "On this page"
  sections that scroll to the heading, and the lesson's CKAD domain.
- The quiz's answered count and Submit button sit in the rail, always in reach.
- The lesson list can be hidden for more room.

## 0.2.0

- **Graders you can trust**: `dojo verify` also runs negative cases (incomplete or wrong work the
  checks must reject) and alternative solutions (equivalent work they must accept). 25 lab cases
  and 145 exam cases, all passing on Kubernetes v1.37.
- **Working Ingress**: labs and exams that need it install PodDojo's own Traefik controller in
  `dojo-ingress` and grade real requests (host, path, TLS), not just the YAML.
- **NetworkPolicies graded by connecting**: allowed and forbidden traffic are both tested.
- Kustomize overlays isolated per environment; lessons that need the worker node say so
  (`requires = ["two-nodes"]`); quiz distractors rewritten.
- No hosted CI: gates and verification run locally (see `docs/releasing.md`).

## 0.1.0

- Docker Foundations (13 lessons) and CKAD v1.37 (26 lessons) with self-grading labs on a kind
  lab cluster, 3 timed mock exams.
- Lab workspace with an interactive terminal and resizable panes; exam workspace with timer and
  flags.
- Profiles, guest mode and resumable labs; syllabus; living animated UI; `dojo` CLI.
