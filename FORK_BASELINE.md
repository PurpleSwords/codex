# Official 0.153.4 baseline

The rebuild starts at upstream tag `rust-v0.153.4`, commit
`3d2ee51ca2d5db578f328aa75e20aa22c0197c9a`.

- Local `main` points to the unmodified official release.
- `archive/main-before-01534` preserves the previous fork at `c68c0d64b8`.
- Commits on `ci/official-01534` are restricted to CI and this maintenance record.
  Local source, test, formatter and lockfile edits are deliberately uncommitted.
- Remote `main` has not been replaced. Replacing its history requires approval.

The previously published fork package `0.153.4` was based on a different upstream
revision. Its successful publication is not evidence that this baseline passes.
That npm version is already used and must never be published again.

## Paused build metadata correction

The official release commit changes the workspace version to `0.153.4` but leaves
149 local package versions at `0.0.0` in `Cargo.lock`. Consequently, Cargo rejects
`--locked` before running tests. The lockfile correction changes only those local
versions, without upgrading external dependencies. Keep `--locked` in CI.
This correction is local only, pending approval to resume non-CI changes. A
CI-only checkout therefore still fails its locked Cargo build; do not regenerate
the lockfile inside CI to hide the mismatch.

## Automatic checks

One `blocking-ci` entrypoint handles PRs and main pushes:

- Both `rust-ci` and `bazel` remain required dependency families. The Rust family
  owns the shared Cargo build; SDK checks consume its binaries without rebuilding.
- Existing repository formatting, package-builder tests, spelling, dependency
  policy and changed-blob checks.
- Ordinary Ubuntu 24.04 Cargo build of all workspace binaries, including the CLI,
  code-mode companion and runtime test helpers.
- CLI version/help smoke checks using an isolated configuration directory.
- The complete `core` integration test target, including the Responses client.
- TUI library and installation-context tests.
- Python SDK lint, formatting and tests in the original glibc Docker environment.
- TypeScript SDK build, lint and tests, using this candidate's CLI and companion
  from one shared artifact rather than building the CLI a second time.
  The artifact is uploaded before integration tests. Test failures do not suppress
  independent TUI, installation-context or SDK results; build failures still
  prevent Rust tests and leave TypeScript without its required artifact.
- Unused dependency detection, V8 manifest checksums and CI-script tests.
- Real Bazel smoke tests for `ansi-escape` and `utils/absolute-path`, in addition
  to lockfile validation. This is explicitly not the complete Bazel test sweep.

The Cargo job uses two build workers, no debug information or incremental state,
a dependency-keyed cache, and checksum-verified public V8 artifacts. No private
secrets, RBE, self-hosted runners or larger runners are required.

The automatic Bazel family calls `fork-bazel.yml`, not the unchanged internal
matrix in `bazel.yml`. The latter and the old full/V8 workflows remain historical
diagnostic tools; their presence does not imply public-runner compatibility.
This draft is not the full upstream test suite and is not ready for adoption
until the coverage gaps below are resolved.

## Coverage migration ledger

| Upstream check                                              | Current draft                                                           |
| ----------------------------------------------------------- | ----------------------------------------------------------------------- |
| Repository format, spelling, dependency policy, blob policy | Retained                                                                |
| Python and TypeScript SDK checks                            | Retained; hosted runner and shared Cargo artifact                       |
| Cargo format                                                | Retained explicitly alongside repository-wide formatting                |
| Cargo shear                                                 | Original `rust-ci` job retained                                         |
| V8 manifest checksums and CI-script tests                   | Public Bazel job                                                        |
| Bazel lockfile drift                                        | Public Bazel job                                                        |
| Full Rust tests across platforms                            | Explicit Bazel `tests` mode on native Linux/macOS; not yet validated    |
| Cross-platform Clippy                                       | Explicit Bazel `clippy` mode on native Linux/macOS; not yet validated   |
| Benchmark smoke                                             | Original command retained on manual `rust-ci` runs; not yet validated   |
| Argument-comment lint                                       | Explicit Bazel aspect sweep; also included in manual `rust-ci` runs     |
| Argument-comment lint package tests                         | Original conditional Linux job retained; wrapper tests verified locally |
| Release-build and bwrap validation                          | Explicit Bazel `release-build` mode; not yet validated                  |
| Windows and Linux musl Bazel lanes                          | Still unresolved; native smoke is not equivalent coverage               |

Move expensive coverage to explicit or periodic runs only with its scope recorded.
Do not treat an unimplemented replacement as a passing check.

## Running the retained heavy checks

Use **Actions > Public Bazel checks > Run workflow**, selecting this branch,
`ubuntu-24.04`, `macos-15` (ARM) or `macos-15-intel`, and exactly one check:
`tests`, `clippy`, `release-build` or `argument-comment-lint`. Target discovery
and release-only compiler flags are retained from the upstream scripts. Select
`smoke` for the same bounded target set used automatically. Windows is deliberately
not selectable until its native toolchain boundary is fixed.

Use **Actions > rust-ci > Run workflow** for the Rust baseline plus the original
`just bench-smoke` command and the full Linux argument-comment aspect sweep.
No schedule is enabled until these lanes have measured successful hosted runs.

Public Bazel execution uses two jobs, no remote endpoints and job-local output
and repository-contents directories. Only downloaded archives and local action
outputs are cached. LLVM repository overlays contain absolute symlinks, so their
contents cache must not be shared between output bases.

## Local evidence so far

These measurements include the uncommitted lockfile/formatter corrections where
needed. They are not results for a clean CI-only checkout or a hosted CI run.

- Official client integration module: 47 passed, 1596 other tests excluded by the
  module filter. This does not establish the cause of the old CI failures.
- Cargo lockfile diff: exactly 149 local version changes, no external updates.
- `just bazel-lock-update`: passed, no Bazel lockfile changes.
- V8 manifest checksums, workflow syntax and changed-file whitespace checks passed.
- CLI and companion build passed after retry with checksum-verified public V8
  artifacts; isolated CLI version/help checks passed (`0.153.4`).
- TypeScript SDK build/lint and all 48 tests passed against that CLI.
- Cargo shear passed; all 35 CI-script tests passed.
- Python SDK: 131 passed, 38 skipped by upstream conditions; lint/format passed.
  Its pinned runtime wheel is `0.147.0`, not the newly built CLI. The initial local
  run shared a virtual environment with formatting and was invalid; isolated runs
  exposed and then verified the formatter correction described below.
- Full workspace binary build passed. The first full core integration run had
  1498 passes and 137 failures, including 124 explicit missing-helper errors.
  After building helpers: 1621 passed, 14 failed, 8 skipped. Missing-helper errors
  dropped to zero. Thirteen failures contain `GLIBC_2.38` incompatibility messages
  (the local host has glibc 2.35). The remaining async-hook user-turn timeout was
  traced to a second hook completing against an already-open fixture gate, causing
  a third Responses request after the two-response mock was exhausted (404).
  A local fixture-gate correction passed the targeted hook/MCP selection (7 tests).
  That correction and the unused MCP import removal remain uncommitted.
- The TUI/installation-context library run completed with failures, including
  pending TUI snapshots. These snapshots have not been accepted; source/test work
  is paused. Do not describe the Rust baseline as passing.
- Hosted validation has started, but has not passed. No product patches were migrated.
- Public Bazel smoke: both test targets passed in 183.942 seconds with two local
  execution slots and 1681 actions. A fresh output base and repository-contents
  cache were used, but existing archive/action caches were available; this is not
  a hosted cold-cache measurement. An initial run using old repository contents
  failed on dangling LLVM overlay symlinks before compilation.
- Argument-comment wrapper helpers: all 6 tests passed. The nightly lint package
  tests and heavy aspect/benchmark lanes have not been executed locally.

## Formatter correction

Upstream `9127f21890` adds a test requiring repository-wide Rust file discovery,
but the release's formatter still invokes only `cargo fmt`. The isolated Python
SDK run reproduced this single failure (130 passed, 1 failed, 38 skipped).
The uncommitted implementation meets the existing test without weakening assertions.
This independently supports the purpose of old fork commit `141e476849`; the
rewrite retains discovery, stable ordering, configuration and check-mode behavior.

## Official CI history

The official release commit did not have a blocking-CI run. Its parent came from
[upstream PR 42878](https://github.com/openai/codex/pull/42878). That PR's
[blocking run 33926390711](https://github.com/openai/codex/actions/runs/33926390711)
ended with 20 successful, 9 failed and 2 skipped jobs. Bazel test jobs passed;
formatting, SDK, spelling and Clippy failures remained. The macOS Clippy log
includes the same unused `body_json` import found locally.

The release commit's [release run 33926543788](https://github.com/openai/codex/actions/runs/33926543788)
also ended in failure (winget), despite successful build/npm jobs. This establishes
an identifiable official baseline, not an all-green baseline. It does not establish
that every failure in the old fork or the local host has the same cause.

## CI-only hosted probe

[PR 2](https://github.com/PurpleSwords/codex/pull/2) targets the separate official
baseline branch, not the old remote main. Its initial
[run 34468608276](https://github.com/PurpleSwords/codex/actions/runs/34468608276)
confirmed the following before all jobs completed:

- Locked Cargo build and Bazel dependency loading fail on the official lockfile
  version mismatch. Cargo deny and shear complete their main checks, but modify
  that lockfile and correctly fail the clean-worktree gate.
- Repository checks find a stale code-mode feature exception in the CI manifest
  verifier. Remove that exception, not the verifier or the crate's lint policy.
- Codespell mistakes the valid Rust flag `OFlags::WRONLY` for a spelling error.
  Add only `wronly` to the existing word allowlist; keep source files checked.
- Rust formatting, changed-area detection and blob policy passed. This is not an
  all-green run, and the build failure prevents runtime coverage.

The verifier correction passes locally against all workspace manifests, and the
spelling correction passes a targeted codespell check. Neither requires a Rust
source, Cargo manifest or lockfile change. The blocking lockfile correction remains
outside the CI-only commits until separately approved.

## Migration gates

Do not transplant product changes until this CI baseline has a successful hosted
run. Preserve failing tests and diagnose failures instead of filtering them out.
Then use separate reviewable changes for Double Esc, WSL terminal probing, npm
installation/update behavior and publishing. Preserve `fork-release.yml`, OIDC,
provenance and six platform packages when restoring the publishing workflow from
the archive; do not publish, tag or create a release without approval.

Models catalogue refresh, slow image input and frequent disk writes are separate
backlog items, not part of CI repair.
