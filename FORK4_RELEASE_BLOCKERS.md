# fork.4 release blockers

Candidate source: `5baa3bd4234c88eeeff22a4e0e4be0391d7cb6f0`.
Release validation run: https://github.com/PurpleSwords/codex/actions/runs/45039040729
(six builds, packaging, and six install/launch checks succeeded).
Blocking CI: https://github.com/PurpleSwords/codex/actions/runs/45039019569
(failed).

## Prepared corrections

- Update the bundled model-picker snapshot from the actual CI diff. The menu
  displays GPT-6.2 Sol as the default and GPT-6.2 Astra alongside existing models.
- Include the bundled-catalog snapshot in the targeted model-catalog CI profile;
  the old filter only selected the separately mocked catalog test.
- Remove the optional Fence host-audit wrapper from the blob-size job. Its
  executable failed with EACCES before checkout. The actual blob-size policy and
  clean-worktree checks remain required.
- Give Dylint 5 a local Clippy mirror with a `master` alias at the lint package's
  pinned revision. Clippy renamed its branch to `main`; Dylint 5 hard-codes
  `master`. Keep the existing nightly and lint version, and retain source history
  needed by its compiler-symbol lookup. This requires CI execution to validate.
  Upstream diagnosis: https://github.com/trailofbits/dylint/commit/f1ed83623faead3799afd91533f8647715272c33

## Dependency audit remains blocking

Cargo.toml, Cargo.lock, and deny.toml are unchanged between the previous green
source `61c6fd4b08` and this candidate. The model synchronization introduced no
dependency update. Nevertheless the current audit reports 52 errors, which must
not be represented as passing just because they predate the model change.

The table summarizes the failed run's advisory output, not an independent
exploitability assessment. Versions are the run's reported minimum patched
versions; compatibility and transitive constraints still need evaluation.
An omitted safe upgrade means the advisory did not offer one for that finding.

| Locked crate                           | Reported next step                                                      |
| -------------------------------------- | ----------------------------------------------------------------------- |
| basic-toml 0.1.10                      | Unmaintained; replace or review continued use                           |
| chrono 0.4.43                          | Unmaintained; replace or review continued use                           |
| crypto-bigint 0.5.5                    | 0.7.6; requires compatibility work                                      |
| hickory-proto 0.25.2                   | Some fixes in 0.26.2; other DNSSEC findings have no safe upgrade listed |
| hickory-resolver 0.25.2                | 0.26.2                                                                  |
| libsqlite3-sys 0.37.0                  | 0.39.0                                                                  |
| openssl 0.10.75                        | 0.10.80 covers the highest reported minimum                             |
| pin-utils 0.1.0                        | No safe upgrade listed                                                  |
| rmcp 3.1.3                             | 3.9.0 covers the highest reported minimum                               |
| rustls 0.23.45                         | 0.23.46                                                                 |
| rustls-webpki 0.103.15                 | 0.103.16 on the existing release line                                   |
| unsafe-libyaml 0.2.11                  | No safe upgrade listed                                                  |
| url 2.5.8                              | 2.5.9                                                                   |
| webpki-roots 1.0.5                     | 1.0.10                                                                  |
| zbus_names 3.0.0                       | No safe upgrade listed                                                  |
| zip 2.4.2                              | 9.0.5; requires compatibility work                                      |
| zstd 0.13.3                            | 0.13.4 on the existing release line                                     |
| zstd-safe 7.2.4                        | 7.4.0 on the existing release line                                      |
| zstd-sys 2.0.16+zstd.1.5.7             | 2.1.2                                                                   |
| zvariant 4.2.0 / zvariant_derive 4.2.0 | No safe upgrade listed                                                  |

No advisory exceptions or publish-gate relaxations are included in this repair.
Do not update the release candidate record to claim these checks passed.
Dependency remediation changes the binary inputs and requires new validation;
the old six-platform artifacts cannot certify that changed dependency graph.
