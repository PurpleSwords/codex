# Community Codex CLI

This is the community-maintained [PurpleSwords/codex](https://github.com/PurpleSwords/codex)
fork of [OpenAI Codex](https://github.com/openai/codex), not an official OpenAI package.

The npm package name is `@purplesword/codex`; its executable is `codex`.
It shares that executable name with the official package, so do not install both
globally into the same prefix. Use an isolated prefix when validating a candidate.

The root package selects one of six platform payloads through npm optional
dependency aliases. Each payload retains the native Codex package layout,
including the code-mode companion executable. LICENSE and NOTICE accompany the
packages. This fork does not imply OpenAI endorsement.

## Backend selection

Ordinary launches, including `resume`, `fork`, and session management commands,
use the backend embedded in the fork executable. An existing official background
server in the same Codex home does not replace that backend.

Connect to a shared server explicitly with `--remote`. For the default local
daemon socket, use `codex --remote unix://` or `codex agents --remote unix://`.
The selected server then supplies model requests and tools, so choose a server
built from the fork when those backend changes are needed. `codex agents` requires
`--remote`; daemon lifecycle commands remain available for intentional management.

HTTPS requests negotiate HTTP/2 when the server supports it and retain HTTP/1.1
compatibility otherwise.

Version `0.153.4` has already been published from the previous fork history.
Rebuilding the current baseline does not replace that npm version. Candidate
packaging is not authorization to publish or evidence of release readiness.

## Fork version policy

The distribution version is `<upstream-version>-fork.<revision>`, for example
`0.153.4-fork.1`. Revisions start at 1 and increase for the same upstream baseline;
they restart at 1 when the upstream baseline changes. These are examples, not
allocated or published versions. The packager verifies that the upstream portion
matches `codex-rs/Cargo.toml` and rejects bare versions for this fork.

Rust workspace and lockfile versions remain the upstream version. The npm root
and six platform manifests record `codexUpstreamVersion` separately. The launcher
passes `CODEX_NPM_PACKAGE_VERSION` from the installed root package; this metadata
must not replace the native version used for protocol or model compatibility.

Under SemVer, the suffix is a prerelease and sorts below the corresponding bare
version. Fork upgrade policy therefore compares upstream versions first, then
numeric revisions. Only the previously published fork `0.153.4` is treated as
revision zero for migration. This policy is not applicable to official installs;
runtime updater integration is still pending. Do not rely on ordinary npm version
ranges for this migration. A future stable distribution uses an explicitly chosen
`latest` dist-tag, not the highest SemVer version among root and platform payloads.

Print a read-only plan (it does not check npm availability or publish anything):

```sh
python3 scripts/fork_release.py --version 0.153.4-fork.1 --current-version 0.153.4
```

## Publishing validated builds

Run the `fork-release` workflow on `main` with mode `validate` to build and verify
all six platforms. Record the successful source CI, validation run and npm artifact
identity in `fork-release-candidate.json` before promotion.

Mode `promote-check` checks the reviewed npm packages and native GitHub archives
without publishing. Mode `publish` first publishes npm, then automatically creates
`fork-v<version>` as the latest GitHub Release, attaching the same six native
archives and `SHA256SUMS`. Promotion reuses the successful build artifacts and
does not compile again. A failed npm publication prevents GitHub publication.

GitHub assets upload to a draft, which becomes public only after all checksums
match. Retry mode `publish` to resume an interrupted promotion; byte-identical
npm packages and GitHub assets are kept, while conflicting versions, tags or
assets cause promotion to fail. Validation artifacts expire after seven days,
so promote the reviewed candidate while its artifacts are still available.
