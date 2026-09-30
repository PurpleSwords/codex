//! Test-only helpers exposed for dependent crate tests.
//!
//! Production code should not depend on this module.

use crate::ModelsManagerConfig;
use crate::manager::construct_model_info_from_candidates;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelPreset;
use codex_protocol::openai_models::ModelsResponse;

/// Stable metadata for behavioral tests, independent of the live model catalog.
/// Entries here are fixtures, not a statement of service availability.
pub fn test_models_response() -> serde_json::Result<ModelsResponse> {
    serde_json::from_str(include_str!("../tests/fixtures/models.json"))
}

/// Get model identifier without consulting remote state or cache.
pub fn get_model_offline_for_tests(model: Option<&str>) -> String {
    if let Some(model) = model {
        return model.to_string();
    }
    let mut response =
        test_models_response().unwrap_or_else(|err| panic!("test models.json should parse: {err}"));
    response.models.sort_by_key(|model| model.priority);
    let presets: Vec<ModelPreset> = response.models.into_iter().map(Into::into).collect();
    presets
        .iter()
        .find(|preset| preset.show_in_picker)
        .or_else(|| presets.first())
        .map(|preset| preset.model.clone())
        .unwrap_or_default()
}

/// Build `ModelInfo` without consulting remote state or cache.
pub fn construct_model_info_offline_for_tests(
    model: &str,
    config: &ModelsManagerConfig,
) -> ModelInfo {
    let candidates: &[ModelInfo] = if let Some(model_catalog) = config.model_catalog.as_ref() {
        &model_catalog.models
    } else {
        &[]
    };
    construct_model_info_from_candidates(model, candidates, config)
}
