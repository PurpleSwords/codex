#!/usr/bin/env python3
"""Select checks from the candidate's actual diff; unknown inputs run the baseline."""

import json
import os
from pathlib import Path
import subprocess


def classify(paths):
    catalog = False
    baseline = False
    bazel = False
    dependencies = False
    sdk = False
    lint_package = False
    workflows = False
    for path in paths:
        if path.endswith(".md") and ("/" not in path or path.startswith("docs/")):
            continue
        if path == "codex-rs/models-manager/models.json" or (
            path.startswith("codex-rs/tui/src/chatwidget/snapshots/")
            and path.endswith("__model_selection_popup.snap")
        ):
            catalog = True
            continue
        manifest = Path(path).name in {"Cargo.toml", "Cargo.lock"}
        build = (
            manifest
            or path.endswith((".bzl", ".bazel", ".bazel.lock"))
            or Path(path).name in {"BUILD", "WORKSPACE", ".bazelrc", ".bazelversion"}
            or path.startswith(("tools/", "third_party/", "patches/"))
            or path
            in {
                "workspace_root_test_launcher.sh.tpl",
                "workspace_root_test_launcher.bat.tpl",
            }
        )
        workflow = path.startswith(".github/")
        workflows |= workflow
        baseline = True
        bazel |= build or workflow
        dependencies |= manifest or workflow or path.endswith("deny.toml")
        sdk |= (
            manifest
            or workflow
            or path.startswith(
                (
                    "sdk/",
                    "codex-cli/",
                    "codex-rs/core/",
                    "codex-rs/cli/",
                    "codex-rs/exec/",
                    "codex-rs/protocol/",
                    "codex-rs/app-server",
                )
            )
            or path
            in {"package.json", "pnpm-lock.yaml", "pnpm-workspace.yaml", "justfile"}
        )
        lint_package |= path.startswith("tools/argument-comment-lint/") or path in {
            ".github/workflows/rust-ci.yml",
            ".github/workflows/rust-ci-full.yml",
        }
    return {
        "cargo_profile": "baseline"
        if baseline
        else "model-catalog-extensions"
        if catalog
        else "none",
        "codex": baseline,
        "workflows": workflows,
        "argument_comment_lint": baseline,
        "argument_comment_lint_package": lint_package,
        "bazel": bazel,
        "cargo_deny": dependencies,
        "sdk": sdk,
    }


def changed_paths(event_name, event):
    if event_name == "pull_request":
        base = event["pull_request"]["base"]["sha"]
        head = os.environ["GITHUB_SHA"]
    elif event_name == "push" and event.get("before", "").strip("0"):
        base, head = event["before"], event["after"]
    else:
        return [".github/workflows/rust-ci.yml"]
    try:
        result = subprocess.check_output(
            [
                "git",
                "diff",
                "--name-only",
                "--no-renames",
                "-z",
                base,
                head,
                "--",
            ]
        )
    except subprocess.CalledProcessError:
        return [".github/workflows/rust-ci.yml"]
    return result.decode("utf-8").split("\0")[:-1]


def main():
    event = json.loads(Path(os.environ["GITHUB_EVENT_PATH"]).read_text())
    plan = classify(changed_paths(os.environ["GITHUB_EVENT_NAME"], event))
    with open(os.environ["GITHUB_OUTPUT"], "a") as output:
        for name, value in plan.items():
            print(f"{name}={str(value).lower()}", file=output)
    print(json.dumps(plan, indent=2))


if __name__ == "__main__":
    main()
