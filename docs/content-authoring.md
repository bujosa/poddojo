# Writing PodDojo content

Everything a learner studies lives in `content/` as plain text. The app and the `dojo` CLI embed it
at build time; set `PODDOJO_CONTENT_DIR=content` to load it from disk while you write (no rebuild).

```text
content/docker/track.toml        a track: id, title, order and its modules
content/docker/NN-<slug>.md      a lesson (front matter + Markdown)
content/ckad/…                   the CKAD track
content/exams/<id>.toml          a timed mock exam
```

Lessons are studied in module order (as listed in `track.toml`), then by file name. Number files
`01-…`, `02-…` within a track.

**Quality bar:** `content/docker/01-what-is-a-container.md` is the reference lesson. Match its depth,
tone and structure.

## Gates (run both before you commit)

```bash
cargo run -q -p dojo -- validate content          # schema, ids, domains, quiz answers, unsafe commands
PODDOJO_CONTENT_DIR=content cargo run -q -p dojo -- verify <lesson-id|docker|ckad|exam:ID>
```

`dojo verify` runs the lab for real on the chosen machine: reset → setup → for every step, the
checks must **fail** before the step's solution runs, the solution's `bash` blocks run with `set -e`,
then every check must **pass** → cleanup. A lesson is not done until it verifies.

## A lesson

```markdown
+++
id = "ckad-configmaps"                 # unique, lowercase-dashes; prefix docker- or ckad-
title = "ConfigMaps"
module = "config-security"             # a module id from track.toml
minutes = 30                           # honest estimate: reading + quiz + lab
domain = "config-security"             # CKAD domain id (CKAD lessons only)
needs = "cluster"                      # "docker" | "cluster" | "none"
objectives = ["…", "…"]                # 3–5, each starting with a verb

[[quiz]]                               # 4–6 questions
q = "…"
choices = ["…", "…", "…", "…"]         # 4 plausible choices
answer = 2                             # 0-based index
why = "…"                              # teaches, not just "C is correct"

[lab]
summary = "…"
files = { "app/Dockerfile" = """…""" } # optional, written into the lab folder first
setup = ["…"]                          # idempotent preparation
cleanup = ["…"]                        # removes everything the lab creates; never fails (|| true)

[[lab.steps]]                          # 3–6 steps, each building on the previous
title = "…"
task = """…"""                         # Markdown: what to achieve, with exact names
hints = ["…", "…"]                     # nudges, revealed one by one
solution = """
```bash
…commands that reach the end state non-interactively…
```
Explanation of why it works.
"""

[[lab.steps.checks]]                   # 1–4 per step
name = "…"                             # what is verified, in plain words
run = "…"                              # read-only command
equals = "…"                           # or contains = / matches = (regex) / nothing (exit 0 only)
# exit = 1                             # when the command is expected to fail
# timeout = 60                         # seconds (default 30)
+++

## Concepts in Markdown…
```

### The body (concepts)

- 900–1800 words of Markdown: `##` sections, short paragraphs, tables, `text` diagrams, and
  `bash` / `yaml` code blocks the learner can copy or run from the app.
- Teach the *why* and the mental model, then the exact syntax. Show the imperative command **and**
  the YAML. Point out exam traps and time savers (`--dry-run=client -o yaml`, `kubectl explain`).
- End with a short "Exam tips" (CKAD) or "Why this matters for Kubernetes" (Docker) section.
- English only. Kubernetes-accurate for **v1.37** (CKAD curriculum v1.37). Use current APIs
  (`apps/v1`, `batch/v1`, `networking.k8s.io/v1`, `autoscaling/v2`, native sidecars with
  `restartPolicy: Always` init containers, `kubectl debug`, …).

## The lab environment

Every command (setup, checks, solutions, and what the learner types in the console) runs with
`/bin/sh -c` in the lesson's lab folder and this environment:

| Variable | Value |
|---|---|
| `DOCKER_CONTEXT` | the machine the learner chose (Docker Desktop, or a Linux server over SSH) |
| `KUBECONFIG` | PodDojo's own kubeconfig: the lab cluster `kind-poddojo` only |
| `PODDOJO_HOST` | the address that reaches ports published on that machine (`127.0.0.1` locally) |
| `PODDOJO_LAB_DIR` | the lab folder (also the working directory) |
| `PODDOJO_CLUSTER` / `KIND_CLUSTER_NAME` | `poddojo` |

The lab cluster is kind with one control-plane node and one worker node, the default CNI (kindnet,
which enforces NetworkPolicies) and the `standard` StorageClass (local-path, `WaitForFirstConsumer`).
Container port `30080` and `30443` on the control-plane node are published on the machine, so a
NodePort Service on `30080` is reachable at `http://$PODDOJO_HOST:30080`. No Ingress controller and no
metrics-server are installed by default: a lab that needs one installs it in `setup` (and checks that
need a controller must allow time), or checks the objects' spec only.

### Rules (the validator enforces the first ones)

1. **The machine is real and shared.** Name every Docker resource `dojo-…` (containers, networks,
   volumes, images `dojo/…:tag`). Never prune, never act on "all containers", never `sudo`, never
   change the Docker context or kubeconfig (`--context`, `--kubeconfig`, `docker context use`).
2. **Kubernetes labs use their own namespace(s)** named after the lesson (`cm-lab`, `probes-lab`…).
   Create them idempotently in `setup`:
   `kubectl create namespace cm-lab --dry-run=client -o yaml | kubectl apply -f -`, delete them in
   `cleanup`: `kubectl delete namespace cm-lab --ignore-not-found --wait=false`.
   Cluster-scoped objects (PVs, CRDs, ClusterRoles…) get a `dojo-` prefix and their own cleanup.
3. **Checks are read-only and precise**: `docker inspect -f '{{…}}'`, `kubectl get … -o jsonpath='{…}'`,
   `kubectl auth can-i`, `curl -s`. Check the *outcome* the task asked for (names, images, replicas,
   ports, labels, the value inside a file) rather than the exact command typed: several correct
   solutions must all pass.
4. **Checks must fail before the step is done** (`dojo verify` enforces this). Don't check things the
   setup already created.
5. **Wait for state, don't race it**: in solutions use `kubectl rollout status … --timeout=120s`,
   `kubectl wait --for=condition=Ready pod/x --timeout=120s`, `kubectl wait --for=condition=complete job/x`.
   Checks that read eventually-consistent state may poll:
   `run = "for i in $(seq 1 30); do v=$(kubectl -n x get deploy web -o jsonpath='{.status.readyReplicas}'); [ \"$v\" = 3 ] && echo ok && exit 0; sleep 2; done; echo $v"`
   with `equals = "ok"` and `timeout = 90`.
6. **Solutions are non-interactive** `bash` blocks: no `vim`, no `kubectl edit`, no `-it`. Put YAML in
   heredocs (`kubectl apply -f - <<'EOF' … EOF`) or write files with `cat > file <<'EOF'`. Mention the
   interactive way (the exam way) in prose.
7. **Pin small public images**: `alpine:3.20`, `busybox:1.36`, `nginx:1.27-alpine`,
   `python:3.12-alpine`, `redis:7.4-alpine`, `httpd:2.4-alpine`, `registry:2`. Never `:latest`.
   Images built in a Docker lab are tagged `dojo/<name>:<tag>`; to use one in the cluster load it
   with `kind load docker-image dojo/<name>:<tag> --name poddojo` and set `imagePullPolicy: IfNotPresent`.
8. **Offline-friendly where possible**: Helm labs use a local chart (`helm create`) instead of a
   remote repository.
9. In TOML, prefer `'''literal'''` strings for `run` commands that contain backslashes or `{{ }}`,
   and `"""basic"""` strings for Markdown. Inside `"""` a `\` must be escaped (`\\`).

## A mock exam

```toml
id = "ckad-mock-1"
title = "CKAD Mock Exam 1"
summary = "…"
minutes = 120
pass_percent = 66
setup = ["…"]                 # shared preparation (namespaces…)
cleanup = ["…"]               # removes every namespace / object the exam created

[[tasks]]                     # 15–20 tasks; weights sum to 100; domains follow the CKAD weights
id = "q01"
domain = "deployment"         # design-build | deployment | observability | config-security | networking
weight = 6
title = "Rolling update"
namespace = "mock1-q01"       # one namespace per task, created in setup
task = """…"""                # like the real exam: precise names, namespaces, images, values
setup = ["…"]                 # task-specific broken state to start from, if any
solution = """```bash …```"""
[[tasks.checks]]              # 2–5; partial credit per check
name = "…"
run = "…"
equals = "…"
```

Real-exam style: every task names its namespace, tasks are independent, about a third start from an
existing (sometimes broken) state that setup creates, and several require fixing rather than
creating. Domain weights: design-build 20, deployment 20, observability 15, config-security 25,
networking 20.
