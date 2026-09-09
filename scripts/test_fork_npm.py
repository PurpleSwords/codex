"""Exercise fork metadata through the real npm pack command without publishing."""

import importlib.util
import json
from pathlib import Path
import tarfile
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "build_npm_package", ROOT / "codex-cli/scripts/build_npm_package.py"
)
BUILDER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(BUILDER)
NAME = "@purplesword/codex"
VERSION = "0.153.4"


class ForkNpmTest(unittest.TestCase):
    def test_pack_root_and_six_platform_payloads(self):
        with tempfile.TemporaryDirectory(prefix="fork-npm-test-") as temporary:
            root = Path(temporary)
            packages = {}
            for package in BUILDER.PACKAGE_EXPANSIONS["codex"]:
                with self.subTest(package=package):
                    stage = root / package
                    stage.mkdir()
                    BUILDER.stage_sources(
                        stage,
                        VERSION,
                        package,
                        npm_name=NAME,
                        repository_url="https://github.com/PurpleSwords/codex.git",
                        description="Community Codex",
                        readme=ROOT / "codex-cli/README.fork.md",
                    )
                    platform = BUILDER.CODEX_PLATFORM_PACKAGES.get(package)
                    if platform:
                        # These are packaging fixtures, not runnable platform builds.
                        vendor = root / f"vendor-{package}"
                        binaries = vendor / platform["target_triple"] / "bin"
                        binaries.mkdir(parents=True)
                        suffix = ".exe" if platform["os"] == "win32" else ""
                        for binary in ("codex", "codex-code-mode-host"):
                            (binaries / f"{binary}{suffix}").write_bytes(b"fixture")
                        BUILDER.copy_native_binaries(
                            vendor,
                            stage,
                            ["codex-package"],
                            {platform["target_triple"]},
                        )
                    archive = BUILDER.run_npm_pack(stage, root / f"{package}.tgz")
                    with tarfile.open(archive) as packed:
                        metadata = json.load(packed.extractfile("package/package.json"))
                        names = packed.getnames()
                    packages[package] = metadata
                    self.assertEqual(metadata["name"], NAME)
                    self.assertEqual(
                        metadata["repository"]["url"],
                        "https://github.com/PurpleSwords/codex.git",
                    )
                    if platform:
                        self.assertEqual(
                            metadata["version"], f"{VERSION}-{platform['npm_tag']}"
                        )
                        self.assertEqual(metadata["os"], [platform["os"]])
                        self.assertEqual(metadata["cpu"], [platform["cpu"]])
                        prefix = f"package/vendor/{platform['target_triple']}/bin"
                        for binary in ("codex", "codex-code-mode-host"):
                            self.assertIn(f"{prefix}/{binary}{suffix}", names)
                    else:
                        self.assertIn("package/bin/codex.js", names)
                        self.assertEqual(metadata["version"], VERSION)
                        self.assertEqual(
                            metadata["optionalDependencies"],
                            {
                                f"{NAME}-{tag}": f"npm:{NAME}@{VERSION}-{tag}"
                                for tag in (
                                    "linux-x64",
                                    "linux-arm64",
                                    "darwin-x64",
                                    "darwin-arm64",
                                    "win32-x64",
                                    "win32-arm64",
                                )
                            },
                        )
            self.assertEqual(len(packages), 7)


if __name__ == "__main__":
    unittest.main()
