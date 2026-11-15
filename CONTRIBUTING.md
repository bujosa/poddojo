# Contributing to PodDojo

Help improve the Docker/CKAD explanations, quizzes, labs, native UI, accessibility or documentation. Start with [ROADMAP.md](ROADMAP.md), [architecture](docs/architecture.md) and the [content authoring guide](docs/content-authoring.md).

## Local setup

Install Git and rustup; `rust-toolchain.toml` pins the Rust toolchain. The packaged target is macOS Apple Silicon; see [releasing](docs/releasing.md) for the documented Linux build. Real Docker/CKAD work also requires Docker, kubectl and kind, plus Helm for Helm lessons. The kubectl version must be compatible with the lab cluster; use the app’s Doctor to check your setup.

```sh
export PODDOJO_HOME="$(mktemp -d -t poddojo-contrib)"
cargo run -q -p dojo -- status
cargo run -p poddojo --release
```

`PODDOJO_HOME` isolates files, profiles and progress. It does **not** create a separate Docker engine or give the fixed-name lab cluster a new identity. Use a dedicated disposable Docker engine/context for destructive lab verification, not an existing learner’s engine/cluster. Never modify or switch `~/.kube/config` or the current Docker context as part of a test.

## Map of the code

- `crates/core`: curriculum, grading, progress and review; no system commands.
- `crates/runner`: Docker contexts, the guarded kind cluster, tools and terminal.
- `crates/ui`: native screens and background jobs.
- `apps/poddojo`, `apps/dojo`: desktop app and CLI sharing learner data.
- `content`: tracks, English lessons, quizzes, exam tasks and reference pages.

## Validation

```sh
cargo xtask ci
```

This runs format, Clippy with warnings denied, tests and curriculum validation. Content changes additionally require real verification of each affected lesson or exam on a dedicated lab engine:

```sh
cargo run -q -p dojo -- validate content
PODDOJO_CONTENT_DIR=content cargo run -q -p dojo -- verify <lesson-id>
PODDOJO_CONTENT_DIR=content cargo run -q -p dojo -- verify exam:<exam-id>
```

The placeholders above must be replaced with the actual affected IDs. Verify setup/cleanup, reject wrong or incomplete work, accept valid alternatives, and prove the reference solution passes. Broader release verification and signing are described in [releasing](docs/releasing.md). State precisely when real cluster verification did not run and why.

For UI changes, use an isolated profile with fictional data and review the native app in wide/compact windows with keyboard access. Do not add automated presentation/snapshot tests. Preserve meaningful core/runner tests.

## Write a lesson or lab

Follow [Writing PodDojo content](docs/content-authoring.md), including the reference lesson, English teaching style, TOML/front-matter schema, quiz explanations and negative/alternative cases. Set `PODDOJO_CONTENT_DIR=content` to review source content without rebuilding.

All resources must use the documented `dojo-…` names and the app’s own kubeconfig/context guard. No broad prune, `sudo`, context switching or cleanup of unrelated resources. Keep Docker/Kubernetes trademarks and third-party artwork out of original app assets.

## Send a contribution

1. Open an issue for a substantial change so the learning goal or behavior can be agreed first. Small fixes can go straight to a PR.
2. Fork this repository, clone your fork, add `https://github.com/bujosa/poddojo.git` as `upstream`, and create a feature branch from `upstream/main`. Maintainer branches use `bujosa/<short-purpose>`.
3. Keep the change focused. Preserve existing learner data, unrelated changes and the app’s English UI. Use Conventional Commits, for example `fix: preserve saved drafts` or `docs: explain local setup`.
4. Open a PR against `bujosa/poddojo:main`. Explain the problem, resulting behavior, commands/results, manual review and any limits. Include a screenshot for a visual change, using fictional data and an isolated profile.

There is no hosted CI. Maintainers verify the relevant local gates before merge; a fork does not need access to a maintainer’s machine, token or Apple account. Never commit secrets, real learner drafts, local settings, build output or private screenshots. Use original or compatibly licensed assets and update `ATTRIBUTION.md` and notices when adding dependencies or artwork.

Be respectful in issues and reviews. Review may ask for a smaller scope, stronger examples, clearer explanations or additional checks. See `AGENTS.md` for the repository’s engineering agreements; it is also useful when working without an agent.

## Good first contributions

- Reproduce a reported issue with clear steps and a fictional profile.
- Improve a confusing explanation, hint, keyboard instruction or diagnostic.
- Fix a narrow accessibility or layout problem and include native visual evidence.
- Review an existing issue or roadmap item before starting a large feature.

Contributions are credited in the commit history and [GitHub contributors graph](https://github.com/bujosa/poddojo/graphs/contributors).
