# Fork Workflow Strategy

`blocking-ci.yml` validates PR merge candidates and pushes to `main`. Both use
the actual diff. Missing Git history and manual Rust CI runs fall back to baseline
checks. Direct pushes retain validation.

## Automatic checks

- Root Markdown and `docs/` Markdown changes skip Cargo, Bazel, dependency and
  SDK checks. Embedded prompts under `codex-rs/` still receive code checks.
- Changes limited to `codex-rs/models-manager/models.json` and its model-picker
  snapshot use `model-catalog-extensions`: model manager/provider unit tests and
  focused TUI picker tests, without building all workspace binaries or running
  the full core integration suite.
- Other code changes keep the Cargo baseline. SDK and relevant CLI/API changes
  run SDK tests using binaries produced by that baseline.
- Bazel smoke runs for build definitions, dependencies, tooling and CI changes.
  Dependency checks run for Cargo manifests/locks, deny policy and CI changes.
- Repository, formatting, spelling and blob-size checks remain enabled.
- `CI required` accepts only explicitly planned skips. Failed/cancelled checks,
  failed change detection and unexpectedly skipped required jobs still fail.

## Diagnostics and releases

- The baseline runs integration regressions as part of the full suite once.
  Ten-round rollback checks remain in the manual `core-regressions` profile;
  twenty-round race checks remain in `integration-races`.
- A newer `fork-checks` run supersedes an older run for the same PR/ref and profile.
- Six-platform release validation remains manual. Use it for a release candidate,
  not for each model catalog change. Publishing reuses validated artifacts.
- PR validation and relevant post-merge validation both remain: PR success alone
  does not prove that a different final merge tree has passed.

Run routing and result-policy tests without compiling Rust:

```sh
python3 -m unittest discover -s .github/scripts -p 'test_ci_*.py'
```
