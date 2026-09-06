# Workflow Strategy

The community fork's CI and release workflows use standard GitHub-hosted
runners throughout. Pull requests and `main` get one review-friendly CI run,
while the heavyweight full matrix is started manually to avoid duplicate runs
and notification noise.

## Pull Requests

- Required checks run against GitHub's synthetic merge commit, not the pull
  request head alone. This includes changes already on `main` and catches
  conflicts before they reach the branch.
- `bazel.yml` is the main pre-merge verification path for Rust code.
  It runs Bazel `test` and Bazel `clippy` on the supported Bazel targets,
  including the generated Rust test binaries needed to lint inline `#[cfg(test)]`
  code.
- `rust-ci.yml` keeps the Cargo-native PR checks intentionally small:
  - `cargo fmt --check`
  - `cargo shear`
  - `argument-comment-lint` on Linux, macOS, and Windows
  - `tools/argument-comment-lint` package tests when the lint or its workflow wiring changes

## Full Verification

- `bazel.yml` also runs as part of `blocking-ci.yml` on pushes to `main`.
  Bazel executes directly on the ephemeral GitHub runner and uses GitHub cache;
  this fork does not require BuildBuddy or self-hosted runners.
- `rust-ci-full.yml` is the full Cargo-native verification workflow.
  It is available directly or through the manually dispatched
  `postmerge-ci.yml`, keeping the heavier checks off the automatic path:
  - the full Cargo `clippy` matrix
  - the full Cargo `nextest` matrix via per-platform archive-backed shards
  - native Windows ARM64 nextest archives and shards
  - release-profile Cargo builds
  - cross-platform `argument-comment-lint`
  - Linux remote-env tests

## Rule Of Thumb

- If a build/test/clippy check can be expressed in Bazel, prefer putting the PR-time version in `bazel.yml`.
- Keep `rust-ci.yml` fast enough that it usually does not dominate PR latency.
- Reserve `rust-ci-full.yml` for heavyweight Cargo-native coverage that Bazel does not replace yet.

## Fork runner policy

- Linux x64/arm64 use `ubuntu-24.04` and `ubuntu-24.04-arm`.
- macOS Intel/arm64 use `macos-15-intel` and `macos-15`.
- Windows x64/arm64 use `windows-2025` and `windows-11-arm`.
- In a public repository these standard GitHub-hosted runners are free. “Local
  Bazel” means Bazel runs inside that temporary GitHub VM rather than through
  BuildBuddy remote execution; it never means a maintainer's computer.
- `blocking-ci.yml` cancels superseded runs for the same branch or pull request.
  `postmerge-ci.yml` is manual and also cancels an older full run for the same ref.
