"""Packaging contracts using tiny fixture payloads, never a Rust build or publish."""

import importlib.util
import json
import os
import platform
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import unittest


SCRIPT = Path(__file__).with_name("build_npm_package.py")
SPEC = importlib.util.spec_from_file_location("build_npm_package", SCRIPT)
builder = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(builder)
FORK_NAME = "@purplesword/codex"
REPOSITORY = "git+https://github.com/PurpleSwords/codex.git"
UPSTREAM_VERSION = builder.read_workspace_version(
    builder.REPO_ROOT / "codex-rs" / "Cargo.toml"
)
VERSION = f"{UPSTREAM_VERSION}-fork.1"


class ForkNpmTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="codex-npm-contract-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.env = os.environ.copy()
        self.env.update(
            NPM_CONFIG_CACHE=str(self.root / "cache"),
            NPM_CONFIG_USERCONFIG=str(self.root / "npmrc"),
            NPM_CONFIG_AUDIT="false",
            NPM_CONFIG_FUND="false",
        )
        (self.root / "npmrc").write_text("", encoding="utf-8")

    def run_command(self, args, cwd=None, expected=0, env=None):
        result = subprocess.run(
            args,
            cwd=cwd or self.root,
            env=env or self.env,
            text=True,
            capture_output=True,
            timeout=60,
        )
        self.assertEqual(result.returncode, expected, result.stdout + result.stderr)
        return result

    def stage(self, directory, package="codex", name=FORK_NAME):
        directory.mkdir(parents=True)
        builder.stage_sources(
            directory,
            VERSION if name == FORK_NAME else UPSTREAM_VERSION,
            package,
            npm_name=name,
            repository_url=REPOSITORY if name == FORK_NAME else None,
            readme=builder.CODEX_CLI_ROOT / "README.fork.md",
        )
        return json.loads((directory / "package.json").read_text())

    def test_official_and_fork_root_aliases(self):
        for name in (builder.CODEX_NPM_NAME, FORK_NAME):
            with self.subTest(name=name):
                directory = self.root / name.replace("/", "-")
                manifest = self.stage(directory, name=name)
                version = VERSION if name == FORK_NAME else UPSTREAM_VERSION
                expected = {
                    f"{name}-{tag}": f"npm:{name}@{version}-{tag}"
                    for tag in (
                        "linux-x64",
                        "linux-arm64",
                        "darwin-x64",
                        "darwin-arm64",
                        "win32-x64",
                        "win32-arm64",
                    )
                }
                self.assertEqual(manifest["name"], name)
                self.assertEqual(manifest["version"], version)
                self.assertEqual(manifest["optionalDependencies"], expected)
                if name == FORK_NAME:
                    self.assertEqual(manifest["codexUpstreamVersion"], UPSTREAM_VERSION)
                else:
                    self.assertNotIn("codexUpstreamVersion", manifest)

    def test_all_platform_tarballs_preserve_native_layout_and_attribution(self):
        for package, config in builder.CODEX_PLATFORM_PACKAGES.items():
            with self.subTest(package=package):
                staging = self.root / package
                manifest = self.stage(staging, package)
                triple = config["target_triple"]
                vendor = self.root / f"fixture-{package}"
                binaries = vendor / triple / "bin"
                binaries.mkdir(parents=True)
                suffix = ".exe" if config["os"] == "win32" else ""
                names = [f"codex{suffix}", f"codex-code-mode-host{suffix}"]
                for name in names:
                    (binaries / name).write_bytes(b"fixture-only-not-a-release")
                builder.copy_native_binaries(
                    vendor, staging, [builder.CODEX_PACKAGE_COMPONENT], {triple}
                )
                self.assertEqual(
                    (
                        manifest["name"],
                        manifest["version"],
                        manifest["os"],
                        manifest["cpu"],
                    ),
                    (
                        FORK_NAME,
                        f"{VERSION}-{config['npm_tag']}",
                        [config["os"]],
                        [config["cpu"]],
                    ),
                )
                self.assertEqual(manifest["repository"]["url"], REPOSITORY)
                self.assertEqual(manifest["codexUpstreamVersion"], UPSTREAM_VERSION)
                packed = json.loads(
                    self.run_command(
                        ["npm", "pack", "--json", "--ignore-scripts"], staging
                    ).stdout
                )
                with tarfile.open(staging / packed[0]["filename"]) as archive:
                    contents = set(archive.getnames())
                    for name in names:
                        self.assertIn(f"package/vendor/{triple}/bin/{name}", contents)
                    for name in ("LICENSE", "NOTICE", "README.md"):
                        self.assertIn(f"package/{name}", contents)

    def test_root_pack_dry_run_and_isolated_install(self):
        staging = self.root / "staged"
        self.stage(staging)
        preview = self.run_command(
            ["npm", "pack", "--dry-run", "--json", "--ignore-scripts"], staging
        )
        self.assertEqual(json.loads(preview.stdout)[0]["name"], FORK_NAME)
        packed = json.loads(
            self.run_command(
                ["npm", "pack", "--json", "--ignore-scripts"], staging
            ).stdout
        )
        prefix = self.root / "installed"
        self.run_command(
            [
                "npm",
                "install",
                "--prefix",
                str(prefix),
                "--offline",
                "--omit=optional",
                "--ignore-scripts",
                "--no-package-lock",
                str(staging / packed[0]["filename"]),
            ]
        )
        installed = prefix / "node_modules" / "@purplesword" / "codex"
        self.assertEqual(
            json.loads((installed / "package.json").read_text())["name"], FORK_NAME
        )
        # The root intentionally has no native payload; its repair advice must
        # identify the fork rather than replacing it with the official package.
        env = self.env | {"npm_config_user_agent": "npm/test", "npm_execpath": ""}
        result = self.run_command(
            ["node", str(installed / "bin" / "codex.js")], expected=1, env=env
        )
        self.assertIn("npm install -g @purplesword/codex@latest", result.stderr)
        self.assertNotIn("@openai/codex", result.stderr)

    @unittest.skipUnless(
        sys.platform == "linux" and platform.machine() in ("x86_64", "AMD64"),
        "the executable fixture uses the Linux x64 payload layout",
    )
    def test_launcher_resolves_alias_and_forwards_identity_arguments_and_exit(self):
        installed = self.root / "node_modules" / "@purplesword" / "codex"
        self.stage(installed)
        # Mimic npm's alias directory. The real executable is deliberately a tiny
        # probe; this checks the launcher contract, not native CLI functionality.
        triple = "x86_64-unknown-linux-musl"
        alias = installed.parent / "codex-linux-x64"
        binary = alias / "vendor" / triple / "bin" / "codex"
        binary.parent.mkdir(parents=True)
        (alias / "package.json").write_text(
            json.dumps({"name": FORK_NAME}), encoding="utf-8"
        )
        binary.write_text(
            "#!/usr/bin/env node\n"
            "console.log(JSON.stringify({name:process.env.CODEX_NPM_PACKAGE_NAME,"
            "version:process.env.CODEX_NPM_PACKAGE_VERSION,"
            "repo:process.env.CODEX_GITHUB_REPOSITORY,args:process.argv.slice(2)}));\n"
            "process.exit(7);\n",
            encoding="utf-8",
        )
        binary.chmod(0o755)
        env = self.env | {
            "CODEX_GITHUB_REPOSITORY": "stale/repository",
            "CODEX_NPM_PACKAGE_VERSION": "stale-version",
        }
        result = self.run_command(
            ["node", str(installed / "bin" / "codex.js"), "--version", "two words"],
            expected=7,
            env=env,
        )
        self.assertEqual(
            json.loads(result.stdout),
            {
                "name": FORK_NAME,
                "version": VERSION,
                "repo": "PurpleSwords/codex",
                "args": ["--version", "two words"],
            },
        )

    def test_custom_name_requires_repository_before_staging(self):
        for name in (FORK_NAME, "invalid name"):
            with self.subTest(name=name):
                staging = self.root / name.replace("/", "-")
                result = self.run_command(
                    [
                        "python3",
                        str(SCRIPT),
                        "--version",
                        VERSION,
                        "--npm-name",
                        name,
                        "--staging-dir",
                        str(staging),
                    ],
                    expected=1,
                )
                self.assertIn("RuntimeError", result.stderr)
                self.assertFalse(staging.exists())


if __name__ == "__main__":
    unittest.main()
