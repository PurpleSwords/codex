use super::*;
use codex_install_context::distribution::Distribution;
use pretty_assertions::assert_eq;

#[test]
fn update_command_keeps_the_installed_distribution() {
    for (action, prefix) in [
        (UpdateAction::NpmGlobalLatest, "npm install -g"),
        (UpdateAction::BunGlobalLatest, "bun install -g"),
        (UpdateAction::VitePlusGlobalLatest, "vp install -g"),
        (UpdateAction::PnpmGlobalLatest, "pnpm add -g"),
    ] {
        assert_eq!(
            action.command_str_for_distribution(Distribution::Fork),
            format!("{prefix} @purplesword/codex@latest")
        );
        assert_eq!(
            action.command_str_for_distribution(Distribution::Official),
            format!("{prefix} @openai/codex")
        );
    }
}
