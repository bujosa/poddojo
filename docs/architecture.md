# Architecture

PodDojo is a Cargo workspace. Each layer only depends on the ones above it.

```text
content/            Markdown + TOML curriculum (embedded at build time by crates/core/build.rs)
   │
crates/core         Catalog (tracks, lessons, quizzes, labs, exams, reference) + validation
                    Progress (resume, completion, readiness verdict, streak)  ── JSON in the data folder
                    Review deck (Leitner spaced repetition of quiz questions and exam tasks)
                    Check evaluation + exam grading (pure, no processes)
   │
crates/runner       exec      /bin/sh -c with the lab environment, timeouts, live output
                    targets   Docker contexts = machines (local socket, ssh://, tcp://)
                    cluster   kind lab cluster + guard (only kind-poddojo, own kubeconfig)
                    lab       prepare / check / reset a lesson's lab; set up one exam task for a drill
                    explore   docker ps/images/history, kubectl get … -o json summaries
                    doctor    docker, kubectl, kind, helm, ssh
                    shell     the built-in terminal: bash on a pseudo-terminal + a VT100 screen
                    terminal  "External" terminal app with the lab environment
   │
crates/ui           egui pages; every command runs on a thread (jobs.rs) and comes back as a message
   │
apps/poddojo        native window (eframe + glow)      apps/dojo   CLI on the same data
```

## The lab environment

Every command PodDojo runs (lab setup, checks, the built-in terminal, the external one it opens) gets:

- `DOCKER_CONTEXT=<chosen machine>` and no `DOCKER_HOST`, so `docker` talks to that machine's engine
  (SSH contexts work exactly as in the learner's own terminal);
- `KUBECONFIG=<data>/kubeconfig`, a file that only ever holds the `kind-poddojo` context.
  `cluster::guard` re-checks this before any Kubernetes work;
- `PODDOJO_HOST`, the address that reaches ports published on the machine;
- the lesson's own working folder as the current directory.

Talking to Docker and Kubernetes through their CLIs (instead of client libraries) keeps the binary
small, inherits SSH, credential helpers and auth plugins for free, and is exactly how the CKAD is
taken.

## The built-in terminal

`runner::shell` starts `bash --noprofile --rcfile <profile>/lab-shell.bashrc -i` on a
pseudo-terminal (`pty-process`, rustix only). The rc file re-exports the lab environment after
anything inherited, so the learner's dotfiles never point `KUBECONFIG` elsewhere, and adds the CKAD
habits (`alias k=kubectl`, completion, `$do`). A reader thread feeds the output into a `vt100`
screen model (scrollback, alternate screen, colours) and wakes the UI; a writer thread sends keys and
pastes, so the UI never blocks. `ui::terminal` draws the grid cell by cell and maps egui keys to
terminal bytes. A new machine or lab folder starts a new shell.

## The lab cluster

`kind create cluster --name poddojo` on the chosen machine, on PodDojo's own Docker network
`dojo-kind` (with fallback subnets when Docker's address pools are exhausted). On a remote machine
the API server listens on the machine's address so this computer can reach it. NodePort 30080 and
30443 are published on the machine.

Lessons and exams may declare `requires`. Before setup, `lab::provide` checks `two-nodes` (a Ready
worker besides the control plane) and installs `ingress-controller` on demand: the Traefik manifest
in `crates/runner/assets/ingress-controller.yaml` (namespace `dojo-ingress`, HTTPS on NodePort 30443,
and a `probe` curl Deployment for graders). It stays installed until the cluster is deleted.

## Grading

A check is `run` (a read-only command) plus an expectation (`equals` / `contains` / `matches` /
exit status). Lab steps pass when all their checks pass. Exams give partial credit per check, weight
tasks, and roll scores up into the five CKAD domains. `dojo verify` proves content is gradeable:
checks must fail before a step's solution runs, reject every declared negative case (incomplete or
wrong work), accept every alternative solution, and pass after the worked solution.

## Spaced review and readiness

`core::review` keeps a deck of cards in the progress file: one per quiz question answered (keyed by
lesson id + a hash of the question text, so reordering keeps history) and one per mock-exam task
attempted. A correct answer moves a card up a box (due again in 1, 3, 7, 16, 35 days); a miss sends
it back to box 0, due again in 10 minutes. Exam tasks without full marks are due at once as
**drills**: `lab::prepare_task` deletes the task's namespaces, reruns the shared setup that concerns
them and the task's own setup, and the Review page times it at exam pace (1.2 minutes per point).

Readiness is a verdict, not a blended number: only **fresh** attempts (the first attempt of each
mock) count, because a retake measures memory. The verdict combines curriculum coverage (lessons
done, weighted like the exam), the mean of the last two fresh mocks, and weak domains (under 66%).
"Ready to book" needs two or more fresh mocks averaging 80%+, no weak domain and 90%+ coverage.

## Progress and resume

`<data>/profiles/<id>/progress.json` records lessons read, the review deck, best quiz scores, passed lab steps, study minutes,
active days, exam attempts and the exam in progress. "Resume" is the last unfinished lesson, else
the next unfinished one. The app and the CLI read and write the same file.
