use std::sync::Arc;

use ldk_node::lightning::events::PaymentFailureReason;
use ldk_node::lightning::ln::channelmanager::PaymentId;
use ldk_node::payment::PaymentDetails;
use ldk_node::{Event, Node};
use tokio::sync::{broadcast, mpsc};
use tokio::time::error::Elapsed;
use tokio::time::{timeout, Duration};

use crate::ldk::store::Store;
use crate::node_api::{IncomingPayment, NodeError, NodeResult};

pub async fn start_event_handling(
    node: Arc<Node>,
    events_tx: broadcast::Sender<Event>,
    store: Store,
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
                match store.load_preimage(&payment_hash) {
                    Ok(preimage) => {
                        let bolt11 = match store.load_bolt11(&payment_hash) {
                            Ok(bolt11) => bolt11,
                            Err(err) => {
                                error!("Failed to read bolt11 for payment with id={payment_id:?}: {err}");
                                None
                            }
                        };
                        let payment = IncomingPayment {
                            payment_hash: payment_hash.0.to_vec(),
                            preimage: preimage.0.to_vec(),
                            amount_msat,
                            bolt11: bolt11.unwrap_or_default(),
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
                match store.load_preimage(&payment_hash) {
                    Ok(preimage) => {
                        if let Err(e) = node.bolt11_payment().claim_for_hash(
                            payment_hash,
                            claimable_amount_msat,
                            preimage,
                        ) {
                            error!("Failed to claim payment: {e}");
                        }
                    }
                    Err(err) => {
                        error!("Failed to read preimage when payment claimable for payment with id={payment_id:?}: {err}");
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
