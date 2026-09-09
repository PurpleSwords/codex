# Fork baseline audit — 2026-09-10

This is an evidence log, not a release approval. No new version is selected.

## Git baseline

- Clean starting worktree, `main` / `origin/main`: `c68c0d64b8896a2b41b4ea70fa3528b3b81916c5`.
- Repair branch: `repair/fork-ci-baseline`; main is unchanged.
- Origin and upstream each fetched once successfully. The first sandbox attempt failed before fetching because `.git/FETCH_HEAD` was read-only.
- Merge base: `ddf04ad26789d040f9ef6a96736f76602e35a6cc`.
- Fetched upstream/main: `ce2c2759ebee2d64565922f6f7365082284f9570` (214 upstream-only commits).
- 16 fork-only commits; merge-base diff: 50 files, +1623/-688 lines before repairs.
- Published `0.153.4` / `503cacb577` is historical release evidence, not evidence that blocking CI passed.

## Commit disposition

“Keep” below means retain the intent pending the listed verification, not that all platforms passed. No historical commits are rewritten or removed by this audit.

| Commit       | Category                          | Decision and verification gap                                                                                                                                                                                                                                  |
| ------------ | --------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `631a1cbad7` | npm installation/update + release | Keep separate scope and dynamic update URLs; rewrite release safety defaults. Packaging overrides have no dedicated fork packaging tests. Environment-driven branding needs launcher/install verification.                                                     |
| `fd93d9b05f` | release                           | Keep `CODEX_REPO_ROOT`: package assembler requires the checkout root.                                                                                                                                                                                          |
| `7c6233ed65` | runtime                           | Keep in-place edit intent. Existing `prompt_edit_reverts_before_selected_prompt_in_place`, first-prompt, backtrack selection/error snapshot tests exercise it. Both legacy rollback and paginated revert must be checked.                                      |
| `d9d036f3eb` | runtime                           | Keep lazy detection and bounded direct child probe. Existing hanging-process and explicit override tests cover these paths. `wait_with_output` after child exit can still wait on an inherited pipe; WSL interop behavior is not proven by a Linux sleep test. |
| `fd87136ba6` | CI + runtime lint                 | Keep legitimate formatting/lint fixes. Removal of stale code-mode sandbox manifest exception is narrow; verify manifest checker.                                                                                                                               |
| `e2a740f18d` | formatting                        | Keep mechanical Python formatting.                                                                                                                                                                                                                             |
| `0c005bf4fe` | runtime                           | Keep history-based count intent; no new regression test in this commit. Need coverage where visible prompts and history turns differ.                                                                                                                          |
| `c6fd88d6c0` | release                           | Keep upstream-shaped version numbering. Regex checks shape only, not correspondence to an upstream tag or unused npm version.                                                                                                                                  |
| `ccbacfe4d2` | release                           | Keep recovery intent; rewrite validation. Existing npm versions are skipped without content comparison; registry errors are treated as missing versions.                                                                                                       |
| `ee4ccb3f20` | release                           | Keep stripping and companion binary packaging. Release success does not substitute for runtime tests.                                                                                                                                                          |
| `466ba490e5` | release                           | Rewrite repair-run validation: downloaded artifacts must belong to the same source/version/workflow. Do not solve failures by longer timeouts.                                                                                                                 |
| `503cacb577` | release                           | Keep single-artifact directory layout fix. Validate mixed-run artifacts before reuse.                                                                                                                                                                          |
| `070992c461` | CI infrastructure                 | Rewrite automatic checks. Standard runners are appropriate; copying the full local Bazel matrix is not a validated baseline. Keep consolidated entrypoint/cancellation intent.                                                                                 |
| `141e476849` | CI formatting                     | Reassess/rewrite: enumerates all Rust files with one edition/config, bypassing Cargo module traversal. No dedicated regression test; process argument length is a Windows risk. Reverting blindly may reintroduce formatting failure.                          |
| `d13a68e694` | CI + SDK packaging                | Keep SDK codex/code-mode-host colocation; rewrite local Bazel automatic matrix. Four local jobs cannot replace remote execution capacity.                                                                                                                      |
| `c68c0d64b8` | CI Windows setup                  | Keep compatible work-volume fallback where used. Successful setup does not establish successful Windows compilation.                                                                                                                                           |

The fetched upstream has changes in prompt-edit orchestration, `app_backtrack.rs`, `app_server_session.rs`, CLI main, and npm staging. These require a deliberate merge review. It has no changes since the merge base in the two WSL fix files, `codex-cli`, or install-context. File overlap is not itself a confirmed merge conflict; no merge/rebase has been performed.

## CI evidence

[blocking-ci 34038689058](https://github.com/PurpleSwords/codex/actions/runs/34038689058) ran at the starting HEAD. GitHub reports 13 successful, 12 failed, 6 cancelled jobs; the overall conclusion is cancelled. Aggregate gate failures are included, so 12 is not a count of independent defects.

- Both SDK jobs passed; retain the companion binary layout.
- macOS ARM clippy job `101501455066`: unused `wiremock::matchers::body_json`, `core/tests/suite/openai_file_mcp.rs:47`.
- Windows native job `101501454914`: ring/aws-lc objects reference `__stack_chk_fail`, `___chkstk_ms`, `__mingw_fprintf`, and CRT functions. `BUILD.bazel` defines separate gnullvm/msvcrt and MSVC platforms; `.bazelrc` defaults the Windows host to the former. A target named MSVC does not prove its C and host dependencies use MSVC.
- Linux GNU job `101501455047` includes failures in `permission_profile_intersection::tests::effective_workspace_intersection_preserves_network_metadata_and_temp`, `suite::managed_proxy::managed_proxy_mode_routes_through_bridge_and_blocks_direct_egress`, and `suite::v2::executor_skills::selected_executor_root_exposes_plugin_skill_and_forwards_budget_warning`. Do not relabel these as flakes without reproducing them.
- Several Bazel jobs consumed approximately 180 minutes; argument lint also timed out at its shorter limit. Time limits must remain budgets, not the primary repair mechanism.

## Repair order

1. Safe release defaults; focused runtime checks; real Linux CLI build and smoke test.
2. Fork npm pack/dry-run/isolated installation checks, preserving six platform aliases and `fork-release.yml` trusted publishing identity.
3. Small automatic Cargo/package/policy gate with explicit coverage; heavy platform testing manually dispatched. Reuse verified prebuilt public V8 artifacts, not RBE. Windows Cargo uses MSVC throughout.
4. Same-source artifact recovery checks and independent upstream-version verification before any new release candidate.
5. Validate the new gate on GitHub-hosted runners. Do not declare stable CI on the basis of local checks or reduced coverage alone.

## Separate backlog

Read `/home/purplesword/.codex/LOG_DB_WRITE_FIX.md` in full as historical context; no user configuration or database was changed.

- models.json refresh: investigate source, refresh cadence, and compatibility separately.
- Slow image input: measure clipboard/PowerShell/PNG work and event-loop blocking separately.
- High-frequency SQLite writes: independently verify filtering before enqueue/write; the historical local trigger is not a source-level fix.

## First repair stage and local verification

- Automatic Bazel/full argument-lint/SDK matrix calls removed from the blocking entrypoint, replaced by a scoped Linux Cargo workflow. No runtime patch was deleted. The full diagnostic workflows remain available explicitly; their failures are not considered fixed.
- Native macOS/Windows checks now have a manual Cargo workflow; Windows uses MSVC targets without Bazel outputs. Hosted validation is pending.
- Release creation defaults to false; build-only dispatch is permitted on repair branches. Publishing still requires main. OIDC/provenance and six platform packages are unchanged.
- Removed the confirmed unused import in `openai_file_mcp.rs`.
- `cargo build --locked -p codex-cli --bin codex`: passed (6m31s with existing cache); built Linux CLI `--version` and `--help` passed. Development version remains `0.0.0`; no release version was set.
- `just test --locked -p codex-tui --lib -E 'test(prompt_edit) | test(backtrack) | test(keyboard_modes) | test(resize_reflow)'`: 58 passed, 4204 filtered out.
- `CARGO_INCREMENTAL=0 just test --locked -p codex-install-context`: 17 passed.
- `CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=4 just test --locked -p codex-core --test all -E 'test(openai_file_mcp)'`: 4 passed, 1681 filtered out. The first command used the wrong test target `suite`; after correction and a machine restart, rustc hit an incremental verification ICE in codex-config. Disabling incremental compilation completed successfully; no compiler flags were changed in product source.
- Three repository manifest/boundary/lint-consistency checks passed with Python 3.12. The default local Python 3.10 lacks tomllib; the package builder tests additionally require CODEX_REPO_ROOT. With those prerequisites, 15 package-builder tests passed.
- New fork npm test passed: real `npm pack` for the root and six synthetic native payloads checks scope, aliases, versions, platform selectors, and companion binaries. Fixtures are not six real platform builds.
- Built the current root npm tarball at existing version `0.153.4`; isolated npm installation resolved the already-published Linux platform dependency. Installed `--version` (`0.153.4`) and `--help` passed. This validates the current launcher with the published binary, separately from the locally built source CLI. The global official installation was not replaced.
- Network-enabled `npm publish --dry-run` correctly rejected `0.153.4` as already published. The earlier sandbox dry-run returned success without the same registry check and is not accepted as complete preflight evidence. No package was published. A new release needs a corresponding unused upstream version and successful preflight.
- Actionlint 1.7.12 passed for the modified/new workflows. `just fmt`, Prettier repository check, and `git diff --check` passed. Full workspace tests require user approval per AGENTS.md and have not run.

Release readiness: **not ready**. Remaining gates include hosted cold-run timing/results, native platform checks, broader runtime edge cases, artifact-source/version validation for repair/recovery runs, and an unused upstream-aligned version. Do not infer approval from this first repair stage.

## Next upstream sync

1. Start from a clean main after approved repairs land; record its SHA and currently published version.
2. Fetch each remote once and create `sync/upstream-YYYY-MM-DD` from main.
3. Record merge base and upstream SHA; review upstream changes to each fork-owned file in the table above.
4. Use a normal merge of the recorded upstream SHA (no reset, rebase, or force-push); resolve runtime, packaging, and workflow overlaps independently.
5. Run focused TUI/install-context tests, Linux build/smoke, npm pack/install, and the automatic hosted checks; run manual Windows MSVC/macOS coverage before a release candidate.
6. Update this ledger and inspect the merge-base diff. Keep scope/version metadata separate from runtime patches. Select an unused corresponding upstream version only after checks pass.
7. Run the release workflow with both publication inputs false; inspect all six platform artifacts. Obtain explicit user confirmation before npm publication, tag creation, or GitHub Release creation.
