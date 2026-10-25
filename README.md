<p align="center">
  <img src="assets/app-icon/poddojo-256.png" width="128" height="128" alt="PodDojo icon: a pod of teal containers with a black-belted amber container on top">
</p>

<h1 align="center">PodDojo</h1>

<p align="center">
  <b>A hands-on dojo for Docker and the CKAD, on your own machines.</b><br>
  Learn the concepts, do real labs that grade themselves, take timed mock exams,<br>
  and always pick up exactly where you left off. Ultralight, native, pure Rust.
</p>

<p align="center">
  <img alt="100% Rust" src="https://img.shields.io/badge/100%25-Rust-b7410e?style=flat-square&logo=rust">
  <img alt="CKAD curriculum v1.37" src="https://img.shields.io/badge/CKAD-v1.37-2dd4bf?style=flat-square">
  <img alt="Kubernetes v1.37 lab" src="https://img.shields.io/badge/lab-Kubernetes%20v1.37-326ce5?style=flat-square">
  <img alt="License: MIT OR Apache-2.0" src="https://img.shields.io/badge/license-MIT%20%2F%20Apache--2.0-3a3a3a?style=flat-square">
</p>

<p align="center">
  <img src="docs/images/home.png" alt="PodDojo home: continue card, CKAD readiness by domain, the chosen machine and the running lab cluster" width="100%">
</p>

<table>
  <tr>
    <td width="25%" valign="top">
      <h3>🖧 Your machines</h3>
      Pick where Docker runs: Docker Desktop on this Mac or any Linux box over SSH. Every Docker context is a machine you can train on.
    </td>
    <td width="25%" valign="top">
      <h3>🧪 Labs that grade themselves</h3>
      155 lab steps across 39 lessons. Press <b>Check</b> and PodDojo inspects the real containers and Kubernetes objects you created.
    </td>
    <td width="25%" valign="top">
      <h3>⏱ Real exam practice</h3>
      Three timed, performance-based CKAD mock exams (51 tasks) with partial credit and a score per curriculum domain.
    </td>
    <td width="25%" valign="top">
      <h3>⚡ Ultralight</h3>
      A native egui window (no Electron, no web view) that drives <code>docker</code>, <code>kubectl</code>, <code>kind</code> and <code>helm</code> exactly like you will in the exam.
    </td>
  </tr>
</table>

## The curriculum

| Track | What you learn | Lessons |
|---|---|---|
| **Docker Foundations** | containers as processes, the engine and contexts, run/inspect/debug, images and layers, Dockerfiles, multi-stage builds, registries and digests, volumes, networks, Compose, limits and hardening | 13 |
| **CKAD** (curriculum v1.37) | the lab cluster and kubectl at exam speed, Pods, workloads, Jobs/CronJobs, multi-container Pods, volumes, rolling updates, blue/green and canary, Helm, Kustomize, probes, logs, debugging, API deprecations, ConfigMaps, Secrets, resources and quotas, ServiceAccounts and RBAC, SecurityContexts, CRDs and Operators, Services, Ingress, NetworkPolicies, exam strategy | 26 |
| **Mock exams** | 16–18 tasks each, 2 hours, 66% to pass, weighted 20/20/15/25/20 like the real exam | 3 |

Every lesson has **Concepts** (with ▶ Run buttons on every shell snippet), a **Quiz** that explains
each answer, and a **Lab** with hints and worked solutions. A lesson is complete when you have read
it, passed the quiz (70%) and passed every lab step. Your **CKAD readiness** combines lessons done and
mock-exam scores, weighted like the exam.

<table>
<tr>
<td width="50%" valign="top"><img src="docs/images/lesson.png" alt="A CKAD lesson on NetworkPolicies with objectives, concepts and a table" width="100%"></td>
<td width="50%" valign="top"><img src="docs/images/lab.png" alt="A lab with four steps, each with Check, Hint and Solution buttons" width="100%"></td>
</tr>
<tr>
<td width="50%" valign="top"><img src="docs/images/cluster.png" alt="The lab cluster page listing pods with their status" width="100%"></td>
<td width="50%" valign="top"><img src="docs/images/exams.png" alt="The mock exams page" width="100%"></td>
</tr>
</table>

## Get started

You need Docker (Docker Desktop, or Docker Engine on Linux), `kubectl` and `kind`; `helm` for the
Helm lessons. On macOS:

```bash
brew install kubectl kind helm
```

Then build and run (Rust 1.99, pinned in `rust-toolchain.toml`):

```bash
cargo run -p poddojo --release
```

1. **Pick a machine** in the top bar. To add a Linux server once:
   `docker context create my-server --docker host=ssh://user@192.168.1.20`
2. Start with **Docker Foundations**, or jump to the CKAD track.
3. For the CKAD track, create the **lab cluster** (Cluster page, about a minute).
4. Work in the built-in console, or press **Open terminal** for a real shell that already points at
   your machine and lab cluster.

### Safety

Your machines and clusters are real, so PodDojo is careful:

- It only ever talks to Kubernetes through **its own kubeconfig** (context `kind-poddojo`) and refuses
  anything else. Your `~/.kube/config` and your other clusters are never read or touched.
- Labs name everything `dojo-…` and use their own namespaces. Lesson commands are validated: no
  prune, no "all containers", no `sudo`, no context switching.
- Removing a container you did not create through PodDojo asks first.

## The `dojo` CLI

The same progress, from any terminal (or for an AI tutor such as Claude Code or Codex):

```bash
dojo status            # completion, CKAD readiness by domain, where to continue
dojo next              # the lesson to continue with
dojo prepare <lesson>  # set the lab up
eval "$(dojo env <lesson>)"   # point this shell at your machine and the lab cluster
dojo check <lesson>    # grade the lab
dojo exam start ckad-mock-1   # start the clock; dojo exam grade when done
dojo status --json     # machine-readable, for agents
```

Run `cargo run -q -p dojo -- help` for everything.

## Under the hood

Pure Rust in a small workspace: `core` (curriculum, progress, grading), `runner` (machines, commands,
lab cluster), `ui` (egui), `apps/poddojo` and `apps/dojo`. See [docs/architecture.md](docs/architecture.md)
and, to write lessons, [docs/content-authoring.md](docs/content-authoring.md). Every lab and exam is
verified end to end on a real cluster with `dojo verify`.

```bash
cargo xtask ci     # fmt, clippy -D warnings, tests, content validation
```

## License

Code and original assets: MIT OR Apache-2.0. Docker, Kubernetes and CKAD are trademarks of their
owners; PodDojo is an independent study tool, not affiliated with Docker, Inc., the CNCF or the Linux
Foundation. The exam format changes: always check the official CKAD page and candidate handbook.
