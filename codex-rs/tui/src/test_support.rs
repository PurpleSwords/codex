//! Test-only helpers shared across the TUI crate.

use std::sync::LazyLock;

use codex_models_manager::bundled_models_response;
use codex_protocol::openai_models::ModelPreset;
pub(crate) use codex_utils_absolute_path::test_support::PathBufExt;
pub(crate) use codex_utils_absolute_path::test_support::test_path_buf;
use serde::Serialize;
use serde::de::DeserializeOwned;

pub(crate) static TEST_MODEL_PRESETS: LazyLock<Vec<ModelPreset>> = LazyLock::new(|| {
    let mut response = bundled_models_response()
        .unwrap_or_else(|err| panic!("bundled models.json should parse: {err}"));
    response.models.sort_by_key(|model| model.priority);
    let mut presets: Vec<ModelPreset> = response.models.into_iter().map(Into::into).collect();
    ModelPreset::mark_default_by_picker_visibility(&mut presets);
    presets
});

pub(crate) fn test_path_display(path: &str) -> String {
    test_path_buf(path).display().to_string()
}

/// Stabilize version text in rendered snapshot headers without changing their
/// width, other content, or the version used by production code.
pub(crate) fn normalize_cli_version(text: &str) -> String {
    normalize_version(text, crate::version::CODEX_CLI_VERSION)
}

fn normalize_version(text: &str, version: &str) -> String {
    text.split('\n')
        .map(|line| {
            let header = format!("OpenAI Codex (v{version})");
            let update = format!("Update available! {version} ->");
            let replacement = if line.contains(&header) {
                line.replacen(&header, "OpenAI Codex (v0.0.0)", 1)
            } else if line.contains(&update) {
                line.replacen(&update, "Update available! 0.0.0 ->", 1)
            } else {
                return line.to_string();
            };
            let Some((content, suffix)) = replacement.rsplit_once('│') else {
                return replacement;
            };
            let trimmed = content.trim_end_matches(' ');
            let Some(padding) =
                (content.len() - trimmed.len() + version.len()).checked_sub("0.0.0".len())
            else {
                // Do not hide wrapping/truncation changes when the fixture
                // version would no longer fit inside the rendered border.
                return line.to_string();
            };
            format!("{trimmed}{}│{suffix}", " ".repeat(padding))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[path = "test_support_tests.rs"]
mod tests;

pub(crate) fn session_source_cli<T>() -> T
where
    T: DeserializeOwned,
{
    from_app_server_wire(codex_app_server_protocol::SessionSource::Cli)
}

pub(crate) fn skill_scope_user<T>() -> T
where
    T: DeserializeOwned,
{
    from_app_server_wire(codex_app_server_protocol::SkillScope::User)
}

pub(crate) fn skill_scope_repo<T>() -> T
where
    T: DeserializeOwned,
{
    from_app_server_wire(codex_app_server_protocol::SkillScope::Repo)
}

fn from_app_server_wire<T>(value: impl Serialize) -> T
where
    T: DeserializeOwned,
{
    serde_json::to_value(value)
        .and_then(serde_json::from_value)
        .unwrap_or_else(|err| {
            panic!("app-server wire value should map to legacy helper type: {err}")
        })
}
