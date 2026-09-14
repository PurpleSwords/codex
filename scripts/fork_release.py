#!/usr/bin/env python3
"""Print a fork release plan; never change versions, publish, or create a tag."""

import argparse
import importlib.util
import json
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parent.parent
BUILD_SCRIPT = REPO_ROOT / "codex-cli" / "scripts" / "build_npm_package.py"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--version", required=True, help="Candidate <upstream>-fork.<revision>"
    )
    parser.add_argument(
        "--current-version", help="Optional installed fork version for upgrade planning"
    )
    args = parser.parse_args()
    spec = importlib.util.spec_from_file_location("fork_npm_builder", BUILD_SCRIPT)
    builder = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(builder)
    try:
        upstream = builder.validate_fork_version(
            args.version, REPO_ROOT / "codex-rs" / "Cargo.toml"
        )
        upgrade = (
            None
            if args.current_version is None
            else builder.is_newer_fork_release(args.version, args.current_version)
        )
    except ValueError as error:
        parser.error(str(error))
    config = json.loads((REPO_ROOT / "fork-release.json").read_text(encoding="utf-8"))
    print(
        json.dumps(
            {
                "npmPackage": config["npmPackage"],
                "upstreamVersion": upstream,
                "distributionVersion": args.version,
                "proposedTag": f"fork-v{args.version}",
                "upgradeFromCurrent": upgrade,
                "platformVersions": {
                    platform["npm_tag"]: builder.compute_platform_package_version(
                        args.version, platform["npm_tag"]
                    )
                    for platform in builder.CODEX_PLATFORM_PACKAGES.values()
                },
                "registryAvailabilityChecked": False,
                "publishAuthorized": False,
            },
            indent=2,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
