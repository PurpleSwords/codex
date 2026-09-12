use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn role_provider_selection_and_replacement_preserve_parent_config() {
    let parent_provider = ModelProviderInfo {
        name: "parent".to_string(),
        base_url: Some("https://parent.example/v1".to_string()),
        env_key: Some("PARENT_PROVIDER_KEY".to_string()),
        http_headers: Some(HashMap::from([(
            "x-parent-secret".to_string(),
            "secret".into(),
        )])),
        ..Default::default()
    };
    let (home, mut parent) = test_config_with_cli_overrides(vec![(
        "model_providers.custom".to_string(),
        TomlValue::try_from(&parent_provider).expect("serialize provider"),
    )])
    .await;
    let role_path = write_role_config(
        &home,
        "provider.toml",
        r#"
model = "child-model"
model_provider = "custom"
[model_providers.custom]
name = "child"
base_url = "https://child.example/v1"
wire_api = "responses"
"#,
    )
    .await;
    parent.agent_roles.insert(
        "custom".to_string(),
        AgentRoleConfig {
            description: None,
            config_file: Some(role_path),
            nickname_candidates: None,
        },
    );
    let mut child = parent.clone();
    apply_role_to_config(&mut child, Some("custom"))
        .await
        .expect("role should apply");
    let expected = ModelProviderInfo {
        name: "child".to_string(),
        base_url: Some("https://child.example/v1".to_string()),
        ..Default::default()
    };
    assert_eq!(child.model_provider, expected);
    assert_eq!(child.model_providers["custom"], expected);
    assert_eq!(parent.model_providers["custom"], parent_provider);
    assert_eq!(child.permissions, parent.permissions);
    assert_eq!(
        child.config_layer_stack.effective_config()["model_providers"]["custom"],
        TomlValue::try_from(&expected).expect("serialize provider")
    );
    assert_eq!(
        child.config_layer_stack.effective_config()["model_provider"].as_str(),
        Some("custom")
    );
}

#[tokio::test]
async fn role_can_select_an_existing_provider_without_redefining_it() {
    let (home, mut config) = test_config_with_cli_overrides(Vec::new()).await;
    let role_path = write_role_config(&home, "provider.toml", "model_provider = 'ollama'").await;
    config.agent_roles.insert(
        "custom".to_string(),
        AgentRoleConfig {
            description: None,
            config_file: Some(role_path),
            nickname_candidates: None,
        },
    );
    let expected = config.model_providers["ollama"].clone();
    apply_role_to_config(&mut config, Some("custom"))
        .await
        .expect("role should apply");
    assert_eq!(config.model_provider, expected);
    assert_eq!(config.model_provider_id, "ollama");
}

#[tokio::test]
async fn unknown_role_provider_is_rejected_without_mutating_config() {
    let (home, mut config) = test_config_with_cli_overrides(Vec::new()).await;
    let role_path = write_role_config(
        &home,
        "provider.toml",
        "model_provider = 'missing-provider'",
    )
    .await;
    config.agent_roles.insert(
        "custom".to_string(),
        AgentRoleConfig {
            description: None,
            config_file: Some(role_path),
            nickname_candidates: None,
        },
    );
    let before = config.clone();
    assert!(
        apply_role_to_config(&mut config, Some("custom"))
            .await
            .is_err()
    );
    assert_eq!(config, before);
}
