use breez_sdk_core::{BreezEvent, EventListener};
use tokio::runtime::Handle;
use tokio::sync::mpsc;
use tracing::info;

pub struct EventListenerImpl {
    tx: mpsc::Sender<BreezEvent>,
    handle: Handle,
}

impl EventListenerImpl {
    pub fn new(tx: mpsc::Sender<BreezEvent>) -> Self {
        Self {
            tx,
            handle: Handle::current(),
        }
    }
}

impl EventListener for EventListenerImpl {
    fn on_event(&self, e: BreezEvent) {
        info!("Event: {e:?}");
        let _ = tokio::task::block_in_place(|| self.handle.block_on(self.tx.send(e)));
    }
}
