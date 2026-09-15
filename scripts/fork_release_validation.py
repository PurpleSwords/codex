#!/usr/bin/env python3
"""Validate a fork release without publishing or changing source versions."""

import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile
from urllib.request import urlopen

import stage_npm_packages as staging


ROOT = Path(__file__).resolve().parent.parent
BUILDER = staging._BUILD_MODULE


def release_config(version):
    BUILDER.validate_fork_version(version, ROOT / "codex-rs/Cargo.toml")
    config = json.loads((ROOT / "fork-release.json").read_text(encoding="utf-8"))
    if config["npmPackage"] != "@purplesword/codex":
        raise ValueError("Unexpected fork package identity")
    repository = os.environ.get("GITHUB_REPOSITORY")
    if repository and repository != config["githubRepository"]:
        raise ValueError("Workflow repository does not match fork-release.json")
    return config


def expected_versions(version):
    return [
        version,
        *(
            f"{version}-{p['npm_tag']}"
            for p in BUILDER.CODEX_PLATFORM_PACKAGES.values()
        ),
    ]


def check_available(version, metadata):
    versions = metadata["versions"]
    if not isinstance(versions, dict):
        raise ValueError("Invalid registry versions metadata")
    existing = set(expected_versions(version)).intersection(versions)
    if existing:
        raise ValueError(
            f"Already published; choose a new revision: {sorted(existing)}"
        )


def plan(version):
    config = release_config(version)
    # Fail closed: authentication, network errors and even a missing package
    # require investigation, not an assumption that a version is available.
    with urlopen(
        "https://registry.npmjs.org/@purplesword%2fcodex", timeout=30
    ) as response:
        metadata = json.load(response)
    check_available(version, metadata)
    print(
        json.dumps(
            {
                "version": version,
                "upstreamVersion": BUILDER.read_workspace_version(
                    ROOT / "codex-rs/Cargo.toml"
                ),
                "npmPackage": config["npmPackage"],
                "sourceSha": subprocess.check_output(
                    ["git", "rev-parse", "HEAD"], text=True
                ).strip(),
                "versions": expected_versions(version),
                "registryAvailabilityChecked": True,
                "publishAuthorized": False,
            },
            indent=2,
        )
    )


def package(version, artifacts, output):
    config = release_config(version)
    output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="fork-npm-") as temporary:
        root = Path(temporary)
        vendor = root / "vendor"
        staging.install_codex_package_archives(
            artifacts, vendor, staging.BINARY_TARGETS
        )
        for package_name in ["codex", *BUILDER.CODEX_PLATFORM_PACKAGES]:
            stage = root / package_name
            stage.mkdir()
            BUILDER.stage_sources(
                stage,
                version,
                package_name,
                npm_name=config["npmPackage"],
                repository_url=config["repositoryUrl"],
                description=config["description"],
                readme=ROOT / config["readme"],
            )
            if package_name != "codex":
                BUILDER.copy_native_binaries(
                    vendor,
                    stage,
                    [BUILDER.CODEX_PACKAGE_COMPONENT],
                    target_filter={
                        BUILDER.CODEX_PLATFORM_PACKAGES[package_name]["target_triple"]
                    },
                )
            tarball = output / staging.tarball_name_for_package(package_name, version)
            BUILDER.run_npm_pack(stage, tarball)
            # These are the only publish commands in the validation path.
            # --dry-run is unconditional, never controlled by user input.
            tag = (
                "latest"
                if package_name == "codex"
                else BUILDER.CODEX_PLATFORM_PACKAGES[package_name]["npm_tag"]
            )
            subprocess.run(
                [
                    "npm",
                    "publish",
                    str(tarball.resolve()),
                    "--dry-run",
                    "--ignore-scripts",
                    "--access",
                    "public",
                    "--tag",
                    tag,
                    "--provenance",
                ],
                check=True,
            )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=["plan", "package"])
    parser.add_argument("--version", required=True)
    parser.add_argument("--artifacts-dir", type=Path)
    parser.add_argument("--output-dir", type=Path)
    args = parser.parse_args()
    if args.mode == "plan":
        plan(args.version)
    else:
        if args.artifacts_dir is None or args.output_dir is None:
            parser.error("package requires --artifacts-dir and --output-dir")
        package(args.version, args.artifacts_dir.resolve(), args.output_dir.resolve())


if __name__ == "__main__":
    main()
