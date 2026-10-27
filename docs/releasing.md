# Releasing

PodDojo has no hosted CI: GitHub Actions is disabled on the repository. Every gate runs on a
maintainer's Mac, against a real lab cluster, before a merge and before a release.

## Before merging a pull request

```bash
cargo xtask ci                                   # fmt, clippy -D warnings, tests, content validation
PODDOJO_CONTENT_DIR=content cargo run -q -p dojo -- verify all
for f in content/exams/*.toml; do
  PODDOJO_CONTENT_DIR=content cargo run -q -p dojo -- verify "exam:$(basename "$f" .toml)"
done
```

`dojo verify` needs the lab cluster (`dojo cluster up`). It runs every lab and exam for real:
checks must fail before a solution and pass after it, negative cases must be rejected and
alternative solutions accepted.

## Cutting a release

1. Bump `version` in the workspace `Cargo.toml` and add the release to `CHANGELOG.md`, in a pull
   request. Merge it.
2. From a clean worktree of `main`:
   ```bash
   cargo xtask release        # gates, signed PodDojo.app, zip and SHA256SUMS in target/release-assets
   ```
   The app is signed ad-hoc by default. With an Apple Developer ID it is signed with the hardened
   runtime and notarised, so it opens without any Gatekeeper warning:
   ```bash
   xcrun notarytool store-credentials poddojo-notary --apple-id <id> --team-id <team>   # once
   PODDOJO_SIGN_IDENTITY="Developer ID Application: <name> (<team>)" \
   PODDOJO_NOTARY_PROFILE=poddojo-notary cargo xtask release
   ```
3. Publish (the command is printed at the end of step 2):
   ```bash
   gh release create vX.Y.Z --target main --title "PodDojo vX.Y.Z" --notes-file <notes> \
     target/release-assets/PodDojo-vX.Y.Z-macos-arm64.zip target/release-assets/SHA256SUMS.txt
   ```

Without a Developer ID the release notes must say how to open the app the first time
(right-click › Open, or `xattr -dr com.apple.quarantine PodDojo.app`).

## Linux

PodDojo builds and its tests pass on Linux (checked in a `rust` container, see the pull request
that added this note). Build it with `cargo build --release -p poddojo -p dojo`; the app needs an
X11 or Wayland session with OpenGL.
