use super::OutputHandles;
use super::UnifiedExecProcess;
use crate::unified_exec::UNIFIED_EXEC_OUTPUT_MAX_BYTES;
use crate::unified_exec::head_tail_buffer::HeadTailBuffer;
use pretty_assertions::assert_eq;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tokio::sync::Mutex;
use tokio::sync::Notify;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn transcript_preserves_output_before_subscription_and_after_lag() {
    let output = OutputHandles {
        output_buffer: Arc::new(Mutex::new(HeadTailBuffer::default())),
        output_notify: Arc::new(Notify::new()),
        output_closed: Arc::new(AtomicBool::new(false)),
        output_closed_notify: Arc::new(Notify::new()),
        cancellation_token: CancellationToken::new(),
    };
    let transcript = Arc::new(Mutex::new(HeadTailBuffer::default()));
    let (source_tx, source_rx) = broadcast::channel(/*capacity*/ 8);
    let (output_tx, _) = broadcast::channel(/*capacity*/ 1);
    let task = UnifiedExecProcess::spawn_local_output_task(
        source_rx,
        output.clone(),
        output_tx.clone(),
        Arc::clone(&transcript),
    );

    // Finish producing the head before subscribing, without relying on sleeps.
    let produced = output.output_notify.notified();
    tokio::pin!(produced);
    produced.as_mut().enable();
    let head = b"HEAD\n\xff\xff\xff\xff\xff\xff\xf0\x9f\x98\x80\xff\xff\xff\xc3\xa9";
    source_tx.send(head.to_vec()).unwrap();
    produced.await;
    let mut receiver = output_tx.subscribe();

    // Polling is destructive, but must not consume the final event's transcript.
    *output.output_buffer.lock().await = HeadTailBuffer::default();
    let middle = vec![b'x'; UNIFIED_EXEC_OUTPUT_MAX_BYTES * 2];
    source_tx.send(middle.clone()).unwrap();
    let tail = b"\xfe\xfe\nTAIL";
    source_tx.send(tail.to_vec()).unwrap();
    drop(source_tx);
    task.await.unwrap();

    assert!(matches!(
        receiver.try_recv(),
        Err(broadcast::error::TryRecvError::Lagged(_))
    ));
    assert_eq!(receiver.try_recv().unwrap(), tail);
    let mut expected = HeadTailBuffer::<UNIFIED_EXEC_OUTPUT_MAX_BYTES>::default();
    expected.push_chunk(head);
    expected.push_chunk(&middle);
    expected.push_chunk(tail);
    assert_eq!(
        transcript.lock().await.to_bytes_with_omission_marker(),
        expected.to_bytes_with_omission_marker()
    );
}
