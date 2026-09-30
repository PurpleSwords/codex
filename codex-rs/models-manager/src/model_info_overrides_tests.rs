use crate::ModelsManagerConfig;
use crate::manager::ModelsManager;
use crate::manager::RefreshStrategy;
use crate::test_support::test_models_response;
use codex_protocol::openai_models::ModelsResponse;
use codex_protocol::openai_models::TruncationPolicyConfig;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

use super::DEFAULT_HTTP_CLIENT_FACTORY;
use super::TestModelsEndpoint;
use super::openai_manager_for_tests;
use super::remote_model;

#[tokio::test]
async fn fallback_catalog_preserves_remote_refresh_and_cache() {
    let codex_home = TempDir::new().expect("create temp dir");
    let fallback = ModelsResponse {
        models: vec![remote_model(
            "test-offline",
            "Offline model",
            /*priority*/ 0,
        )],
    };
    let remote = ModelsResponse {
        models: vec![remote_model(
            "test-remote",
            "Remote model",
            /*priority*/ 0,
        )],
    };
    let endpoint = TestModelsEndpoint::new(vec![Vec::new(), remote.models.clone()]);
    let manager = openai_manager_for_tests(codex_home.path().to_path_buf(), endpoint.clone())
        .with_fallback_catalog(fallback.clone());

    assert_eq!(
        manager
            .raw_model_catalog(RefreshStrategy::Offline, DEFAULT_HTTP_CLIENT_FACTORY)
            .await,
        fallback
    );
    assert_eq!(
        manager
            .raw_model_catalog(RefreshStrategy::Online, DEFAULT_HTTP_CLIENT_FACTORY)
            .await,
        fallback
    );
    assert_eq!(
        manager
            .raw_model_catalog(RefreshStrategy::Online, DEFAULT_HTTP_CLIENT_FACTORY)
            .await,
        remote
    );
    assert_eq!(
        manager
            .raw_model_catalog(
                RefreshStrategy::OnlineIfUncached,
                DEFAULT_HTTP_CLIENT_FACTORY
            )
            .await,
        remote
    );
    assert_eq!(endpoint.fetch_count(), 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn offline_model_info_without_tool_output_override() {
    let codex_home = TempDir::new().expect("create temp dir");
    let config = ModelsManagerConfig::default();
    let manager = openai_manager_for_tests(
        codex_home.path().to_path_buf(),
        TestModelsEndpoint::new(Vec::new()),
    )
    .with_fallback_catalog(test_models_response().expect("test models.json should parse"));

    let model_info = manager.get_model_info("gpt-5.2", &config).await;

    assert_eq!(
        model_info.truncation_policy,
        TruncationPolicyConfig::bytes(/*limit*/ 10_000)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn offline_model_info_with_tool_output_override() {
    let codex_home = TempDir::new().expect("create temp dir");
    let config = ModelsManagerConfig {
        tool_output_token_limit: Some(123),
        ..Default::default()
    };
    let manager = openai_manager_for_tests(
        codex_home.path().to_path_buf(),
        TestModelsEndpoint::new(Vec::new()),
    )
    .with_fallback_catalog(test_models_response().expect("test models.json should parse"));

    let model_info = manager.get_model_info("gpt-5.4", &config).await;

    assert_eq!(
        model_info.truncation_policy,
        TruncationPolicyConfig::tokens(/*limit*/ 123)
    );
}
