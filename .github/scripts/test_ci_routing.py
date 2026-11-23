import os
import subprocess
import unittest
from unittest.mock import patch

from check_ci_results import failed_dependencies
from ci_changes import changed_paths
from ci_changes import classify


class RoutingTests(unittest.TestCase):
    def test_docs_skip_compilation(self):
        plan = classify(["README.md", "docs/config.md"])
        self.assertEqual(plan["cargo_profile"], "none")
        self.assertFalse(any(plan[key] for key in ("bazel", "sdk", "cargo_deny")))

    def test_catalog_and_snapshot_take_targeted_path(self):
        plan = classify(
            [
                "codex-rs/models-manager/models.json",
                "codex-rs/tui/src/chatwidget/snapshots/codex_tui__chatwidget__tests__model_selection_popup.snap",
                "codex-rs/tui/src/chatwidget/snapshots/codex_tui__chatwidget__tests__model_selection_popup_bundled_models.snap",
            ]
        )
        self.assertEqual(plan["cargo_profile"], "model-catalog-extensions")
        self.assertFalse(
            any(plan[key] for key in ("bazel", "sdk", "cargo_deny", "codex"))
        )

    def test_code_alongside_catalog_requires_baseline(self):
        plan = classify(
            ["codex-rs/models-manager/models.json", "codex-rs/core/src/codex.rs"]
        )
        self.assertEqual(plan["cargo_profile"], "baseline")
        self.assertTrue(plan["sdk"])

    def test_build_and_dependency_changes(self):
        for path in ("codex-rs/Cargo.lock", "codex-rs/http-client/Cargo.toml"):
            with self.subTest(path=path):
                plan = classify([path])
                self.assertTrue(plan["bazel"])
                self.assertTrue(plan["cargo_deny"])
                self.assertTrue(plan["sdk"])
        self.assertTrue(classify(["codex-rs/tui/BUILD.bazel"])["bazel"])

    def test_sdk_gets_cli_artifacts(self):
        plan = classify(["sdk/typescript/src/thread.ts"])
        self.assertTrue(plan["sdk"])
        self.assertEqual(plan["cargo_profile"], "baseline")

    def test_unknown_inputs_and_ci_changes_run_baseline(self):
        for path in (
            "new-build-system",
            ".github/workflows/blocking-ci.yml",
            "codex-rs/core/prompt.md",
        ):
            self.assertEqual(classify([path])["cargo_profile"], "baseline")
        self.assertTrue(
            classify([".github/workflows/rust-ci.yml"])["argument_comment_lint_package"]
        )

    @patch(
        "ci_changes.subprocess.check_output", return_value=b"README.md\0deleted.rs\0"
    )
    def test_push_uses_actual_diff_and_keeps_deleted_paths(self, command):
        self.assertEqual(
            changed_paths("push", {"before": "old", "after": "new"}),
            ["README.md", "deleted.rs"],
        )
        self.assertEqual(command.call_args.args[0][-3:], ["old", "new", "--"])

    @patch.dict(os.environ, {"GITHUB_SHA": "merge"})
    @patch("ci_changes.subprocess.check_output", return_value=b"README.md\0")
    def test_pr_inspects_tested_merge_tree(self, command):
        changed_paths(
            "pull_request",
            {"pull_request": {"base": {"sha": "base"}, "head": {"sha": "head"}}},
        )
        self.assertEqual(command.call_args.args[0][-3:], ["base", "merge", "--"])

    @patch(
        "ci_changes.subprocess.check_output",
        side_effect=subprocess.CalledProcessError(128, "git"),
    )
    def test_missing_history_falls_back_to_full_checks(self, command):
        paths = changed_paths("push", {"before": "old", "after": "new"})
        self.assertTrue(classify(paths)["argument_comment_lint_package"])
        self.assertEqual(
            classify(changed_paths("push", {"before": "0" * 40}))["cargo_profile"],
            "baseline",
        )
        self.assertEqual(
            classify(changed_paths("workflow_dispatch", {}))["cargo_profile"],
            "baseline",
        )


class ResultPolicyTests(unittest.TestCase):
    def test_only_explicitly_planned_skips_pass(self):
        needs = {"changes": {"result": "success"}, "bazel": {"result": "skipped"}}
        self.assertEqual(failed_dependencies(needs, {"bazel": True}), [])
        for allowed in ({}, {"bazel": False}, {"bazel": "true"}):
            self.assertEqual(
                failed_dependencies(needs, allowed), [("bazel", "skipped")]
            )

    def test_failures_cancellation_and_detection_failure_never_pass(self):
        for result in ("failure", "cancelled"):
            self.assertEqual(
                failed_dependencies({"bazel": {"result": result}}, {"bazel": True}),
                [("bazel", result)],
            )
        self.assertEqual(
            failed_dependencies({"changes": {"result": "failure"}}, {}),
            [("changes", "failure")],
        )

    def test_unknown_dependency_is_rejected(self):
        self.assertEqual(
            failed_dependencies({}, {"bazel": True}), [("bazel", "missing dependency")]
        )


if __name__ == "__main__":
    unittest.main()
