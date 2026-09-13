use crate::create_model_provider;
use codex_model_provider_info::ModelProviderInfo;
use codex_protocol::openai_models::ModelsResponse;
use pretty_assertions::assert_eq;
use serde_json::json;

#[tokio::test]
async fn provider_applies_home_extensions_to_static_and_dynamic_catalogs() {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let home = std::env::temp_dir().join(format!(
        "codex-model-extensions-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir(&home).expect("create isolated home");
    let catalog = codex_models_manager::bundled_models_response().expect("bundled catalog");
    let original = catalog.models[0].clone();
    std::fs::write(
        home.join("models.extend.json"),
        serde_json::to_vec(&json!({"models": [{
            "slug": original.slug,
            "display_name": "Fork-local model label"
        }]}))
        .expect("serialize"),
    )
    .expect("write extensions");
    let provider = create_model_provider(ModelProviderInfo::default(), /*auth_manager*/ None);
    for configured in [
        None,
        Some(ModelsResponse {
            models: vec![original.clone()],
        }),
    ] {
        let manager = provider.models_manager(home.clone(), configured);
        let mut expected = original.clone();
        expected.display_name = "Fork-local model label".to_string();
        assert_eq!(
            manager
                .get_remote_models()
                .await
                .into_iter()
                .find(|entry| entry.slug == original.slug),
            Some(expected)
        );
    }
    assert_eq!(catalog.models[0], original);
    std::fs::remove_file(home.join("models.extend.json")).expect("remove test extensions");
    std::fs::remove_dir(&home).expect("remove isolated home");
}
