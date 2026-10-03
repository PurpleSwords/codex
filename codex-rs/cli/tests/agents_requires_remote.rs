use assert_cmd::Command;
use codex_utils_cargo_bin::cargo_bin;
use predicates::prelude::*;

#[test]
fn shared_agents_requires_an_explicit_server() {
    let codex_home = tempfile::TempDir::new().expect("temporary Codex home");
    Command::new(cargo_bin("codex").expect("codex binary"))
        .env("CODEX_HOME", codex_home.path())
        .arg("agents")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "`codex agents` requires an explicit server; use `codex agents --remote unix://`",
        ));
}
