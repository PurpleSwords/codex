#!/usr/bin/env python3
"""Compute release cache keys without falling back across toolchain boundaries."""

import hashlib
import json
import os
import subprocess


def cache_keys(environment, rustc, source_tree):
    # Runner image versions cover SDK/compiler changes not tracked in Cargo.lock.
    identity = {
        name: environment[name]
        for name in (
            "TARGET",
            "ImageOS",
            "ImageVersion",
            "RELEASE_CACHE_CONFIG",
        )
    }
    if not all(identity.values()):
        raise ValueError("Missing release cache identity; refuse an ambiguous key")
    identity["rustc"] = rustc
    identity["cargo"] = {
        name: value
        for name, value in environment.items()
        if name.startswith("CARGO_PROFILE_RELEASE_")
        or name
        in (
            "CARGO_INCREMENTAL",
            "CARGO_BUILD_JOBS",
            "RUSTFLAGS",
            "CARGO_ENCODED_RUSTFLAGS",
        )
    }
    digest = hashlib.sha256(json.dumps(identity, sort_keys=True).encode()).hexdigest()
    prefix = f"fork-release-cargo-v1-{identity['TARGET']}-{digest}-"
    # Same build inputs can be reused after workflow-only edits. Different
    # source trees may restore a compatible cache; Cargo still checks freshness.
    key = f"{prefix}{source_tree}-{environment['CODEX_FORK_VERSION']}"
    return key, prefix


if __name__ == "__main__":
    rustc = subprocess.check_output(["rustc", "-Vv"], text=True).strip()
    source_tree = subprocess.check_output(
        ["git", "rev-parse", "HEAD:codex-rs"], text=True
    ).strip()
    key, prefix = cache_keys(os.environ, rustc, source_tree)
    with open(os.environ["GITHUB_OUTPUT"], "a", encoding="utf-8") as output:
        print(f"key={key}\nprefix={prefix}", file=output)
