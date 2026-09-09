# Fork workflow strategy

`blocking-ci.yml` is the single automatic entrypoint for PRs targeting main and
pushes to main. It cancels superseded runs and reports one `CI required` gate.
The gate evaluates the checked-out PR merge revision.

Automatic coverage:

- `fork-checks.yml`: a Linux Cargo build of the CLI and code-mode host, CLI smoke
  checks, focused prompt-edit/WSL regression tests, and install-context tests.
  Public checksum-verified V8 artifacts avoid building V8 from source.
- Repository formatting and boundary checks, package builder tests, and real
  `npm pack` checks of the fork root package and all six platform fixtures.
- Dependency policy, spelling, and blob-size checks.

This is deliberately scoped coverage. Passing it does not mean the upstream
full test suite or all release targets passed. The 60-minute Cargo job budget
must be measured on a cold standard runner; do not extend it to mask failures.
See `FORK_AUDIT.md` for the validation status and historical failures.

`postmerge-ci.yml` is manual and groups `fork-platforms.yml` and `sdk.yml`.
Native macOS Intel/ARM and Windows x64/ARM use standard GitHub-hosted runners.
Windows Cargo uses MSVC host/target dependencies; it does not consume Bazel's
gnullvm outputs. SDK packaging keeps codex and codex-code-mode-host together.

The old `bazel.yml`, `rust-ci.yml`, `rust-ci-full.yml`, and V8 workflows remain
available for explicit diagnosis. They are not called by the automatic gate.
Their known failures and long runtimes remain open issues, not ignored passes.

`fork-release.yml` retains its npm Trusted Publisher filename, OIDC permission,
provenance, and six-platform package layout. Both publication inputs default to
false. Build-only checks can run on a repair branch. Publication remains main-only
and requires explicit user confirmation; a successful build or npm dry-run is
not release approval.
