# Changelog

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
