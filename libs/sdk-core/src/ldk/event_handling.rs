use core::convert::TryInto;
use std::sync::Arc;

use ldk_node::lightning_types::payment::PaymentPreimage;
use ldk_node::{Event, Node};
use tokio::sync::{broadcast, mpsc};

use crate::ldk::node_api::{
    preimage_store_key, KVStore, PREIMAGES_PRIMARY_NS, PREIMAGES_SECONDARY_NS,
};
use crate::node_api::IncomingPayment;

pub async fn start_event_handling(
    node: Arc<Node>,
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

        match event {
            Event::PaymentReceived {
                payment_id,
                payment_hash,
                amount_msat,
                ..
            } => {
                let key = preimage_store_key(&payment_hash);
                match kv_store.read(PREIMAGES_PRIMARY_NS, PREIMAGES_SECONDARY_NS, &key) {
                    Ok(preimage) => {
                        if let Err(err) = kv_store.remove(
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
                let preimage = match kv_store.read(
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
        }

        if let Err(e) = node.event_handled() {
            error!("Failed to report that event was handled: {e}");
        }
    }
}
