use super::NeverEndingTask;
use super::attach_thread_persistence;
use super::make_session_and_context_with_rx;
use super::raw_history_items;
use super::user_message;
use super::wait_for_thread_rollback_failed;
use super::wait_for_thread_rolled_back;
use crate::session::handlers;
use crate::state::ActiveTurn;
use crate::state::TaskKind;
use codex_history::ResponseItemEnvelope;
use codex_history::RolloutItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::TurnAbortReason;
use pretty_assertions::assert_eq;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn rollback_waits_for_terminal_event_cleanup() {
    let (mut session, turn, events) = make_session_and_context_with_rx().await;
    attach_thread_persistence(Arc::get_mut(&mut session).unwrap()).await;
    let history = vec![user_message("remove this turn")];
    session
        .replace_history(history.clone(), Some(turn.to_turn_context_item()))
        .await;
    let rollout: Vec<_> = history
        .into_iter()
        .map(ResponseItemEnvelope::new)
        .map(RolloutItem::ResponseItem)
        .collect();
    session.persist_rollout_items(&rollout).await;

    let completion = CancellationToken::new();
    *session.active_turn.lock().await = Some(ActiveTurn {
        completion: Some(completion.clone()),
        ..Default::default()
    });
    let mut rollback = Box::pin(handlers::thread_rollback(
        &session,
        "rollback".to_string(),
        /*num_turns*/ 1,
    ));
    // Force the window after TurnComplete but before active-turn cleanup.
    // No sleeps or timing assumptions are needed to hold the finalizer here.
    assert!(futures::poll!(&mut rollback).is_pending());
    assert!(events.try_recv().is_err());
    *session.active_turn.lock().await = None;
    completion.cancel();
    rollback.await;

    assert_eq!(wait_for_thread_rolled_back(&events).await.num_turns, 1);
    assert_eq!(
        raw_history_items(&session.clone_history().await),
        Vec::<ResponseItem>::new()
    );
}

#[tokio::test]
async fn rollback_rechecks_a_new_turn_after_finalization() {
    let (session, _turn, events) = make_session_and_context_with_rx().await;
    let completion = CancellationToken::new();
    *session.active_turn.lock().await = Some(ActiveTurn {
        completion: Some(completion.clone()),
        ..Default::default()
    });
    let mut rollback = Box::pin(handlers::thread_rollback(
        &session,
        "rollback".to_string(),
        /*num_turns*/ 1,
    ));
    assert!(futures::poll!(&mut rollback).is_pending());
    // Finishing the old turn does not authorize rolling back a newly reserved one.
    *session.active_turn.lock().await = Some(ActiveTurn::default());
    completion.cancel();
    rollback.await;
    let error = wait_for_thread_rollback_failed(&events).await;
    assert_eq!(
        error.message,
        "Cannot rollback while a turn is in progress."
    );
}

#[tokio::test]
async fn rollback_still_rejects_a_running_task() {
    let (session, turn, events) = make_session_and_context_with_rx().await;
    session
        .spawn_task(
            turn,
            Vec::new(),
            NeverEndingTask {
                kind: TaskKind::Regular,
                listen_to_cancellation_token: true,
            },
        )
        .await;
    handlers::thread_rollback(&session, "rollback".to_string(), /*num_turns*/ 1).await;
    let error = wait_for_thread_rollback_failed(&events).await;
    session.abort_all_tasks(TurnAbortReason::Interrupted).await;
    assert_eq!(
        error.message,
        "Cannot rollback while a turn is in progress."
    );
}
