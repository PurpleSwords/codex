"""Release promotion rejects untrusted inputs and preserves publish ordering."""

import copy
import hashlib
import io
import json
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch
import zipfile

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "scripts"))
import fork_release_publish as publish
from fork_release_validation import expected_versions


class PromotionTests(unittest.TestCase):
    def test_source_run_must_be_successful_main_in_expected_repository(self):
        candidate = {"sourceSha": "source"}
        run = {
            "repository": {"full_name": publish.REPOSITORY},
            "head_repository": {"full_name": publish.REPOSITORY},
            "head_sha": "source",
            "head_branch": "main",
            "status": "completed",
            "conclusion": "success",
            "event": "workflow_dispatch",
            "path": ".github/workflows/fork-release.yml",
        }
        publish.validate_run(run, candidate, "fork-release")
        for field, value in [
            ("head_sha", "other"),
            ("head_branch", "feature"),
            ("conclusion", "failure"),
            ("status", "in_progress"),
            ("event", "pull_request"),
            ("path", "other.yml"),
            ("head_repository", {"full_name": "other/codex"}),
        ]:
            with self.subTest(field=field), self.assertRaises(ValueError):
                publish.validate_run(
                    dict(run, **{field: value}), candidate, "fork-release"
                )

    def test_artifact_requires_matching_run_digest_and_unexpired_identity(self):
        candidate = {
            "artifactId": 12,
            "validationRun": 34,
            "sourceSha": "source",
            "artifactSha256": "digest",
        }
        artifact = {
            "id": 12,
            "name": "npm-dist",
            "expired": False,
            "workflow_run": {"id": 34, "head_sha": "source"},
            "digest": "sha256:digest",
        }
        publish.validate_artifact(artifact, candidate)
        for field, value in [
            ("id", 13),
            ("expired", True),
            ("digest", "sha256:other"),
            ("name", "other"),
            ("workflow_run", {"id": 35, "head_sha": "source"}),
        ]:
            with self.subTest(field=field), self.assertRaises(ValueError):
                publish.validate_artifact(dict(artifact, **{field: value}), candidate)

    def test_archive_digest_and_flat_member_names_are_enforced(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            archive = root / "artifact.zip"
            for bad_name in [None, "../escape.tgz", "nested/file.tgz"]:
                with zipfile.ZipFile(archive, "w") as output:
                    for index in range(7):
                        output.writestr(
                            bad_name if index == 0 and bad_name else f"{index}.tgz",
                            b"data",
                        )
                digest = hashlib.sha256(archive.read_bytes()).hexdigest()
                if bad_name:
                    with self.assertRaises(ValueError):
                        publish.unpack_verified(archive, root / "packages", digest)
                else:
                    publish.unpack_verified(archive, root / "packages", digest)
                    with self.assertRaises(ValueError):
                        publish.unpack_verified(archive, root / "packages", "wrong")

    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name)
        self.version = "0.153.4-fork.1"
        for version in expected_versions(self.version):
            data = json.dumps(
                {"name": "@purplesword/codex", "version": version}
            ).encode()
            with tarfile.open(self.directory / f"{version}.tgz", "w:gz") as archive:
                info = tarfile.TarInfo("package/package.json")
                info.size = len(data)
                archive.addfile(info, io.BytesIO(data))
        self.metadata = {"versions": {}, "dist-tags": {"latest": "0.153.4"}}

    def test_default_dry_run_checks_all_packages_and_root_is_last(self):
        with (
            patch.object(publish, "registry_metadata", return_value=self.metadata),
            patch.object(publish.subprocess, "run") as run,
        ):
            publish.publish_packages(
                self.directory, self.version, "0.153.4", execute=False
            )
        self.assertEqual(run.call_count, 7)
        for call in run.call_args_list:
            self.assertIn("--dry-run", call.args[0])
            self.assertIn("--provenance", call.args[0])
            self.assertIn("--ignore-scripts", call.args[0])
        self.assertEqual(
            run.call_args_list[-1].args[0][2],
            str(self.directory / f"{self.version}.tgz"),
        )

    def test_conflicting_version_or_changed_latest_prevents_all_writes(self):
        conflict = copy.deepcopy(self.metadata)
        conflict["versions"][self.version] = {"dist": {"integrity": "different"}}
        moved = copy.deepcopy(self.metadata)
        moved["dist-tags"]["latest"] = "0.153.4-fork.2"
        for metadata in [conflict, moved]:
            with (
                patch.object(publish, "registry_metadata", return_value=metadata),
                patch.object(publish.subprocess, "run") as run,
            ):
                with self.assertRaises(ValueError):
                    publish.publish_packages(
                        self.directory, self.version, "0.153.4", execute=True
                    )
                run.assert_not_called()

    def test_platform_publish_failure_never_publishes_root(self):
        with (
            patch.object(publish, "registry_metadata", return_value=self.metadata),
            patch.object(
                publish.subprocess,
                "run",
                side_effect=subprocess.CalledProcessError(1, "npm"),
            ) as run,
        ):
            with self.assertRaises(subprocess.CalledProcessError):
                publish.publish_packages(
                    self.directory, self.version, "0.153.4", execute=True
                )
            self.assertEqual(run.call_count, 1)
            self.assertNotIn("latest", run.call_args.args[0])

    def test_matching_attested_packages_can_resume_without_republishing(self):
        packages, _ = publish.load_packages(self.directory, self.version)
        metadata = copy.deepcopy(self.metadata)
        for version, package in packages.items():
            metadata["versions"][version] = {
                "dist": {
                    **package["dist"],
                    "attestations": {"url": "https://registry.npmjs.org/attestations"},
                }
            }
            tag = (
                "latest"
                if version == self.version
                else version.removeprefix(self.version + "-")
            )
            metadata["dist-tags"][tag] = version
        with (
            patch.object(publish, "registry_metadata", return_value=metadata),
            patch.object(publish.subprocess, "run") as run,
        ):
            publish.publish_packages(
                self.directory, self.version, "0.153.4", execute=True
            )
            run.assert_not_called()


if __name__ == "__main__":
    unittest.main()
