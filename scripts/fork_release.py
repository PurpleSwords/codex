#!/usr/bin/env python3
"""Helpers for the community fork release workflow."""

import argparse
import json
import re
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parent.parent
CONFIG_PATH = REPO_ROOT / "fork-release.json"
WORKSPACE_MANIFEST = REPO_ROOT / "codex-rs" / "Cargo.toml"
VERSION_PATTERN = re.compile(
    r"(?:[1-9][0-9]{3})\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)"
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)

    config = subparsers.add_parser("config", help="Print one release config value.")
    config.add_argument("field")

    validate = subparsers.add_parser("validate-version", help="Validate a release version.")
    validate.add_argument("version")

    set_version = subparsers.add_parser(
        "set-version", help="Set the workspace version in the checked-out CI tree."
    )
    set_version.add_argument("version")
    return parser.parse_args()


def load_config() -> dict[str, str]:
    with open(CONFIG_PATH, encoding="utf-8") as source:
        config = json.load(source)
    if not isinstance(config, dict):
        raise RuntimeError(f"Expected a JSON object in {CONFIG_PATH}")
    return config


def validate_version(version: str) -> str:
    if VERSION_PATTERN.fullmatch(version) is None:
        raise RuntimeError(f"Invalid calendar version: {version}")
    return version


def set_workspace_version(version: str) -> None:
    version = validate_version(version)
    source = WORKSPACE_MANIFEST.read_text(encoding="utf-8")
    workspace_start = source.find("[workspace.package]")
    if workspace_start < 0:
        raise RuntimeError("Missing [workspace.package] in codex-rs/Cargo.toml")
    next_section = source.find("\n[", workspace_start + 1)
    workspace_end = len(source) if next_section < 0 else next_section
    workspace_section = source[workspace_start:workspace_end]
    updated_section, replacements = re.subn(
        r'(?m)^version = "[^"]+"$',
        f'version = "{version}"',
        workspace_section,
        count=1,
    )
    if replacements != 1:
        raise RuntimeError("Expected one workspace package version")
    WORKSPACE_MANIFEST.write_text(
        source[:workspace_start] + updated_section + source[workspace_end:],
        encoding="utf-8",
    )


def main() -> int:
    args = parse_args()
    if args.command == "config":
        value = load_config().get(args.field)
        if not isinstance(value, str) or not value:
            raise RuntimeError(f"Missing string config field: {args.field}")
        print(value)
    elif args.command == "validate-version":
        print(validate_version(args.version))
    elif args.command == "set-version":
        set_workspace_version(args.version)
        print(f"Set workspace version to {args.version}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
