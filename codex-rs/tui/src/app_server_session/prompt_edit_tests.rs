use super::super::ResumeModelSettings;
use crate::legacy_core::config::ConfigBuilder;
use codex_app_server_protocol::ThreadHistoryMode;
use codex_protocol::ThreadId;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::TurnCompleteEvent;
use codex_protocol::protocol::TurnStartedEvent;
use codex_protocol::protocol::UserMessageEvent;
use codex_rollout::RolloutItem;
use color_eyre::Result;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn prompt_edit_preserves_thread_identity_and_rejects_stale_targets() -> Result<()> {
    for history_mode in [ThreadHistoryMode::Legacy, ThreadHistoryMode::Paginated] {
        let home = tempfile::tempdir()?;
        let config = ConfigBuilder::default()
            .codex_home(home.path().to_path_buf())
            .build()
            .await?;
        let create = match history_mode {
            ThreadHistoryMode::Legacy => app_test_support::create_fake_rollout,
            ThreadHistoryMode::Paginated => app_test_support::create_fake_paginated_rollout,
        };
        let thread_id = ThreadId::from_string(
            &create(
                home.path(),
                "2025-01-05T12-00-00",
                "2025-01-05T12:00:00Z",
                "edit this prompt",
                Some(config.model_provider_id.as_str()),
                /*git_info*/ None,
            )
            .expect("create saved prompt"),
        )?;
        // The picker fixture contains only a preview message. Paginated history
        // needs explicit persisted turn boundaries to expose a revert target.
        let path = app_test_support::rollout_path(
            home.path(),
            "2025-01-05T12-00-00",
            &thread_id.to_string(),
        );
        let contents = std::fs::read_to_string(&path)?;
        let metadata = contents.lines().next().expect("session metadata");
        std::fs::write(&path, format!("{metadata}\n"))?;
        let turn_id = uuid::Uuid::now_v7().to_string();
        for event in [
            EventMsg::TurnStarted(TurnStartedEvent {
                turn_id: turn_id.clone(),
                trace_id: None,
                started_at: None,
                model_context_window: None,
                collaboration_mode_kind: Default::default(),
            }),
            EventMsg::UserMessage(UserMessageEvent {
                message: "edit this prompt".to_string(),
                ..Default::default()
            }),
            EventMsg::TurnComplete(TurnCompleteEvent {
                turn_id,
                last_agent_message: None,
                error: None,
                started_at: None,
                completed_at: None,
                duration_ms: None,
                time_to_first_token_ms: None,
            }),
        ] {
            codex_rollout::append_rollout_item_to_path(&path, &RolloutItem::EventMsg(event))
                .await?;
        }
        let mut server = crate::start_embedded_app_server_for_picker(&config).await?;
        let started = server
            .resume_thread(
                config.clone(),
                thread_id,
                ResumeModelSettings::PreserveExistingThread,
            )
            .await?;
        assert_eq!(
            server
                .thread_read(thread_id, /*include_turns*/ false)
                .await?
                .history_mode,
            history_mode
        );
        let before_turn_id = started.turns.first().expect("saved prompt turn").id.clone();
        assert!(
            server
                .revert_before_prompt(thread_id, "missing-turn".to_string())
                .await
                .is_err()
        );
        let unchanged = server
            .resume_thread(
                config.clone(),
                thread_id,
                ResumeModelSettings::PreserveExistingThread,
            )
            .await?;
        assert_eq!(unchanged.turns, started.turns);

        server
            .revert_before_prompt(thread_id, before_turn_id)
            .await?;
        let resumed = server
            .resume_thread(
                config,
                thread_id,
                ResumeModelSettings::PreserveExistingThread,
            )
            .await?;
        assert_eq!(resumed.session.thread_id, thread_id);
        assert_eq!(resumed.turns, Vec::new());
        server.shutdown().await?;
    }
    Ok(())
}
