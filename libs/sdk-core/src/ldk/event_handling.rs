use core::convert::TryInto;
use std::sync::Arc;

use ldk_node::lightning::events::PaymentFailureReason;
use ldk_node::lightning::ln::channelmanager::PaymentId;
use ldk_node::lightning::util::persist::KVStoreSync;
use ldk_node::lightning_types::payment::PaymentPreimage;
use ldk_node::payment::PaymentDetails;
use ldk_node::{Event, Node};
use tokio::sync::{broadcast, mpsc};
use tokio::time::error::Elapsed;
use tokio::time::{timeout, Duration};

use crate::ldk::node_api::{
    preimage_store_key, KVStore, PREIMAGES_PRIMARY_NS, PREIMAGES_SECONDARY_NS,
};
use crate::node_api::{IncomingPayment, NodeError, NodeResult};

pub async fn start_event_handling(
    node: Arc<Node>,
    events_tx: broadcast::Sender<Event>,
    kv_store: KVStore,
    incoming_payments_tx: broadcast::Sender<IncomingPayment>,
    mut shutdown: mpsc::Receiver<()>,
) {
    loop {
        let event = tokio::select! {
            event = node.next_event_async() => event,
            _ = shutdown.recv() => {
                info!("Received shutdown signal, stopping event handling loop");
                return;
            },
        };
        debug!("Event: {event:?}");
        let _ = events_tx.send(event.clone()); // Error here will mean that there are no subscribers.

        match event {
            Event::PaymentReceived {
                payment_id,
                payment_hash,
                amount_msat,
                ..
            } => {
                let key = preimage_store_key(&payment_hash);
                match KVStoreSync::read(
                    kv_store.as_ref(),
                    PREIMAGES_PRIMARY_NS,
                    PREIMAGES_SECONDARY_NS,
                    &key,
                ) {
                    Ok(preimage) => {
                        if let Err(err) = KVStoreSync::remove(
                            kv_store.as_ref(),
                            PREIMAGES_PRIMARY_NS,
                            PREIMAGES_SECONDARY_NS,
                            &key,
                            false,
                        ) {
                            warn!(
								"Failed to remove preimage from store for payment with id={payment_id:?}: {err}"
							);
                        }
                        // TODO: Load bolt11 from the store.
                        let bolt11 = String::new();
                        let payment = IncomingPayment {
                            payment_hash: payment_hash.0.to_vec(),
                            preimage,
                            amount_msat,
                            bolt11,
                        };
                        if let Err(e) = incoming_payments_tx.send(payment) {
                            warn!("Failed to send payment to incoming_payments_tx: {e}");
                        }
                    }
                    Err(err) => {
                        error!(
                            "Payment received but failed to read preimage for payment with id={payment_id:?}: {err}"
                        );
                    }
                }
            }
            Event::PaymentSuccessful { .. } => (),
            Event::PaymentFailed { .. } => (),
            Event::PaymentClaimable {
                payment_id,
                payment_hash,
                claimable_amount_msat,
                ..
            } => {
                let key = preimage_store_key(&payment_hash);
                let preimage = match KVStoreSync::read(
                    kv_store.as_ref(),
                    PREIMAGES_PRIMARY_NS,
                    PREIMAGES_SECONDARY_NS,
                    &key,
                ) {
                    Ok(preimage) => match preimage.as_slice().try_into() {
                        Ok(preimage_arr) => Some(PaymentPreimage(preimage_arr)),
                        Err(err) => {
                            error!("Failed to convert preimage for payment with id={payment_id:?}: {err}");
                            None
                        }
                    },
                    Err(err) => {
                        error!("Failed to read preimage when payment claimable for payment with id={payment_id:?}: {err}");
                        None
                    }
                };
                match preimage {
                    Some(preimage) => {
                        if let Err(e) = node.bolt11_payment().claim_for_hash(
                            payment_hash,
                            claimable_amount_msat,
                            preimage,
                        ) {
                            error!("Failed to claim payment: {e}");
                        }
                    }
                    None => {
                        if let Err(e) = node.bolt11_payment().fail_for_hash(payment_hash) {
                            error!("Failed to fail payment: {e}");
                        }
                    }
                };
            }
            Event::PaymentForwarded { .. } => (),
            Event::ChannelPending { .. } => (),
            Event::ChannelReady { .. } => (),
            Event::ChannelClosed { .. } => (),

            Event::SplicePending { .. } => (),
            Event::SpliceFailed { .. } => (),
        }

        if let Err(e) = node.event_handled() {
            error!("Failed to report that event was handled: {e}");
        }
    }
}

pub async fn wait_for_payment_success(
    node: &Node,
    mut events_rx: broadcast::Receiver<Event>,
    p_id: PaymentId,
) -> NodeResult<PaymentDetails> {
    debug!("Waiting for payment success id:{p_id}");
    timeout(Duration::from_secs(30), async {
        while let Ok(event) = events_rx.recv().await {
            match event {
                Event::PaymentSuccessful { payment_id, .. } if payment_id == Some(p_id) => {
                    return node
                        .list_payments_with_filter(|p| p.id == p_id)
                        .into_iter()
                        .next()
                        .ok_or(NodeError::generic("Failed to find payment we just sent"));
                }
                Event::PaymentFailed {
                    payment_id, reason, ..
                } if payment_id == Some(p_id) => {
                    let reason = reason.unwrap_or(PaymentFailureReason::UnexpectedError);
                    return Err(NodeError::PaymentFailed(format!("{reason:?}")));
                }
                _ => continue,
            }
        }
        Err(NodeError::generic("Node is shutting down"))
    })
    .await
    .map_err(|_elapsed: Elapsed| {
        NodeError::PaymentFailed("Timeout waiting for payment success".to_string())
    })?
}
