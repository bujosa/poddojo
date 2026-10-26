# Releasing

PodDojo has no hosted CI: GitHub Actions is disabled on the repository. Every gate runs on a
maintainer's Mac, against a real lab cluster, before a merge and before a release.

## Before merging a pull request

```bash
cargo xtask ci                                   # fmt, clippy -D warnings, tests, content validation
PODDOJO_CONTENT_DIR=content cargo run -q -p dojo -- verify all
for e in ckad-mock-1 ckad-mock-2 ckad-mock-3; do
  PODDOJO_CONTENT_DIR=content cargo run -q -p dojo -- verify exam:$e
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
   cargo xtask ci
   cargo xtask bundle
   ditto -c -k --keepParent target/bundle/PodDojo.app target/PodDojo-vX.Y.Z-macos-arm64.zip
   shasum -a 256 target/PodDojo-vX.Y.Z-macos-arm64.zip > target/SHA256SUMS.txt
   ```
3. Publish:
   ```bash
   gh release create vX.Y.Z --target main --title "PodDojo vX.Y.Z" --notes-file <notes> \
     target/PodDojo-vX.Y.Z-macos-arm64.zip target/SHA256SUMS.txt
   ```

The app is not signed or notarised yet; the release notes say how to open it the first time.
