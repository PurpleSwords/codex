#!/usr/bin/env python3

"""Require success, except for explicitly planned dependency skips.

Parent workflows pass GitHub's `toJSON(needs)` object through the NEEDS
environment variable. ALLOWED_SKIPS maps job names to booleans from successful
change detection. Without that map the default policy remains success-only.
"""

import json
import os


def failed_dependencies(needs, allowed_skips):
    failures = []
    for name in allowed_skips:
        if name not in needs:
            failures.append((name, "missing dependency"))
    for name, dependency in needs.items():
        result = dependency["result"]
        if result != "success" and not (
            result == "skipped" and allowed_skips.get(name) is True
        ):
            failures.append((name, result))
    return sorted(failures)


def main() -> None:
    # Keep result policy in one script so blocking-ci and postmerge-ci cannot
    # drift in how they interpret dependency conclusions.
    needs = json.loads(os.environ["NEEDS"])
    failures = failed_dependencies(
        needs, json.loads(os.environ.get("ALLOWED_SKIPS", "{}"))
    )

    if failures:
        print("CI dependencies did not succeed:")
        for name, result in failures:
            print(f"{name}: {result}")
        raise SystemExit(1)

    print("All required CI dependencies succeeded; planned skips accepted.")


if __name__ == "__main__":
    main()
