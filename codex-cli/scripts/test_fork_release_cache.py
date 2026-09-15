"""Compatible source edits may reuse caches, incompatible toolchains may not."""

from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "scripts"))
from fork_release_cache import cache_keys


class ReleaseCacheTests(unittest.TestCase):
    def setUp(self):
        self.environment = {
            "TARGET": "x86_64-apple-darwin",
            "ImageOS": "macos15",
            "ImageVersion": "20260901.1",
            "RELEASE_CACHE_CONFIG": "locked-config",
            "CODEX_FORK_VERSION": "0.153.4-fork.1",
            "CARGO_PROFILE_RELEASE_DEBUG": "0",
            "CARGO_BUILD_JOBS": "4",
            "CARGO_INCREMENTAL": "0",
        }

    def test_source_or_distribution_change_keeps_compatible_restore_prefix(self):
        key, prefix = cache_keys(self.environment, "rustc 1.95.0", "tree-a")
        changed_key, changed_prefix = cache_keys(
            self.environment, "rustc 1.95.0", "tree-b"
        )
        self.assertNotEqual(key, changed_key)
        self.assertEqual(prefix, changed_prefix)
        updated = dict(self.environment, CODEX_FORK_VERSION="0.153.4-fork.2")
        self.assertNotEqual(key, cache_keys(updated, "rustc 1.95.0", "tree-a")[0])
        self.assertEqual(prefix, cache_keys(updated, "rustc 1.95.0", "tree-a")[1])

    def test_build_identity_changes_never_share_a_restore_prefix(self):
        original = cache_keys(self.environment, "rustc 1.95.0", "tree-a")[1]
        for name in (
            "TARGET",
            "ImageOS",
            "ImageVersion",
            "RELEASE_CACHE_CONFIG",
            "CARGO_PROFILE_RELEASE_DEBUG",
            "CARGO_BUILD_JOBS",
        ):
            with self.subTest(name=name):
                changed = dict(self.environment)
                changed[name] += "-changed"
                self.assertNotEqual(
                    original, cache_keys(changed, "rustc 1.95.0", "tree-a")[1]
                )
        self.assertNotEqual(
            original, cache_keys(self.environment, "rustc 1.96.0", "tree-a")[1]
        )

    def test_missing_image_identity_is_rejected(self):
        for missing in ("ImageVersion", "RELEASE_CACHE_CONFIG"):
            changed = dict(self.environment)
            changed[missing] = ""
            with self.assertRaises(ValueError):
                cache_keys(changed, "rustc 1.95.0", "tree-a")
