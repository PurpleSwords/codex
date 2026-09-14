#!/usr/bin/env python3
"""Install real tarballs through an isolated loopback registry and run the CLI."""

import argparse
import base64
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile
import threading
from urllib.parse import unquote, urlsplit

from fork_release_validation import BUILDER, expected_versions, release_config


def load_packages(directory, version):
    packages = {}
    tarballs = {}
    for path in directory.glob("*.tgz"):
        with tarfile.open(path, "r:gz") as archive:
            manifest = json.load(archive.extractfile("package/package.json"))
        if manifest["name"] != "@purplesword/codex":
            raise ValueError(f"Unexpected package name in {path}")
        package_version = manifest["version"]
        if package_version in packages:
            raise ValueError(f"Duplicate package version: {package_version}")
        with path.open("rb") as source:
            digest = hashlib.file_digest(source, "sha512").digest()
        manifest["dist"] = {"integrity": "sha512-" + base64.b64encode(digest).decode()}
        packages[package_version] = manifest
        tarballs["/" + path.name] = path
    if set(packages) != set(expected_versions(version)):
        raise ValueError("Expected exactly the root and six platform packages")
    return packages, tarballs


def smoke(directory, version, platform):
    release_config(version)
    packages, tarballs = load_packages(directory, version)

    class Registry(BaseHTTPRequestHandler):
        def do_GET(self):
            path = unquote(urlsplit(self.path).path)
            if path == "/@purplesword/codex":
                data = json.dumps(
                    {
                        "name": "@purplesword/codex",
                        "dist-tags": {"latest": version},
                        "versions": packages,
                    }
                ).encode()
                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(len(data)))
                self.end_headers()
                self.wfile.write(data)
            elif path in tarballs:
                source = tarballs[path]
                self.send_response(200)
                self.send_header("Content-Length", str(source.stat().st_size))
                self.end_headers()
                with source.open("rb") as stream:
                    shutil.copyfileobj(stream, self.wfile)
            else:
                self.send_error(404)

    server = ThreadingHTTPServer(("127.0.0.1", 0), Registry)
    registry = f"http://127.0.0.1:{server.server_port}"
    for path in tarballs.values():
        with tarfile.open(path, "r:gz") as archive:
            manifest = json.load(archive.extractfile("package/package.json"))
        packages[manifest["version"]]["dist"]["tarball"] = f"{registry}/{path.name}"
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        with tempfile.TemporaryDirectory(prefix="fork-install-") as temporary:
            prefix = Path(temporary)
            (prefix / "package.json").write_text('{"private":true}', encoding="utf-8")
            (prefix / "npmrc").write_text("", encoding="utf-8")
            env = dict(
                os.environ,
                CODEX_HOME=str(prefix / "codex-home"),
                npm_config_cache=str(prefix / "npm-cache"),
                npm_config_userconfig=str(prefix / "npmrc"),
            )
            subprocess.run(
                [
                    shutil.which("npm"),
                    "install",
                    f"@purplesword/codex@{version}",
                    "--registry",
                    registry,
                    "--include=optional",
                    "--ignore-scripts",
                    "--no-audit",
                    "--no-fund",
                    "--fetch-retries=0",
                    "--fetch-timeout=30000",
                ],
                cwd=prefix,
                env=env,
                check=True,
                timeout=180,
            )
            root = prefix / "node_modules/@purplesword/codex"
            installed = json.loads((root / "package.json").read_text(encoding="utf-8"))
            if installed["version"] != version:
                raise ValueError("Installed the wrong root version")
            payload = prefix / f"node_modules/@purplesword/codex-{platform}"
            platform_info = BUILDER.CODEX_PLATFORM_PACKAGES[f"codex-{platform}"]
            binary_dir = payload / "vendor" / platform_info["target_triple"] / "bin"
            suffix = ".exe" if platform.startswith("win32") else ""
            for binary in ["codex", "codex-code-mode-host"]:
                if not (binary_dir / f"{binary}{suffix}").is_file():
                    raise ValueError(f"Missing native companion: {binary}")
            for flag in ["--version", "--help"]:
                result = subprocess.run(
                    [
                        shutil.which("node"),
                        str(root / "bin/codex.js"),
                        flag,
                    ],
                    env=env,
                    check=True,
                    capture_output=True,
                    text=True,
                    timeout=30,
                )
                print(result.stdout)
                if (
                    flag == "--version"
                    and installed["codexUpstreamVersion"] not in result.stdout
                ):
                    raise ValueError(
                        "Native upstream version differs from package metadata"
                    )
            print(f"PASS: installed and launched {version} on {platform}")
    finally:
        server.shutdown()
        thread.join()
        server.server_close()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--directory", required=True, type=Path)
    parser.add_argument("--version", required=True)
    parser.add_argument(
        "--platform",
        required=True,
        choices=[p["npm_tag"] for p in BUILDER.CODEX_PLATFORM_PACKAGES.values()],
    )
    args = parser.parse_args()
    smoke(args.directory.resolve(), args.version, args.platform)
