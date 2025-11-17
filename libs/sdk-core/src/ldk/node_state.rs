use ldk_node::bitcoin::secp256k1::PublicKey;
use ldk_node::{Node, PendingSweepBalance};

use crate::ldk::utils::Hex;
use crate::node_api::NodeError;
use crate::{LnPaymentDetails, NodeState, Payment, PaymentDetails, PaymentStatus, PaymentType};

impl From<&Node> for NodeState {
    fn from(node: &Node) -> Self {
        const MAX_PAYMENT_AMOUNT_MSAT: u64 = 4_294_967_000;

        let balances = node.list_balances();
        let pending_onchain_balance_sats: u64 = balances
            .pending_balances_from_channel_closures
            .iter()
            .map(get_balance)
            .sum();

        let connected_peers = node
            .list_peers()
            .iter()
            .filter(|p| p.is_connected)
            .map(|p| p.node_id.to_string())
            .collect();

        let channels = node.list_channels();
        let max_payable_msat = channels
            .iter()
            .map(|c| c.next_outbound_htlc_limit_msat)
            .sum();
        let max_chan_reserve_sats: u64 = channels
            .iter()
            .flat_map(|c| c.unspendable_punishment_reserve)
            .sum();
        let inbound_capacity_msats = channels.iter().map(|c| c.inbound_capacity_msat).sum();

        Self {
            id: node.node_id().to_string(),
            block_height: node.status().current_best_block.height,
            channels_balance_msat: balances.total_lightning_balance_sats * 1000,
            onchain_balance_msat: balances.total_onchain_balance_sats * 1000,
            pending_onchain_balance_msat: pending_onchain_balance_sats * 1000,
            utxos: Vec::new(), // Not available in LDK Node.
            max_payable_msat,
            max_receivable_msat: MAX_PAYMENT_AMOUNT_MSAT,
            max_single_payment_amount_msat: MAX_PAYMENT_AMOUNT_MSAT,
            max_chan_reserve_msats: max_chan_reserve_sats * 1000,
            connected_peers,
            // TODO: Calculate a better approximation.
            max_receivable_single_payment_amount_msat: inbound_capacity_msats,
            total_inbound_liquidity_msats: inbound_capacity_msats,
        }
    }
}

pub fn convert_payment(
    payment: ldk_node::payment::PaymentDetails,
    local_node_id: PublicKey,
) -> Result<Payment, NodeError> {
    let lsp_fee_msat = match payment.kind {
        ldk_node::payment::PaymentKind::Bolt11Jit {
            counterparty_skimmed_fee_msat: Some(lsp_fee_msat),
            ..
        } => lsp_fee_msat,
        _ => 0,
    };
    let details = to_payment_details(&payment, local_node_id)?;
    Ok(Payment {
        id: payment.id.to_hex(),
        payment_type: payment.direction.into(),
        payment_time: payment.latest_update_timestamp as i64,
        amount_msat: payment.amount_msat.unwrap_or_default(),
        fee_msat: payment.fee_paid_msat.unwrap_or(lsp_fee_msat),
        status: payment.status.into(),
        error: None,
        description: None, // TODO: Get it from bolt11.
        details,
        metadata: None,
    })
}

fn to_payment_details(
    payment: &ldk_node::payment::PaymentDetails,
    local_node_id: PublicKey,
) -> Result<PaymentDetails, NodeError> {
    let destination_pubkey = match payment.direction {
        ldk_node::payment::PaymentDirection::Inbound => local_node_id.to_string(),
        ldk_node::payment::PaymentDirection::Outbound => String::new(), // TODO: Get it from bolt11.
    };
    match &payment.kind {
        ldk_node::payment::PaymentKind::Bolt11 { hash, preimage, .. } => Ok(PaymentDetails::Ln {
            data: ln_payment_details(hash, preimage, destination_pubkey, false),
        }),
        ldk_node::payment::PaymentKind::Bolt11Jit { hash, preimage, .. } => {
            Ok(PaymentDetails::Ln {
                data: ln_payment_details(hash, preimage, destination_pubkey, false),
            })
        }
        ldk_node::payment::PaymentKind::Spontaneous { hash, preimage } => Ok(PaymentDetails::Ln {
            data: ln_payment_details(hash, preimage, destination_pubkey, true),
        }),
        other => Err(NodeError::Generic(format!(
            "Unsupported payment kind: {other:?}"
        ))),
    }
}

fn ln_payment_details(
    hash: &ldk_node::lightning_types::payment::PaymentHash,
    preimage: &Option<ldk_node::lightning_types::payment::PaymentPreimage>,
    destination_pubkey: String,
    keysend: bool,
) -> LnPaymentDetails {
    LnPaymentDetails {
        payment_hash: hash.to_hex(),
        destination_pubkey,
        payment_preimage: preimage.as_ref().map(Hex::to_hex).unwrap_or_default(),
        keysend,
        bolt11: String::new(),     // TODO: Put it.
        open_channel_bolt11: None, // TODO: What should we put here?
        ..Default::default()
    }
}

impl From<ldk_node::payment::PaymentStatus> for PaymentStatus {
    fn from(status: ldk_node::payment::PaymentStatus) -> Self {
        match status {
            ldk_node::payment::PaymentStatus::Pending => PaymentStatus::Pending,
            ldk_node::payment::PaymentStatus::Succeeded => PaymentStatus::Complete,
            ldk_node::payment::PaymentStatus::Failed => PaymentStatus::Failed,
        }
    }
}

impl From<ldk_node::payment::PaymentDirection> for PaymentType {
    fn from(direction: ldk_node::payment::PaymentDirection) -> Self {
        match direction {
            ldk_node::payment::PaymentDirection::Inbound => PaymentType::Received,
            ldk_node::payment::PaymentDirection::Outbound => PaymentType::Sent,
        }
    }
}

fn get_balance(balance: &PendingSweepBalance) -> u64 {
    match balance {
        PendingSweepBalance::PendingBroadcast {
            amount_satoshis, ..
        }
        | PendingSweepBalance::BroadcastAwaitingConfirmation {
            amount_satoshis, ..
        }
        | PendingSweepBalance::AwaitingThresholdConfirmations {
            amount_satoshis, ..
        } => *amount_satoshis,
    }
}
