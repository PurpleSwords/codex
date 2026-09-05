# Community fork release notes

This repository is a community-maintained fork of `openai/codex`. It is not an official OpenAI release and is not endorsed by OpenAI.

Fork-specific release metadata lives in `fork-release.json`. The npm package is built under a separate scope, while the native executable and command remain named `codex` for compatibility.

Formal releases reuse the upstream Codex version they track, for example npm version `0.153.4` and Git tag `fork-v0.153.4`. The separate npm scope and `fork-v` tag prefix distinguish these builds from official releases. A given version is published only once; later fork releases move to a newer upstream Codex version.

The `fork-release.yml` workflow builds unsigned native packages on GitHub-hosted runners for Linux, macOS, and Windows on x64 and arm64. It can create a GitHub release without publishing to npm. npm publishing is a separate opt-in input so build artifacts can be inspected first.

The npm package was initialized with `0.0.0-init` under the non-default `init` dist-tag solely to establish ownership. Formal versions are published by GitHub Actions after configuring npm trusted publishing for `PurpleSwords/codex` and workflow filename `fork-release.yml`; no long-lived npm token is required.
