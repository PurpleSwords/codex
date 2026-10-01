#!/usr/bin/env python3
"""Publish the original validated native archives after npm promotion succeeds."""

import hashlib
import json
import os
import re
import subprocess
import zipfile

from fork_release_validation import BUILDER


REPOSITORY = "PurpleSwords/codex"


def github(path, *, missing_ok=False):
    try:
        output = subprocess.check_output(
            ["gh", "api", f"repos/{REPOSITORY}/{path}"],
            text=True,
            stderr=subprocess.PIPE,
        )
    except subprocess.CalledProcessError as error:
        if missing_ok and "(HTTP 404)" in (error.stderr or ""):
            return None
        raise
    return json.loads(output)


def sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return "sha256:" + digest.hexdigest()


def pages(path):
    for page in range(1, 11):
        result = github(f"{path}?per_page=100&page={page}")
        items = result["artifacts"] if isinstance(result, dict) else result
        yield from items
        if len(items) < 100:
            return
    raise ValueError("GitHub pagination exceeded 1,000 records")


def prepare_assets(candidate, directory):
    targets = {p["target_triple"] for p in BUILDER.CODEX_PLATFORM_PACKAGES.values()}
    artifacts = [
        item
        for item in pages(f"actions/runs/{candidate['validationRun']}/artifacts")
        if item["name"] in targets
    ]
    if len(artifacts) != len(targets) or {a["name"] for a in artifacts} != targets:
        raise ValueError("Expected exactly one native artifact for every platform")
    # Validate every identity before downloading or publishing anything.
    for artifact in artifacts:
        if (
            artifact["expired"]
            or artifact["workflow_run"]["id"] != candidate["validationRun"]
            or artifact["workflow_run"]["head_sha"] != candidate["sourceSha"]
            or not re.fullmatch(r"sha256:[0-9a-f]{64}", artifact["digest"] or "")
        ):
            raise ValueError("Native artifact source, expiry or digest mismatch")
    directory.mkdir()
    checksums = {}
    for artifact in artifacts:
        archive_path = directory / "download.zip"
        with archive_path.open("wb") as destination:
            subprocess.run(
                [
                    "gh",
                    "api",
                    f"repos/{REPOSITORY}/actions/artifacts/{artifact['id']}/zip",
                ],
                stdout=destination,
                check=True,
                timeout=600,
            )
        if sha256(archive_path) != artifact["digest"]:
            raise ValueError("Downloaded native artifact digest mismatch")
        name = f"codex-package-{artifact['name']}.tar.gz"
        with zipfile.ZipFile(archive_path) as archive:
            entries = archive.infolist()
            if (
                len(entries) != 1
                or entries[0].filename != name
                or entries[0].file_size > 500_000_000
            ):
                raise ValueError("Expected a single flat native package archive")
            archive.extractall(directory)
        archive_path.unlink()
        checksums[name] = sha256(directory / name).removeprefix("sha256:")
        print(f"Verified native archive: {name}", flush=True)
    (directory / "SHA256SUMS").write_text(
        "".join(f"{checksums[name]}  {name}\n" for name in sorted(checksums)),
        encoding="utf-8",
    )


def find_release(tag):
    # Drafts may have no Git ref yet and are not returned by releases/tags/{tag}.
    return next(
        (release for release in pages("releases") if release["tag_name"] == tag), None
    )


def validate_release(release, expected):
    assets = {asset["name"]: asset for asset in release["assets"]}
    if len(assets) != len(release["assets"]) or not set(assets).issubset(expected):
        raise ValueError("Unexpected or duplicate release assets")
    for name, asset in assets.items():
        if (
            release["draft"]
            and asset["state"] == "starter"
            and asset.get("size") == 0
            and asset.get("digest") is None
        ):
            continue
        if asset["state"] != "uploaded" or asset["digest"] != expected[name]:
            raise ValueError(f"Existing release asset differs: {name}")
    if not release["draft"] and set(assets) != set(expected):
        raise ValueError("Published release is incomplete; refuse to modify it")
    return {
        name: asset for name, asset in assets.items() if asset["state"] == "uploaded"
    }


def tag_source(tag):
    ref = github(f"git/ref/tags/{tag}", missing_ok=True)
    obj = ref["object"] if ref else None
    for _ in range(5):
        if obj is None or obj["type"] != "tag":
            break
        obj = github(f"git/tags/{obj['sha']}")["object"]
    if obj is not None and obj["type"] != "commit":
        raise ValueError("Release tag does not resolve to a commit")
    return obj["sha"] if obj else None


def publish_release(candidate, directory, *, execute):
    tag = f"fork-v{candidate['version']}"
    source = candidate["sourceSha"]
    ref = tag_source(tag)
    if ref is not None and ref != source:
        raise ValueError("Existing release tag points to a different source")
    expected = {path.name: sha256(path) for path in directory.iterdir()}
    required = {"SHA256SUMS"} | {
        f"codex-package-{platform['target_triple']}.tar.gz"
        for platform in BUILDER.CODEX_PLATFORM_PACKAGES.values()
    }
    if set(expected) != required:
        raise ValueError("Expected six native archives and SHA256SUMS")
    release = find_release(tag)
    if release:
        if release["prerelease"] or (
            ref is None and release["target_commitish"] != source
        ):
            raise ValueError("Existing release identity differs from candidate")
        present = validate_release(release, expected)
        if not release["draft"]:
            if ref is None:
                raise ValueError("Published release has no source tag")
            print(
                f"Already published with matching source and assets: {tag}", flush=True
            )
            return
    else:
        present = {}
    latest = github("releases/latest", missing_ok=True)
    if latest and latest["tag_name"] not in (
        f"fork-v{candidate['previousLatest']}",
        tag,
    ):
        raise ValueError("GitHub latest changed since candidate review")
    if not execute:
        print(f"GitHub Release preflight passed: {tag}", flush=True)
        return
    if release is None:
        notes = directory.parent / "release-notes.md"
        run_url = f"https://github.com/{REPOSITORY}/actions/runs/"
        notes.write_text(
            f"Community fork release `{candidate['version']}`.\n\n"
            f"- npm: `@purplesword/codex@{candidate['version']}`\n"
            f"- Binary source and tag: `{source}`\n"
            f"- [Source CI]({run_url}{candidate['blockingRun']})\n"
            f"- [Six-platform build and installation validation]({run_url}{candidate['validationRun']})\n"
            f"- [npm and GitHub publication]({run_url}{os.environ['GITHUB_RUN_ID']})\n\n"
            "These are the original validated Linux musl, macOS and Windows MSVC "
            "x64/ARM64 archives. Verify downloads with `SHA256SUMS`.\n\n"
            "Extract the complete archive, preserving `bin/`, `codex-resources/` "
            "and `codex-path/`. Start `bin/codex` (`bin/codex.exe` on Windows).\n",
            encoding="utf-8",
        )
        subprocess.run(
            [
                "gh",
                "release",
                "create",
                tag,
                "--repo",
                REPOSITORY,
                "--target",
                source,
                "--title",
                f"Community Codex {candidate['version']}",
                "--notes-file",
                str(notes),
                "--draft",
            ],
            check=True,
            timeout=120,
        )
    elif release["draft"]:
        # GitHub documents failed uploads leaving empty starter assets.
        for asset in release["assets"]:
            if asset["name"] not in present:
                subprocess.run(
                    [
                        "gh",
                        "api",
                        f"repos/{REPOSITORY}/releases/assets/{asset['id']}",
                        "--method",
                        "DELETE",
                    ],
                    check=True,
                    timeout=120,
                )
    # Sequential uploads avoid the inactivity timeouts seen with concurrent uploads.
    for name in sorted(expected):
        if name not in present:
            subprocess.run(
                [
                    "gh",
                    "release",
                    "upload",
                    tag,
                    str(directory / name),
                    "--repo",
                    REPOSITORY,
                ],
                check=True,
                timeout=600,
            )
    release = find_release(tag)
    if release is None or set(validate_release(release, expected)) != set(expected):
        raise ValueError("Release uploads are incomplete")
    ref = tag_source(tag)
    if (ref is not None and ref != source) or (
        ref is None and release["target_commitish"] != source
    ):
        raise ValueError("Release source changed during asset uploads")
    latest = github("releases/latest", missing_ok=True)
    if latest and latest["tag_name"] not in (
        f"fork-v{candidate['previousLatest']}",
        tag,
    ):
        raise ValueError("GitHub latest changed during asset uploads")
    subprocess.run(
        [
            "gh",
            "release",
            "edit",
            tag,
            "--repo",
            REPOSITORY,
            "--draft=false",
            "--latest",
        ],
        check=True,
        timeout=120,
    )
    if tag_source(tag) != source:
        raise ValueError("Published release tag does not match binary source")
    release = find_release(tag)
    if (
        release is None
        or release["draft"]
        or set(validate_release(release, expected)) != set(expected)
    ):
        raise ValueError("GitHub Release publication was not confirmed")
    print(f"Published GitHub Release: {release['html_url']}", flush=True)
