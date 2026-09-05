# Community fork release notes

This repository is a community-maintained fork of `openai/codex`. It is not an official OpenAI release and is not endorsed by OpenAI.

Fork-specific release metadata lives in `fork-release.json`. The npm package is built under a separate scope, while the native executable and command remain named `codex` for compatibility.

Releases use calendar versions in `YYYY.M.D` form and Git tags in `fork-vYYYY.M.D` form. If more than one release is needed on the same day, increment the patch component beyond the day, for example `2026.9.6` after `2026.9.5`.

The `fork-release.yml` workflow builds unsigned native packages on GitHub-hosted runners for Linux, macOS, and Windows on x64 and arm64. It can create a GitHub release without publishing to npm. npm publishing is a separate opt-in input so build artifacts can be inspected first.

For the first npm publication, create and verify the npm account that owns the configured scope, log in locally, and publish the six platform tarballs before the root tarball. After the package exists, configure npm trusted publishing for `PurpleSwords/codex` and workflow filename `fork-release.yml`; later releases can publish through GitHub Actions using OIDC.
