use super::*;

#[test]
fn fork_update_notice_snapshot() {
    let cell = UpdateAvailableHistoryCell {
        distribution: Distribution::Fork,
        current_version: "0.153.4-fork.1".to_string(),
        latest_version: "0.153.4-fork.2".to_string(),
        update_action: Some(UpdateAction::NpmGlobalLatest),
    };
    let text = cell
        .raw_lines()
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    insta::assert_snapshot!(text, @"
    Update available!
    0.153.4-fork.1 -> 0.153.4-fork.2
    Run npm install -g @purplesword/codex@latest to update.

    See full release notes:
    https://github.com/PurpleSwords/codex/releases
    ");
}
