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
