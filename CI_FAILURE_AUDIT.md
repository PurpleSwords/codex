# CI failure audit — official 0.153.4 baseline

## Scope and evidence

Owner direction: fix CI-caused problems only. For source/test problems, record
the cause and proposed follow-up without changing code, assertions or snapshots.
Existing local edits in `hooks.rs`, `openai_file_mcp.rs`, `scripts/format.py` and
pending snapshots are preserved, not included in this diagnostic change.

Reference: [run 34470641717](https://github.com/PurpleSwords/codex/actions/runs/34470641717),
head `605e2760538860fb7077535e630b31a23dfe7fdf`, ordinary Ubuntu 24.04 runner.
Final jobs: 10 successful, 4 failed, 1 intentionally skipped manual-only aspect
sweep. Two failed jobs are aggregate gates; the actual failures are the Linux
Cargo test job and Python SDK job.

| Suite                |                          Passed | Failed | Skipped |
| -------------------- | ------------------------------: | -----: | ------: |
| Core integration     | 1557 (including 3 retry passes) |     78 |       8 |
| TUI library          |                            4048 |     28 |       2 |
| Installation context |                              16 |      0 |       0 |
| Python SDK           |                             130 |      1 |      38 |

Workspace binaries built in 9m24s. CLI version/help, SDK artifact transfer,
TypeScript SDK, Bazel smoke, repo checks, Cargo deny/shear, formatting, spelling,
and argument-comment lint package tests passed. Passing Bazel smoke does not
establish full Bazel or cross-platform coverage.

## A. Restricted Linux sandbox abort — CI resource-layout defect identified

Representative evidence in job `102849560149`:

- `agents_md::restricted_project_without_instructions_starts_successfully`
  fails twice while initializing the session, with helper SIGABRT and empty stderr.
- `approvals::approval_matrix_covers_group::read_only` fails twice on
  `echo trusted-read-only`, exit 134. The preceding approved writes succeed.
- The workspace-write approval group also returns 134; full-access group passes.
- Restricted MCP file reads, permission grants, workspace roots and unified-exec
  also fail. Missing tool output or approval events alone do not prove an abort.

Code trail (unchanged relative to the official baseline):

- `core/tests/suite/agents_md.rs:590`: this test adds a denied private path to a
  read-only profile; startup at line 617 fails before submitting the model turn.
- `exec-server/src/fs_sandbox.rs:130`: selects the configured self executable as
  the filesystem helper, transforms it through the required sandbox manager.
- `exec-server/src/fs_sandbox.rs:430`: reports child status and captured stderr;
  the existing log does not identify which inner process triggered the abort.
- `linux-sandbox/src/launcher.rs:126`: prefers a capable system bubblewrap and
  otherwise chooses a bundled executable. Host and Cargo-built binaries must not
  be assumed interchangeable without measuring the selected launcher.

Ruled out or bounded:

- This failure occurs before Responses requests, unlike the separate hook mock
  exhaustion below. It is not the old zlib download 404.
- Runner logs explicitly show both `kernel.unprivileged_userns_clone = 1` and
  `kernel.apparmor_restrict_unprivileged_userns = 0`. Do not claim setup skipped
  these settings in this run.
- Local WSL2/glibc 2.35 sandbox read-only and workspace-write echo probes pass.
  This does not reproduce the hosted Ubuntu 24.04 environment.
- Downloaded the actual cloud CLI artifact. A local Docker probe using a different
  userland and locally built bwrap stopped at an exec permission error, not the
  hosted SIGABRT. It is not equivalent evidence and does not establish a cause.

CI action: add a bounded sandbox preflight on the actual runner after building,
before the long integration suite. Record kernel/libc/bwrap versions and isolated
read-only/workspace-write command status. On failure, collect a process/filesystem
strace without model request payloads. A traced success must not erase an initial
failure; all existing integration tests still run. Upload diagnostic files, not
the isolated configuration home. No sandbox enforcement is disabled by this probe.

Next decision: identify the executable and syscall immediately preceding abort.
Only alter CI dependencies/environment if that trace establishes the cause. If it
identifies a source defect, add the evidence here and leave the code unchanged.
Do not classify all 78 core failures as one cause before this comparison.

Update from [run 34479401425](https://github.com/PurpleSwords/codex/actions/runs/34479401425):
the exact-test trace now identifies a concrete CI layout defect. In artifact
`core-sandbox-trace`, `agents-md.strace:66270` identifies process 214930's executable
as `cargo-target/debug/deps/all-11afbd0adb5ffdbe`. Lines 66369–66376 show all bundled
bwrap candidates missing: `debug/deps/codex-resources/bwrap`,
`debug/codex-resources/bwrap`, and `debug/deps/bwrap`. System PATH lookup also
finds no bwrap. Line 67226 records this process sending itself SIGABRT.

The lookup at `linux-sandbox/src/bundled_bwrap.rs:94` explains the distinction:
bare bwrap is searched next to the executable, while the parent is searched only
for `codex-resources/bwrap`. Cargo builds `debug/bwrap`, which works for the CLI
at `debug/codex` but is not a candidate for a test executable under `debug/deps`.
`linux-sandbox/src/launcher.rs:50` panics when no launcher is available. The trace
does not establish the precise panic-to-abort mechanism, which remains a separate
source diagnostic concern; it does establish missing launcher discovery.

CI repair: after the workspace build, install its exact bwrap binary at
`debug/codex-resources/bwrap` and compare the bytes. This follows the existing
lookup contract, does not alter source or test assertions, and does not replace
the vendored tool with an unrelated system version. Await the hosted result to
measure which failures disappear; do not assume every core failure is resolved.

## B. Python SDK formatter contract — source/tooling mismatch, paused

Job `102859464239` has one failure:
`test_root_format_driver_covers_all_formatter_groups`, assertion at
`sdk/python/tests/test_artifact_workflow_and_binaries.py:248`.

The existing test expects repository-wide Rust discovery and a `rustfmt` command
including a Rust file outside `codex-rs`. The official formatter supplies only
`cargo fmt`. This mismatch is deterministic and independent of the SDK binary or
runner sandbox. Earlier isolated local validation with the uncommitted formatter
correction yielded 131 passed / 38 skipped. Do not remove or relax the test.

Proposed later repair: review the small formatter implementation correction and
its discovery/check-mode behavior separately. No formatter change in this stage.

## C. TUI snapshots — release-version coupling, paused

The cloud diff for `status_snapshot_cached_limits_hide_credits_without_flag`
(`tui/src/status/tests.rs:2151`) shows `OpenAI Codex (v0.0.0)` in the expected
snapshot versus `OpenAI Codex (v0.153.4)` in actual output, with adjusted padding.
This is consistent with the official release version bump retaining development
snapshots. It is not evidence of a runtime rendering regression by itself.

The 28 failures cover status, startup headers and update notices. The displayed
status case establishes version coupling for that case, not every failing
snapshot. Each remaining diff still requires comparison for non-version changes.
Do not batch-accept snapshots or spoof the product version in CI to obtain green.
Proposed later repair: normalize the version in test fixtures where appropriate,
then separately review genuine layout/content changes. No snapshot edits now.

## D. Async hook fixture and other core failures — keep separate

The cloud final failure list includes
`hooks::async_hook_finishing_while_idle_waits_for_the_next_turn::user_turn`.
Earlier local tracing established a fixture race: the first release gate remains
open, so a second hook can finish during the next turn and provoke a third model
request after the two-response mock is exhausted. Local gate correction passed
the targeted hook/MCP selection (7 tests), but remains uncommitted. Do not fold it
into CI environment repair, raise its timeout, or add extra mock responses blindly.

`compact_resume_fork::snapshot_rollback_followup_turn_trims_context_updates` and
other missing-output/approval failures remain unclassified. Preserve their names
and the original run logs; compare again after the sandbox cause is established.
The known unused MCP import is also unchanged and remains a separate source lint
issue, not a reason to disable Clippy.

## Handoff boundary

### Resource-layout repair verified: run 34482921222

[Run 34482921222](https://github.com/PurpleSwords/codex/actions/runs/34482921222)
at `2e9b66b512` reduced the core result to **1632 passed / 3 failed / 8 skipped**
(2 retry passes). No SIGABRT appears in the Cargo job log. The exact AGENTS.md
diagnostic test passes as well. This verifies the CI resource-layout repair;
the full workflow is still red. TUI remains 4048 passed / 28 failed / 2 skipped,
and installation context remains 16 passed. Python SDK still fails its formatter
contract test; TypeScript SDK and Bazel smoke pass.

The remaining three core failures are now attributable to test/code contracts:

1. `hooks::async_hook_finishing_while_idle_waits_for_the_next_turn::user_turn`:
   the previously diagnosed async fixture release-gate race; both cloud attempts
   time out waiting for the next turn. Keep the earlier local correction paused.
2. `skill_approval::shell_zsh_fork_skill_scripts_ignore_declared_permissions`:
   `skill_approval.rs:119` waits for either ExecApprovalRequest or TurnComplete;
   receiving TurnComplete consumes it and returns None. At lines 207–213 the test
   asserts that result is None and then calls `wait_for_turn_complete` again.
   With a normal no-approval completion there is no second event to receive. The
   cloud reports `core/tests/common/lib.rs:354`, `timeout waiting for event` on
   both attempts. Proposed later repair: make the test wait once for completion
   while asserting that no approval was requested; retain output/write-denial
   assertions. Do not raise the timeout or change the approval implementation.
3. `unified_exec_zsh_fork_approvals::unified_exec_zsh_fork_parent_approval_preserves_denied_reads`:
   the test creates a concrete `secret.env` path at lines 61–65. Its custom profile
   conversion at lines 703–705 wraps every deny entry in `FileSystemPath::GlobPattern`,
   even when the path contains no glob metacharacters. The Linux splitter at
   `linux-sandbox/src/bwrap.rs:754–756` returns None when no metacharacter exists;
   the caller at lines 712–715 reports an unsafe/unexpandable glob. The logged path
   has a non-root prefix but no wildcard, matching this branch exactly. This is
   not a runner permissions error or evidence of a successful denied read.
   Proposed later repair: use the production path conversion contract for literal
   deny paths in this test; separately decide whether literal GlobPattern inputs
   should be supported by the runtime. Do not change the CI checkout directory.

No new CI fix is justified by these three remaining failures. The code/test
repairs remain paused under the owner's instruction. This conclusion concerns
the diagnosed automatic run, not yet-unvalidated heavy Windows/musl/macOS lanes.

Follow-up [run 34475507840](https://github.com/PurpleSwords/codex/actions/runs/34475507840)
at `bcb66dc439` completed with core 1558 passed / 77 failed / 8 skipped, TUI
4048 passed / 28 failed / 2 skipped, and installation context 16 passed.
Both CLI preflight policies passed with empty stderr. The diagnostic artifact
reports kernel `6.17.0-1022-azure`, glibc 2.39, userns enabled and the AppArmor
restriction disabled. No system bwrap path was printed. Therefore the CLI probe
did not reproduce the test abort and produced no failure strace.

`core/tests/common/test_codex.rs:633` constructs local runtime paths with
`std::env::current_exe()` (the test binary), plus the resolved Linux sandbox
executable. This differs from exercising the CLI directly. The next diagnostic
captures the exact AGENTS.md test with one test thread and zero retries after the
original core suite fails. It has a two-minute bound and uploads its process/file
trace. It does not replace the suite or clear its failure, and tracing may itself
alter process behavior. The abort's root cause remains unconfirmed.

No source or test fixes are committed by this stage. No test is removed, no
assertion is weakened, no global timeout is increased, and no flaky rerun is used
as a substitute for diagnosis. Remote main remains unchanged. Main migration,
full-platform acceptance and any publication remain blocked on actual validation.
