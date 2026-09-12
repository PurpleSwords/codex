//! In-place prompt editing without creating a new thread.

use super::AppServerSession;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ThreadHistoryMode;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadRevertParams;
use codex_app_server_protocol::ThreadRevertResponse;
use codex_app_server_protocol::ThreadRollbackParams;
use codex_app_server_protocol::ThreadRollbackResponse;
use codex_app_server_protocol::ThreadStatus;
use codex_app_server_protocol::TurnStatus;
use codex_protocol::ThreadId;
use color_eyre::Result;
use color_eyre::eyre::Context;
use color_eyre::eyre::bail;

impl AppServerSession {
    pub(crate) async fn revert_before_prompt(
        &mut self,
        thread_id: ThreadId,
        before_turn_id: String,
    ) -> Result<()> {
        let thread = self.thread_read(thread_id, /*include_turns*/ false).await?;
        if matches!(thread.status, ThreadStatus::Active { .. }) {
            bail!("wait for the active turn to finish before editing an earlier prompt");
        }
        match thread.history_mode {
            ThreadHistoryMode::Paginated => {
                let request_id = self.next_request_id();
                let _: ThreadRevertResponse = self
                    .client
                    .request_typed(ClientRequest::ThreadRevert {
                        request_id,
                        params: ThreadRevertParams {
                            thread_id: thread_id.to_string(),
                            before_turn_id,
                        },
                    })
                    .await
                    .wrap_err("failed to revert before the selected prompt")?;
            }
            ThreadHistoryMode::Legacy => {
                // Count authoritative persisted turns, not visible transcript cells. Review
                // projection and steering can make those two counts different.
                let thread = self.thread_read(thread_id, /*include_turns*/ true).await?;
                let index = thread
                    .turns
                    .iter()
                    .position(|turn| turn.id == before_turn_id)
                    .ok_or_else(|| {
                        color_eyre::eyre::eyre!("the selected prompt is no longer available")
                    })?;
                let removed = &thread.turns[index..];
                // Legacy replay has multiple notions of user boundaries. Do not risk trimming
                // a different prefix when a turn has no user input or contains steers.
                if removed.iter().any(|turn| {
                    turn.status == TurnStatus::InProgress
                        || turn
                            .items
                            .iter()
                            .filter(|item| matches!(item, ThreadItem::UserMessage { .. }))
                            .count()
                            != 1
                }) {
                    bail!(
                        "this legacy history cannot be safely edited in place: pending, steered, or non-user turns follow the selected prompt"
                    );
                }
                let num_turns = u32::try_from(removed.len())?;
                let request_id = self.next_request_id();
                let _: ThreadRollbackResponse = self
                    .client
                    .request_typed(ClientRequest::ThreadRollback {
                        request_id,
                        params: ThreadRollbackParams {
                            thread_id: thread_id.to_string(),
                            num_turns,
                        },
                    })
                    .await
                    .wrap_err("failed to roll back before the selected prompt")?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "prompt_edit_tests.rs"]
mod tests;
