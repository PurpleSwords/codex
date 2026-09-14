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

Version `0.153.4` has already been published from the previous fork history.
Rebuilding the current baseline does not replace that npm version. Candidate
packaging is not authorization to publish or evidence of release readiness.
