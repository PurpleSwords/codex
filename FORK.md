# Community fork release notes

This repository is a community-maintained fork of `openai/codex`. It is not an official OpenAI release and is not endorsed by OpenAI.

Fork-specific release metadata lives in `fork-release.json`. The npm package is built under a separate scope, while the native executable and command remain named `codex` for compatibility.

Formal releases reuse the upstream Codex version they track, for example npm version `0.153.4` and Git tag `fork-v0.153.4`. The separate npm scope and `fork-v` tag prefix distinguish these builds from official releases. A given version is published only once; later fork releases move to a newer upstream Codex version.

The `fork-release.yml` workflow builds unsigned native packages on GitHub-hosted runners for Linux, macOS, and Windows on x64 and arm64. It archives release symbols and strips Unix binaries before packaging, matching the upstream release process closely enough to keep npm artifacts within registry limits. It can create a GitHub release without publishing to npm; rerunning a full build replaces assets on the existing matching release. npm publishing is a separate opt-in input so build artifacts can be inspected first.

The npm package was initialized with `0.0.0-init` under the non-default `init` dist-tag solely to establish ownership. Formal versions are published by GitHub Actions after configuring npm trusted publishing for `PurpleSwords/codex` and workflow filename `fork-release.yml`; no long-lived npm token is required.

If npm publishing fails after a GitHub release has already been created, rerun `fork-release.yml` with `publish_npm` and `reuse_release_assets` enabled. The recovery run skips native builds, downloads the npm tarballs from the existing `fork-v<version>` release, skips package versions that already exist on npm, and publishes the remainder through the same trusted workflow.

If one platform build fails after other matrix jobs succeed, select that `build_target` and set `reuse_run_id` to the failed workflow run. The repair run downloads the successful native artifacts from that run, builds only the selected platform with a longer timeout, then assembles and publishes a complete six-platform release. If the target artifact was built successfully but a later packaging step failed, set `target_artifact_run_id` to that repair run to reuse it without rebuilding.
