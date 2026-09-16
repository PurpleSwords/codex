#!/usr/bin/env python3
"""Promote checksum-pinned, previously validated npm tarballs; never rebuild them."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile
import time
from urllib.request import urlopen
import zipfile

from fork_release_smoke import load_packages
from fork_release_validation import BUILDER, ROOT, release_config


REPOSITORY = "PurpleSwords/codex"
REGISTRY = "https://registry.npmjs.org"


def github(path):
    return json.loads(
        subprocess.check_output(["gh", "api", f"repos/{REPOSITORY}/{path}"], text=True)
    )


def validate_run(run, candidate, workflow):
    if not (
        run["repository"]["full_name"] == REPOSITORY
        and run["head_repository"]["full_name"] == REPOSITORY
        and run["head_sha"] == candidate["sourceSha"]
        and run["head_branch"] == "main"
        and run["path"] == f".github/workflows/{workflow}.yml"
        and run["status"] == "completed"
        and run["conclusion"] == "success"
        and run["event"]
        == ("workflow_dispatch" if workflow == "fork-release" else "push")
    ):
        raise ValueError(
            "Candidate must come from successful main validation and CI runs"
        )


def validate_artifact(artifact, candidate):
    if not (
        artifact["id"] == candidate["artifactId"]
        and artifact["name"] == "npm-dist"
        and not artifact["expired"]
        and artifact["workflow_run"]["id"] == candidate["validationRun"]
        and artifact["workflow_run"]["head_sha"] == candidate["sourceSha"]
        and artifact["digest"] == "sha256:" + candidate["artifactSha256"]
    ):
        raise ValueError("Artifact identity, source, expiry or digest mismatch")


def unpack_verified(archive_path, directory, digest):
    with archive_path.open("rb") as source:
        if hashlib.file_digest(source, "sha256").hexdigest() != digest:
            raise ValueError("Downloaded artifact digest mismatch")
    with zipfile.ZipFile(archive_path) as archive:
        entries = archive.infolist()
        names = [entry.filename for entry in entries]
        if (
            len(entries) != 7
            or len(set(names)) != 7
            or sum(entry.file_size for entry in entries) > 2_000_000_000
            or any(
                "/" in name or "\\" in name or not name.endswith(".tgz")
                for name in names
            )
        ):
            raise ValueError("Expected seven flat npm tarballs in artifact")
        archive.extractall(directory)


def registry_metadata():
    with urlopen(f"{REGISTRY}/@purplesword%2fcodex", timeout=30) as response:
        return json.load(response)


def existing_matches(metadata, version, integrity):
    existing = metadata["versions"].get(version)
    if existing is None:
        return False
    if existing["dist"]["integrity"] != integrity or not existing["dist"].get(
        "attestations"
    ):
        raise ValueError(f"Published version differs or lacks provenance: {version}")
    return True


def publish_packages(directory, version, previous_latest, *, execute):
    packages, tarballs = load_packages(directory, version)
    paths = {}
    for path in tarballs.values():
        with tarfile.open(path, "r:gz") as archive:
            manifest = json.load(archive.extractfile("package/package.json"))
        paths[manifest["version"]] = path
    metadata = registry_metadata()
    if metadata["dist-tags"].get("latest") not in (previous_latest, version):
        raise ValueError(
            "Registry latest changed since candidate review; refuse to overwrite it"
        )
    # Check all versions before writing. Resume only byte-identical, attested packages.
    present = {
        item: existing_matches(metadata, item, package["dist"]["integrity"])
        for item, package in packages.items()
    }
    tags = [
        (f"{version}-{p['npm_tag']}", p["npm_tag"])
        for p in BUILDER.CODEX_PLATFORM_PACKAGES.values()
    ]
    tags.append((version, "latest"))
    for item, tag in tags:
        if present[item] and metadata["dist-tags"].get(tag) != item:
            raise ValueError(f"Existing version has a different dist-tag: {item}")
    for item, tag in tags:
        if present[item]:
            print(
                f"Already published with matching integrity and provenance: {item}",
                flush=True,
            )
            continue
        command = [
            "npm",
            "publish",
            str(paths[item]),
            "--ignore-scripts",
            "--access",
            "public",
            "--tag",
            tag,
            "--provenance",
            "--registry",
            REGISTRY,
        ]
        if not execute:
            command.append("--dry-run")
        subprocess.run(command, check=True, timeout=300)
        if execute:
            for attempt in range(6):
                observed = registry_metadata()
                if existing_matches(
                    observed, item, packages[item]["dist"]["integrity"]
                ):
                    if observed["dist-tags"].get(tag) != item:
                        raise ValueError(f"Published dist-tag mismatch: {item}")
                    break
                if attempt == 5:
                    raise ValueError(
                        f"Published version not visible yet; inspect before retrying: {item}"
                    )
                time.sleep(5)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--publish", action="store_true")
    args = parser.parse_args()
    if (
        os.environ.get("GITHUB_REPOSITORY") != REPOSITORY
        or os.environ.get("GITHUB_EVENT_NAME") != "workflow_dispatch"
    ):
        raise ValueError("Only manual runs in the fork may promote artifacts")
    if args.publish and os.environ.get("GITHUB_REF") != "refs/heads/main":
        raise ValueError("Real publication is restricted to main")
    candidate = json.loads((ROOT / "fork-release-candidate.json").read_text())
    release_config(candidate["version"])
    if os.environ.get("RELEASE_VERSION") != candidate["version"]:
        raise ValueError("Requested version differs from reviewed candidate")
    for key, workflow in [
        ("validationRun", "fork-release"),
        ("blockingRun", "blocking-ci"),
    ]:
        validate_run(github(f"actions/runs/{candidate[key]}"), candidate, workflow)
    jobs = github(f"actions/runs/{candidate['validationRun']}/jobs?per_page=100")[
        "jobs"
    ]
    successful = {job["name"] for job in jobs if job["conclusion"] == "success"}
    required = {"prepare", "package"}
    for platform in BUILDER.CODEX_PLATFORM_PACKAGES.values():
        required.update(
            {
                f"Release build {platform['npm_tag']}",
                f"Install and launch {platform['npm_tag']}",
            }
        )
    if not required.issubset(successful):
        raise ValueError("All six builds and installation checks must have passed")
    artifact = github(f"actions/artifacts/{candidate['artifactId']}")
    validate_artifact(artifact, candidate)
    print(json.dumps(candidate, indent=2), flush=True)
    with tempfile.TemporaryDirectory(prefix="fork-publish-") as temporary:
        root = Path(temporary)
        archive = root / "npm-dist.zip"
        with archive.open("wb") as destination:
            subprocess.run(
                [
                    "gh",
                    "api",
                    f"repos/{REPOSITORY}/actions/artifacts/{candidate['artifactId']}/zip",
                ],
                stdout=destination,
                check=True,
                timeout=300,
            )
        directory = root / "packages"
        unpack_verified(archive, directory, candidate["artifactSha256"])
        publish_packages(
            directory,
            candidate["version"],
            candidate["previousLatest"],
            execute=args.publish,
        )


if __name__ == "__main__":
    main()
