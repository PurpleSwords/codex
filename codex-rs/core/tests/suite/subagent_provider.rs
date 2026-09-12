use anyhow::Context;
use anyhow::Result;
use codex_core::config::AgentRoleConfig;
use codex_features::Feature;
use codex_protocol::ThreadId;
use codex_protocol::protocol::EventMsg;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call_with_namespace;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::time::Duration;
use test_case::test_case;
use tokio::time::sleep;
use tokio::time::timeout;

#[test_case("multi_agent_v1"; "v1")]
#[test_case("collaboration"; "v2")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn child_uses_role_provider_without_redirecting_parent(namespace: &str) -> Result<()> {
    let parent_server = start_mock_server().await;
    let child_server = start_mock_server().await;
    let spawn_args = if namespace == "collaboration" {
        json!({"message": "check the child service", "task_name": "worker", "agent_type": "custom", "fork_turns": "none"})
    } else {
        json!({"message": "check the child service", "agent_type": "custom"})
    };
    let parent_requests = mount_sse_sequence(
        &parent_server,
        vec![
            sse(vec![
                ev_response_created("parent-spawn"),
                ev_function_call_with_namespace(
                    "spawn-child",
                    namespace,
                    "spawn_agent",
                    &spawn_args.to_string(),
                ),
                ev_completed("parent-spawn"),
            ]),
            sse(vec![
                ev_assistant_message("parent-answer", "Delegated."),
                ev_completed("parent-done"),
            ]),
        ],
    )
    .await;
    let child_requests = mount_sse_once(
        &child_server,
        sse(vec![
            ev_assistant_message("child-answer", "Child service reached."),
            ev_completed("child-done"),
        ]),
    )
    .await;
    let child_url = format!("{}/v1", child_server.uri());
    let role_config = format!(
        r#"
model = "gpt-5.6-sol"
model_provider = "child-service"
[model_providers.child-service]
name = "child service"
base_url = "{child_url}"
wire_api = "responses"
requires_openai_auth = false
[model_providers.child-service.http_headers]
x-role-provider = "child"
"#
    );
    let use_v2 = namespace == "collaboration";
    // Disabling the V2 feature does not override the model's V2 metadata.
    let parent_model = if use_v2 { "gpt-5.6-terra" } else { "gpt-5.2" };
    let test = test_codex()
        .with_config(move |config| {
            config
                .features
                .enable(Feature::Collab)
                .expect("enable multi-agent");
            if use_v2 {
                config
                    .features
                    .enable(Feature::MultiAgentV2)
                    .expect("enable v2");
            } else {
                config
                    .features
                    .disable(Feature::MultiAgentV2)
                    .expect("disable v2");
            }
            config.model = Some(parent_model.to_string());
            let role_path = config.codex_home.join("custom-role.toml");
            std::fs::write(&role_path, &role_config).expect("write role config");
            config.agent_roles.insert(
                "custom".to_string(),
                AgentRoleConfig {
                    description: None,
                    config_file: Some(role_path.to_path_buf()),
                    nickname_candidates: None,
                },
            );
        })
        .build_with_auto_env(&parent_server)
        .await?;
    let parent_provider = test.codex.config().await.model_provider.clone();
    // Keep the configured permissions: submit_turn overrides them for this turn.
    test.submit_text_turn("Delegate the check to the custom agent.")
        .await?;
    let child_request = timeout(Duration::from_secs(/*secs*/ 10), async {
        loop {
            if let Some(request) = child_requests.requests().into_iter().next() {
                break request;
            }
            sleep(Duration::from_millis(/*millis*/ 10)).await;
        }
    })
    .await
    .context("child did not reach its own provider")?;
    let body = child_request.body_json();
    let child_id = ThreadId::from_string(
        body["client_metadata"]["thread_id"]
            .as_str()
            .context("child thread id")?,
    )?;
    let child = test.thread_manager.get_thread(child_id).await?;
    wait_for_event(&child, |event| matches!(event, EventMsg::TurnComplete(_))).await;
    assert_eq!(body["model"], json!("gpt-5.6-sol"));
    assert_eq!(
        child_request.header("x-role-provider").as_deref(),
        Some("child")
    );
    assert_eq!(child_request.header("authorization"), None);
    assert_eq!(
        child.config().await.model_provider.base_url.as_deref(),
        Some(child_url.as_str())
    );
    assert_eq!(test.codex.config().await.model_provider, parent_provider);
    assert_eq!(
        child.config().await.permissions,
        test.codex.config().await.permissions
    );
    assert_eq!(
        parent_requests
            .requests()
            .iter()
            .map(|request| request.body_json()["model"].clone())
            .collect::<Vec<_>>(),
        vec![json!(parent_model); 2]
    );
    child.shutdown_and_wait().await?;
    assert_eq!(child_requests.requests().len(), 1);
    Ok(())
}
