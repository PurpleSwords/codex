"""Release safety contracts; fixtures are not publishable native binaries."""

import io
import json
import platform
from pathlib import Path
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch
from urllib.error import URLError

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "scripts"))
import fork_release_validation as validation
import fork_release_smoke as smoke


class ReleaseValidationTests(unittest.TestCase):
    @unittest.skipUnless(
        sys.platform == "linux" and platform.machine() == "x86_64",
        "Linux fixture launcher",
    )
    def test_loopback_registry_installs_alias_and_launches_fixture(self):
        upstream = validation.BUILDER.read_workspace_version(
            validation.ROOT / "codex-rs/Cargo.toml"
        )
        version = f"{upstream}-fork.1"
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            tarballs = root / "tarballs"
            tarballs.mkdir()
            for package in ["codex", *validation.BUILDER.CODEX_PLATFORM_PACKAGES]:
                stage = root / package
                stage.mkdir()
                validation.BUILDER.stage_sources(
                    stage,
                    version,
                    package,
                    npm_name="@purplesword/codex",
                    repository_url="git+https://github.com/PurpleSwords/codex.git",
                )
                if package == "codex-linux-x64":
                    binaries = stage / "vendor/x86_64-unknown-linux-musl/bin"
                    binaries.mkdir(parents=True)
                    for binary in ["codex", "codex-code-mode-host"]:
                        executable = binaries / binary
                        executable.write_text(
                            f"#!/usr/bin/env node\nconsole.log('codex-cli {upstream}');\n",
                            encoding="utf-8",
                        )
                        executable.chmod(0o755)
                with tarfile.open(tarballs / f"{package}.tgz", "w:gz") as archive:
                    archive.add(stage, arcname="package")
            smoke.smoke(tarballs, version, "linux-x64")

    def test_any_existing_platform_or_root_blocks_candidate(self):
        version = "0.153.4-fork.1"
        validation.check_available(version, {"versions": {"0.153.4": {}}})
        for existing in validation.expected_versions(version):
            with self.subTest(existing=existing), self.assertRaises(ValueError):
                validation.check_available(version, {"versions": {existing: {}}})

    def test_network_failure_is_not_treated_as_version_availability(self):
        with (
            patch.object(validation, "release_config", return_value={}),
            patch.object(validation, "urlopen", side_effect=URLError("offline")),
            self.assertRaises(URLError),
        ):
            validation.plan("0.153.4-fork.1")

    def test_packaging_only_ever_dry_runs_publish(self):
        version = "0.153.4-fork.1"
        config = {
            "npmPackage": "@purplesword/codex",
            "repositoryUrl": "git+https://github.com/PurpleSwords/codex.git",
            "description": "fork",
            "readme": "codex-cli/README.fork.md",
        }
        with (
            tempfile.TemporaryDirectory() as temporary,
            patch.object(validation, "release_config", return_value=config),
            patch.object(validation.staging, "install_codex_package_archives"),
            patch.object(validation.BUILDER, "stage_sources"),
            patch.object(validation.BUILDER, "copy_native_binaries"),
            patch.object(validation.BUILDER, "run_npm_pack"),
            patch.object(validation.subprocess, "run") as run,
        ):
            validation.package(version, Path(temporary), Path(temporary) / "npm")
        commands = [call.args[0] for call in run.call_args_list]
        self.assertEqual(len(commands), 7)
        for command in commands:
            self.assertEqual(command[:2], ["npm", "publish"])
            self.assertIn("--dry-run", command)
            self.assertIn("--provenance", command)
            self.assertIn("--ignore-scripts", command)

    def test_registry_rejects_incomplete_or_foreign_artifacts(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            with self.assertRaises(ValueError):
                smoke.load_packages(directory, "0.153.4-fork.1")
            manifest = json.dumps(
                {"name": "@openai/codex", "version": "0.153.4"}
            ).encode()
            with tarfile.open(directory / "foreign.tgz", "w:gz") as archive:
                member = tarfile.TarInfo("package/package.json")
                member.size = len(manifest)
                archive.addfile(member, io.BytesIO(manifest))
            with self.assertRaises(ValueError):
                smoke.load_packages(directory, "0.153.4-fork.1")
