"""GitHub promotion binds artifacts to source and resumes only identical drafts."""

import copy
import hashlib
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch
import zipfile

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "scripts"))
import fork_github_release as release
import fork_release_publish as publish


class GitHubReleaseTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.assets = self.root / "assets"
        self.candidate = {
            "version": "0.153.4-fork.2",
            "previousLatest": "0.153.4-fork.1",
            "sourceSha": "source",
            "validationRun": 12,
            "blockingRun": 13,
            "artifactId": 14,
            "artifactSha256": "digest",
        }
        self.artifacts = []
        self.downloads = {}
        for index, platform in enumerate(
            release.BUILDER.CODEX_PLATFORM_PACKAGES.values()
        ):
            target = platform["target_triple"]
            buffer = io.BytesIO()
            with zipfile.ZipFile(buffer, "w") as archive:
                archive.writestr(f"codex-package-{target}.tar.gz", target.encode())
            data = buffer.getvalue()
            self.downloads[index] = data
            self.artifacts.append(
                {
                    "id": index,
                    "name": target,
                    "expired": False,
                    "workflow_run": {"id": 12, "head_sha": "source"},
                    "digest": "sha256:" + hashlib.sha256(data).hexdigest(),
                }
            )

    def prepare(self):
        def download(command, *, stdout, **kwargs):
            artifact_id = int(command[-1].split("/")[-2])
            stdout.write(self.downloads[artifact_id])

        with (
            patch.object(release, "pages", return_value=iter(self.artifacts)),
            patch.object(release.subprocess, "run", side_effect=download),
        ):
            release.prepare_assets(self.candidate, self.assets)

    def draft(self, names=()):
        return {
            "id": 20,
            "tag_name": "fork-v0.153.4-fork.2",
            "draft": True,
            "prerelease": False,
            "target_commitish": "source",
            "html_url": "release-url",
            "assets": [
                {
                    "name": name,
                    "state": "uploaded",
                    "digest": release.sha256(self.assets / name),
                }
                for name in names
            ],
        }

    def test_downloaded_native_archives_and_checksum_manifest_match(self):
        self.prepare()
        files = sorted(
            path for path in self.assets.iterdir() if path.name != "SHA256SUMS"
        )
        self.assertEqual(len(files), 6)
        self.assertEqual(
            (self.assets / "SHA256SUMS").read_text(),
            "".join(f"{release.sha256(path)[7:]}  {path.name}\n" for path in files),
        )

    def test_untrusted_or_incomplete_artifacts_prevent_all_downloads(self):
        for change in ("expired", "source", "digest", "missing", "duplicate"):
            artifacts = copy.deepcopy(self.artifacts)
            if change == "expired":
                artifacts[0]["expired"] = True
            elif change == "source":
                artifacts[0]["workflow_run"]["head_sha"] = "other"
            elif change == "digest":
                artifacts[0]["digest"] = None
            elif change == "missing":
                artifacts.pop()
            else:
                artifacts.append(artifacts[0])
            with (
                self.subTest(change=change),
                patch.object(release, "pages", return_value=iter(artifacts)),
                patch.object(release.subprocess, "run") as run,
                self.assertRaises(ValueError),
            ):
                release.prepare_assets(self.candidate, self.assets)
            run.assert_not_called()

    def test_tampered_download_or_nested_archive_is_rejected(self):
        for nested in (False, True):
            with (
                self.subTest(nested=nested),
                tempfile.TemporaryDirectory() as temporary,
            ):
                buffer = io.BytesIO()
                with zipfile.ZipFile(buffer, "w") as archive:
                    archive.writestr(
                        "../escape.tar.gz" if nested else "archive", b"bad"
                    )
                data = buffer.getvalue()
                artifacts = copy.deepcopy(self.artifacts)
                if nested:
                    artifacts[0]["digest"] = (
                        "sha256:" + hashlib.sha256(data).hexdigest()
                    )
                with (
                    patch.object(release, "pages", return_value=iter(artifacts)),
                    patch.object(
                        release.subprocess,
                        "run",
                        side_effect=lambda *args, stdout, **kw: stdout.write(data),
                    ),
                    self.assertRaises(ValueError),
                ):
                    release.prepare_assets(self.candidate, Path(temporary) / "assets")

    def test_missing_ref_is_distinct_from_authentication_failure(self):
        for status in (404, 403):
            error = subprocess.CalledProcessError(
                1, "gh", stderr=f"gh: error (HTTP {status})"
            )
            with (
                self.subTest(status=status),
                patch.object(release.subprocess, "check_output", side_effect=error),
            ):
                if status == 404:
                    self.assertIsNone(
                        release.github("git/ref/tags/new", missing_ok=True)
                    )
                else:
                    with self.assertRaises(subprocess.CalledProcessError):
                        release.github("git/ref/tags/new", missing_ok=True)

    def test_dry_run_does_not_create_tag_release_or_assets(self):
        self.prepare()
        with (
            patch.object(release, "tag_source", return_value=None),
            patch.object(release, "find_release", return_value=None),
            patch.object(
                release, "github", return_value={"tag_name": "fork-v0.153.4-fork.1"}
            ),
            patch.object(release.subprocess, "run") as run,
        ):
            release.publish_release(self.candidate, self.assets, execute=False)
        run.assert_not_called()

    def test_release_lookup_includes_drafts_on_later_pages(self):
        draft = self.draft()
        with patch.object(
            release,
            "github",
            side_effect=[
                [{"tag_name": "other"}] * 100,
                [draft],
            ],
        ) as api:
            self.assertEqual(release.find_release(draft["tag_name"]), draft)
        self.assertEqual(api.call_args_list[-1].args, ("releases?per_page=100&page=2",))

    def test_annotated_tag_resolves_to_binary_source(self):
        with patch.object(
            release,
            "github",
            side_effect=[
                {"object": {"type": "tag", "sha": "annotation"}},
                {"object": {"type": "commit", "sha": "source"}},
            ],
        ):
            self.assertEqual(release.tag_source("tag"), "source")

    def test_new_release_is_created_as_draft_at_validated_source(self):
        self.prepare()
        state = {"release": None}

        def command(args, **kwargs):
            if args[2] == "create":
                self.assertIn("--draft", args)
                self.assertEqual(args[args.index("--target") + 1], "source")
                state["release"] = self.draft()
            elif args[2] == "upload":
                path = Path(args[4])
                state["release"]["assets"].append(
                    {
                        "name": path.name,
                        "state": "uploaded",
                        "digest": release.sha256(path),
                    }
                )
            elif args[2] == "edit":
                self.assertEqual(len(state["release"]["assets"]), 7)
                state["release"]["draft"] = False
            else:
                self.fail(f"Unexpected command: {args}")

        with (
            patch.dict("os.environ", {"GITHUB_RUN_ID": "15"}),
            patch.object(release, "tag_source", side_effect=[None, None, "source"]),
            patch.object(
                release, "find_release", side_effect=lambda tag: state["release"]
            ),
            patch.object(
                release, "github", return_value={"tag_name": "fork-v0.153.4-fork.1"}
            ),
            patch.object(release.subprocess, "run", side_effect=command) as run,
        ):
            release.publish_release(self.candidate, self.assets, execute=True)
        self.assertEqual(
            [call.args[0][2] for call in run.call_args_list],
            ["create"] + ["upload"] * 7 + ["edit"],
        )

    def test_changed_latest_or_incomplete_public_release_prevents_writes(self):
        self.prepare()
        incomplete = self.draft(["SHA256SUMS"])
        incomplete["draft"] = False
        for existing in (None, incomplete):
            with (
                self.subTest(existing=existing),
                patch.object(release, "tag_source", return_value="source"),
                patch.object(release, "find_release", return_value=existing),
                patch.object(
                    release, "github", return_value={"tag_name": "fork-v0.153.4-fork.3"}
                ),
                patch.object(release.subprocess, "run") as run,
                self.assertRaises(ValueError),
            ):
                release.publish_release(self.candidate, self.assets, execute=True)
            run.assert_not_called()

    def test_complete_published_release_is_an_idempotent_noop(self):
        self.prepare()
        published = self.draft(path.name for path in self.assets.iterdir())
        published["draft"] = False
        with (
            patch.object(release, "tag_source", return_value="source"),
            patch.object(release, "find_release", return_value=published),
            patch.object(release.subprocess, "run") as run,
        ):
            release.publish_release(self.candidate, self.assets, execute=True)
        run.assert_not_called()

    def test_conflicting_source_or_asset_prevents_writes(self):
        self.prepare()
        draft = self.draft(["SHA256SUMS"])
        conflict = copy.deepcopy(draft)
        conflict["assets"][0]["digest"] = "sha256:wrong"
        for source, existing in (("other", draft), ("source", conflict)):
            with (
                self.subTest(source=source),
                patch.object(release, "tag_source", return_value=source),
                patch.object(release, "find_release", return_value=existing),
                patch.object(release.subprocess, "run") as run,
                self.assertRaises(ValueError),
            ):
                release.publish_release(self.candidate, self.assets, execute=True)
            run.assert_not_called()

    def test_partial_draft_resumes_missing_uploads_before_publication(self):
        self.prepare()
        draft = self.draft(["SHA256SUMS"])

        def command(args, **kwargs):
            if args[2] == "upload":
                path = Path(args[4])
                draft["assets"].append(
                    {
                        "name": path.name,
                        "state": "uploaded",
                        "digest": release.sha256(path),
                    }
                )
            elif args[2] == "edit":
                self.assertEqual(len(draft["assets"]), 7)
                draft["draft"] = False
            else:
                self.fail(f"Unexpected command: {args}")

        with (
            patch.object(release, "tag_source", return_value="source"),
            patch.object(release, "find_release", side_effect=lambda tag: draft),
            patch.object(
                release, "github", return_value={"tag_name": "fork-v0.153.4-fork.1"}
            ),
            patch.object(release.subprocess, "run", side_effect=command) as run,
        ):
            release.publish_release(self.candidate, self.assets, execute=True)
        self.assertEqual(
            [call.args[0][2] for call in run.call_args_list], ["upload"] * 6 + ["edit"]
        )

    def test_source_change_during_uploads_prevents_publication(self):
        self.prepare()
        draft = self.draft(path.name for path in self.assets.iterdir())
        with (
            patch.object(release, "tag_source", side_effect=["source", "other"]),
            patch.object(release, "find_release", return_value=draft),
            patch.object(release, "github", return_value=None),
            patch.object(release.subprocess, "run") as run,
            self.assertRaisesRegex(ValueError, "source changed"),
        ):
            release.publish_release(self.candidate, self.assets, execute=True)
        run.assert_not_called()
        self.assertTrue(draft["draft"])

    def test_retry_only_deletes_empty_starter_assets_from_drafts(self):
        self.prepare()
        draft = self.draft(["SHA256SUMS"])
        name = next(
            path.name for path in self.assets.iterdir() if path.name != "SHA256SUMS"
        )
        draft["assets"].append(
            {"id": 21, "name": name, "state": "starter", "size": 0, "digest": None}
        )
        with (
            patch.object(release, "tag_source", return_value="source"),
            patch.object(release, "find_release", return_value=draft),
            patch.object(release, "github", return_value=None),
            patch.object(release.subprocess, "run") as run,
        ):
            release.publish_release(self.candidate, self.assets, execute=False)
            run.assert_not_called()
            run.side_effect = [None, subprocess.CalledProcessError(1, "upload")]
            with self.assertRaises(subprocess.CalledProcessError):
                release.publish_release(self.candidate, self.assets, execute=True)
        self.assertEqual(
            run.call_args_list[0].args[0],
            [
                "gh",
                "api",
                f"repos/{release.REPOSITORY}/releases/assets/21",
                "--method",
                "DELETE",
            ],
        )
        self.assertEqual(run.call_args_list[1].args[0][2], "upload")
        draft["draft"] = False
        with self.assertRaises(ValueError):
            release.validate_release(
                draft,
                {path.name: release.sha256(path) for path in self.assets.iterdir()},
            )

    def test_empty_assets_directory_cannot_publish(self):
        self.assets.mkdir()
        with (
            patch.object(release, "tag_source", return_value=None),
            patch.object(release.subprocess, "run") as run,
            self.assertRaisesRegex(ValueError, "six native archives"),
        ):
            release.publish_release(self.candidate, self.assets, execute=True)
        run.assert_not_called()

    def test_upload_failure_keeps_draft_private(self):
        self.prepare()
        draft = self.draft()
        with (
            patch.object(release, "tag_source", return_value=None),
            patch.object(release, "find_release", return_value=draft),
            patch.object(release, "github", return_value=None),
            patch.object(
                release.subprocess,
                "run",
                side_effect=subprocess.CalledProcessError(1, "gh"),
            ) as run,
            self.assertRaises(subprocess.CalledProcessError),
        ):
            release.publish_release(self.candidate, self.assets, execute=True)
        self.assertEqual(run.call_count, 1)
        self.assertEqual(run.call_args.args[0][2], "upload")
        self.assertTrue(draft["draft"])

    def test_npm_failure_never_publishes_github_release(self):
        (self.root / "fork-release-candidate.json").write_text(
            json.dumps(self.candidate)
        )
        required = ["prepare", "package"] + [
            f"{prefix} {p['npm_tag']}"
            for p in release.BUILDER.CODEX_PLATFORM_PACKAGES.values()
            for prefix in ("Release build", "Install and launch")
        ]
        jobs = {"jobs": [{"name": name, "conclusion": "success"} for name in required]}
        for fail in (False, True):
            with (
                self.subTest(fail=fail),
                patch.dict(
                    "os.environ",
                    {
                        "GITHUB_REPOSITORY": release.REPOSITORY,
                        "GITHUB_EVENT_NAME": "workflow_dispatch",
                        "GITHUB_REF": "refs/heads/main",
                        "RELEASE_VERSION": self.candidate["version"],
                    },
                ),
                patch.object(sys, "argv", ["publish", "--publish"]),
                patch.object(publish, "ROOT", self.root),
                patch.object(publish, "release_config"),
                patch.object(publish, "github", return_value=jobs),
                patch.object(publish, "validate_run"),
                patch.object(publish, "validate_artifact"),
                patch.object(publish.subprocess, "run"),
                patch.object(publish, "unpack_verified"),
                patch.object(publish, "prepare_assets"),
                patch.object(
                    publish,
                    "publish_packages",
                    side_effect=ValueError("npm failed") if fail else None,
                ),
                patch.object(publish, "publish_release") as github_publish,
            ):
                if fail:
                    with self.assertRaisesRegex(ValueError, "npm failed"):
                        publish.main()
                else:
                    publish.main()
            self.assertEqual(
                [call.kwargs["execute"] for call in github_publish.call_args_list],
                [False] if fail else [False, True],
            )


if __name__ == "__main__":
    unittest.main()
