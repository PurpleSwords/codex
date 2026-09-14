"""Fork version policy and read-only release planning contracts."""

import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

from test_fork_npm import UPSTREAM_VERSION
from test_fork_npm import builder


class ForkVersionTests(unittest.TestCase):
    def test_revision_order_is_numeric_and_upstream_version_has_priority(self):
        for latest, current, expected in (
            ("0.153.4-fork.10", "0.153.4-fork.2", True),
            ("0.153.4-fork.2", "0.153.4-fork.10", False),
            ("0.153.4-fork.2", "0.153.4-fork.2", False),
            ("0.154.0-fork.1", "0.153.4-fork.99", True),
            ("0.153.4-fork.99", "0.154.0-fork.1", False),
        ):
            with self.subTest(latest=latest, current=current):
                self.assertEqual(
                    builder.is_newer_fork_release(latest, current), expected
                )

    def test_legacy_migration_is_limited_to_the_previously_published_fork(self):
        self.assertTrue(builder.is_newer_fork_release("0.153.4-fork.1", "0.153.4"))
        self.assertFalse(builder.is_newer_fork_release("0.153.3-fork.99", "0.153.4"))
        for current in ("0.153.3", "0.154.0", "garbage"):
            with self.subTest(current=current), self.assertRaises(ValueError):
                builder.is_newer_fork_release("0.153.4-fork.1", current)

    def test_root_versions_reject_ambiguous_and_platform_forms(self):
        for version in (
            "0.153.4",
            "0.153.4.1",
            "0.153.4+fork.1",
            "0.153.4-fork.0",
            "0.153.4-fork.01",
            "0.153.4-fork.-1",
            "00.153.4-fork.1",
            "0.153.4-fork.1-linux-x64",
            "0.153.4-fork.1.rc.1",
            "0.153.4-fork.1+metadata",
            "0.153.4-fork.18446744073709551616",
        ):
            with self.subTest(version=version), self.assertRaises(ValueError):
                builder.fork_version_key(version)

    def test_validation_reads_workspace_version_without_modifying_it(self):
        with tempfile.TemporaryDirectory() as directory:
            manifest = Path(directory) / "Cargo.toml"
            source = '[package]\nversion = "9.9.9"\n[workspace.package]\nversion = "0.153.4"\n[workspace.dependencies]\n'
            manifest.write_text(source, encoding="utf-8")
            self.assertEqual(
                builder.validate_fork_version("0.153.4-fork.3", manifest), "0.153.4"
            )
            for version in ("0.153.5-fork.1", "0.0.0-fork.1", "0.153.4"):
                with self.subTest(version=version), self.assertRaises(ValueError):
                    builder.validate_fork_version(version, manifest)
            self.assertEqual(manifest.read_text(encoding="utf-8"), source)

    def test_fork_staging_rejects_bare_or_wrong_baseline_versions(self):
        for version in (UPSTREAM_VERSION, "999.0.0-fork.1"):
            with (
                self.subTest(version=version),
                tempfile.TemporaryDirectory() as directory,
            ):
                staging = Path(directory)
                with self.assertRaises(ValueError):
                    builder.stage_sources(
                        staging, version, "codex", npm_name=builder.FORK_NPM_NAME
                    )
                self.assertEqual(list(staging.iterdir()), [])

    def test_release_plan_keeps_native_version_and_does_not_claim_publish_readiness(
        self,
    ):
        workspace = builder.REPO_ROOT / "codex-rs"
        version = f"{UPSTREAM_VERSION}-fork.2"
        before = [
            (workspace / name).read_bytes() for name in ("Cargo.toml", "Cargo.lock")
        ]
        result = subprocess.run(
            [
                sys.executable,
                str(builder.REPO_ROOT / "scripts" / "fork_release.py"),
                "--version",
                version,
                "--current-version",
                f"{UPSTREAM_VERSION}-fork.1",
            ],
            capture_output=True,
            text=True,
            timeout=10,
            check=True,
        )
        self.assertEqual(
            json.loads(result.stdout),
            {
                "npmPackage": "@purplesword/codex",
                "upstreamVersion": UPSTREAM_VERSION,
                "distributionVersion": version,
                "proposedTag": f"fork-v{version}",
                "upgradeFromCurrent": True,
                "platformVersions": {
                    tag: f"{version}-{tag}"
                    for tag in (
                        "linux-x64",
                        "linux-arm64",
                        "darwin-x64",
                        "darwin-arm64",
                        "win32-x64",
                        "win32-arm64",
                    )
                },
                "registryAvailabilityChecked": False,
                "publishAuthorized": False,
            },
        )
        self.assertEqual(
            [(workspace / name).read_bytes() for name in ("Cargo.toml", "Cargo.lock")],
            before,
        )


if __name__ == "__main__":
    unittest.main()
