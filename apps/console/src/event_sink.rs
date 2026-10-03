//! Channel-backed RuntimeEventSink for Console.
//!
//! Runtime emits events → ChannelSink pushes into mpsc → app event loop drains.
//! This is Console's equivalent of Desktop's TauriEventSink.

use commandui_runtime_core::events::{RuntimeEvent, RuntimeEventSink};
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;

pub struct ChannelSink {
    tx: UnboundedSender<RuntimeEvent>,
}

impl ChannelSink {
    pub fn new(tx: UnboundedSender<RuntimeEvent>) -> Self {
        Self { tx }
    }
}

impl RuntimeEventSink for ChannelSink {
    fn emit(&self, event: RuntimeEvent) {
        let _ = self.tx.send(event);
    }
}

pub fn shared_channel_sink(tx: UnboundedSender<RuntimeEvent>) -> Arc<dyn RuntimeEventSink> {
    Arc::new(ChannelSink::new(tx))
}

#[cfg(test)]
mod tests {
    use super::*;
    use commandui_runtime_core::events::{RuntimeEvent, SessionReadyEvent};

    #[test]
    fn emit_delivers_and_a_dropped_receiver_is_ignored() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let sink = shared_channel_sink(tx);
        sink.emit(RuntimeEvent::SessionReady(SessionReadyEvent {
            session_id: "s1".into(),
            cwd: "/work".into(),
        }));
        match rx.try_recv().unwrap() {
            RuntimeEvent::SessionReady(event) => {
                assert_eq!(event.session_id, "s1");
                assert_eq!(event.cwd, "/work");
            }
            _ => panic!("expected a session-ready event"),
        }
        drop(rx);
        sink.emit(RuntimeEvent::SessionReady(SessionReadyEvent {
            session_id: "s1".into(),
            cwd: "/work".into(),
        }));
    }
}
