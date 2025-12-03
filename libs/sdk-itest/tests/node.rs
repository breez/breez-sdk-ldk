mod event_listener;

use std::time::Duration;

use bitcoin::Amount;
use breez_sdk_core::error::ConnectError;
use breez_sdk_core::{
    BreezEvent, BreezServices, ChannelState, ClosedChannelPaymentDetails, Config, ConnectRequest,
    LnPaymentDetails, PaymentDetails, PaymentType, ReceivePaymentRequest,
    RedeemOnchainFundsRequest, SendPaymentRequest, SendSpontaneousPaymentRequest,
};
use rand::Rng;
use rstest::*;
use sdk_itest::environment::Environment;
use sdk_itest::wait_for;
use testdir::testdir;
use tokio::sync::mpsc;
use tokio::time::sleep;
use tokio::try_join;
use tracing::info;

use crate::event_listener::EventListenerImpl;

const SECOND: Duration = Duration::from_secs(1);

#[rstest]
#[tokio::test(flavor = "multi_thread")]
#[test_log::test]
async fn test_node_receive_payments() {
    let env = Environment::default();
    let (bitcoind, esplora, mempool, vss, lsp, lnd, rgs) = try_join!(
        env.bitcoind(),
        env.esplora_api(),
        env.mempool_api(),
        env.vss_api(),
        env.lsp_external_address(),
        env.lnd_with_channel(),
        env.rgs()
    )
    .unwrap();
    info!("Esplora is running: {}", esplora.external_endpoint());
    info!("Mempool is running: {}", mempool.external_endpoint());
    info!("    VSS is running: {}", vss.external_endpoint());
    info!("    LSP is running: {lsp}");
    info!("    LND is running");
    info!("    RGS is running: {}", rgs.external_endpoint());

    let mut config = Config::regtest(String::new());
    config.working_dir = testdir!().to_string_lossy().to_string();
    config.mempoolspace_url = Some(mempool.external_endpoint());
    config.esplora_url = esplora.external_endpoint();
    config.vss_url = vss.external_endpoint();
    config.rgs_url = rgs.external_endpoint();
    config.lsps2_address = lsp;

    let seed = rand::rng().random::<[u8; 64]>().to_vec();
    {
        info!("Starting a fresh node with restore_only=Some(true)");
        let req = ConnectRequest {
            config: config.clone(),
            seed: seed.clone(),
            restore_only: Some(true),
        };

        let (tx, _) = mpsc::channel(100);
        let services = BreezServices::connect(req, Box::new(EventListenerImpl::new(tx))).await;
        assert!(matches!(services, Err(ConnectError::RestoreOnly { .. })));
    }

    info!("Starting a fresh node with restore_only=None");
    let req = ConnectRequest {
        config: config.clone(),
        seed: seed.clone(),
        restore_only: None,
    };

    let (tx, mut events) = mpsc::channel(100);

    let services = BreezServices::connect(req, Box::new(EventListenerImpl::new(tx)))
        .await
        .unwrap();

    info!("Waiting for BreezEvent::Synced...");
    assert!(matches!(events.recv().await, Some(BreezEvent::Synced)));

    // Receiving a JIT payment.
    let huge_amount_msat = 10_000_000;
    let response = services
        .receive_payment(ReceivePaymentRequest {
            amount_msat: huge_amount_msat,
            description: "Init".to_string(),
            ..Default::default()
        })
        .await
        .unwrap();
    let opening_fee_msat = response.opening_fee_msat.unwrap_or_default();
    assert_eq!(opening_fee_msat, 1_000_000);
    let bolt11 = response.ln_invoice.bolt11;
    info!("Invoice created: {bolt11}");

    lnd.pay(bolt11).await.unwrap();
    info!("Waiting for BreezEvent::InvoicePaid...");
    wait_for!(matches!(
        events.recv().await,
        Some(BreezEvent::InvoicePaid { .. })
    ));
    let balance_msat = services.node_info().unwrap().channels_balance_msat;
    assert_eq!(balance_msat, huge_amount_msat - opening_fee_msat);
    let payments = services.list_payments(Default::default()).await.unwrap();
    assert_eq!(payments.len(), 1);
    let payment = payments.first().cloned().unwrap();
    assert_eq!(payment.amount_msat, huge_amount_msat - opening_fee_msat);
    assert_eq!(payment.fee_msat, opening_fee_msat);
    assert_eq!(payment.payment_type, PaymentType::Received);

    // Receiving a normal payment.
    let small_amount_msat = 10_000;
    let response = services
        .receive_payment(ReceivePaymentRequest {
            amount_msat: small_amount_msat,
            description: "small".to_string(),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(response.opening_fee_msat, None);
    let bolt11 = response.ln_invoice.bolt11;
    info!("Invoice created: {bolt11}");
    lnd.pay(bolt11).await.unwrap();
    info!("Waiting for BreezEvent::InvoicePaid...");
    wait_for!(matches!(
        events.recv().await,
        Some(BreezEvent::InvoicePaid { .. })
    ));
    info!("Waiting for BreezEvent::Synced...");
    assert!(matches!(events.recv().await, Some(BreezEvent::Synced)));
    let payments = services.list_payments(Default::default()).await.unwrap();
    assert_eq!(payments.len(), 2);
    let payment = payments.first().cloned().unwrap();
    assert_eq!(payment.amount_msat, small_amount_msat);
    assert_eq!(payment.fee_msat, 0);
    assert_eq!(payment.payment_type, PaymentType::Received);

    // Ensure that the next payment does not occur at the same time (down to the second).
    sleep(SECOND).await;

    // Paying BOLT-11 invoice.
    let amount = Amount::from_sat(1000);
    let bolt11 = lnd.receive(&amount).await.unwrap();
    let payment = services
        .send_payment(SendPaymentRequest {
            bolt11,
            amount_msat: None,
        })
        .await
        .unwrap()
        .payment;
    assert_eq!(payment.amount_msat, amount.to_msat());
    assert_eq!(payment.fee_msat, 1000);
    assert!(matches!(
        payment.details,
        PaymentDetails::Ln {
            data: LnPaymentDetails { keysend: false, .. }
        }
    ));
    info!("Waiting for BreezEvent::PaymentSucceed...");
    wait_for!(matches!(
        events.recv().await,
        Some(BreezEvent::PaymentSucceed { .. })
    ));
    let payments = services.list_payments(Default::default()).await.unwrap();
    assert_eq!(payments.len(), 3);
    let payment = payments.first().cloned().unwrap();
    assert_eq!(payment.payment_type, PaymentType::Sent);
    assert_eq!(payment.amount_msat, amount.to_msat());
    assert_eq!(payment.fee_msat, 1000);

    // Ensure that the next payment does not occur at the same time (down to the second).
    sleep(SECOND).await;

    // Paying open amount BOLT-11 invoice.
    let bolt11 = lnd.receive(&Amount::ZERO).await.unwrap();
    let amount = Amount::from_sat(1100);
    let payment = services
        .send_payment(SendPaymentRequest {
            bolt11,
            amount_msat: Some(amount.to_msat()),
        })
        .await
        .unwrap()
        .payment;
    assert_eq!(payment.amount_msat, amount.to_msat());
    assert_eq!(payment.fee_msat, 1000);
    assert!(matches!(
        payment.details,
        PaymentDetails::Ln {
            data: LnPaymentDetails { keysend: false, .. }
        }
    ));
    info!("Waiting for BreezEvent::PaymentSucceed...");
    wait_for!(matches!(
        events.recv().await,
        Some(BreezEvent::PaymentSucceed { .. })
    ));
    let payments = services.list_payments(Default::default()).await.unwrap();
    assert_eq!(payments.len(), 4);
    let payment = payments.first().cloned().unwrap();
    assert_eq!(payment.payment_type, PaymentType::Sent);
    assert_eq!(payment.amount_msat, amount.to_msat());
    assert_eq!(payment.fee_msat, 1000);

    // Ensure that the next payment does not occur at the same time (down to the second).
    sleep(SECOND).await;

    // Sending spontaneous payment.
    let lnd_id = lnd.get_id().await.unwrap();
    let amount = Amount::from_sat(1200);
    let payment = services
        .send_spontaneous_payment(SendSpontaneousPaymentRequest {
            node_id: lnd_id,
            amount_msat: amount.to_msat(),
            extra_tlvs: None,
        })
        .await
        .unwrap()
        .payment;
    assert_eq!(payment.amount_msat, amount.to_msat());
    assert_eq!(payment.fee_msat, 1000);
    assert!(matches!(
        payment.details,
        PaymentDetails::Ln {
            data: LnPaymentDetails { keysend: true, .. }
        }
    ));
    info!("Waiting for BreezEvent::PaymentSucceed...");
    wait_for!(matches!(
        events.recv().await,
        Some(BreezEvent::PaymentSucceed { .. })
    ));
    let payments = services.list_payments(Default::default()).await.unwrap();
    assert_eq!(payments.len(), 5);
    let payment = payments.first().cloned().unwrap();
    assert_eq!(payment.payment_type, PaymentType::Sent);
    assert_eq!(payment.amount_msat, amount.to_msat());
    assert_eq!(payment.fee_msat, 1000);
    assert!(matches!(
        payment.details,
        PaymentDetails::Ln {
            data: LnPaymentDetails { keysend: true, .. }
        }
    ));

    // Ensure that the next payment does not occur at the same time (down to the second).
    sleep(SECOND).await;

    // Close channels.
    info!("Closing channels");
    services.close_lsp_channels().await.unwrap();
    let payments = services.list_payments(Default::default()).await.unwrap();
    assert_eq!(payments.len(), 6);
    let payment = payments.first().cloned().unwrap();
    assert_eq!(payment.payment_type, PaymentType::Received);
    assert!(matches!(
        payment.details,
        PaymentDetails::ClosedChannel {
            data: ClosedChannelPaymentDetails {
                state: ChannelState::PendingClose,
                ..
            }
        }
    ));
    assert_eq!(payment.amount_msat, 5707000);
    assert_eq!(payment.fee_msat, 1170000);

    // Waiting here for an extra block to let LDK Node to catch up.
    bitcoind.generate_blocks(1).await.unwrap();
    info!("Waiting for BreezEvent::NewBlock...");
    wait_for!(matches!(
        events.recv().await,
        Some(BreezEvent::NewBlock { .. })
    ));
    let tip = services.node_info().unwrap().block_height;
    let block_numers = 6;
    bitcoind.generate_blocks(block_numers).await.unwrap();
    info!("Waiting for BreezEvent::NewBlock...");
    wait_for!(matches!(
        events.recv().await,
        Some(BreezEvent::NewBlock { block }) if block == tip + block_numers
    ));
    let node_info = services.node_info().unwrap();
    assert_eq!(node_info.channels_balance_msat, 0);
    assert_eq!(node_info.pending_onchain_balance_msat, 0);
    assert_eq!(node_info.onchain_balance_msat, 5707000);

    let payments = services.list_payments(Default::default()).await.unwrap();
    assert_eq!(payments.len(), 6);
    let payment = payments.first().cloned().unwrap();
    assert_eq!(payment.payment_type, PaymentType::Received);
    assert!(matches!(
        payment.details,
        PaymentDetails::ClosedChannel {
            data: ClosedChannelPaymentDetails {
                state: ChannelState::Closed,
                ..
            }
        }
    ));
    assert_eq!(payment.amount_msat, 5707000);
    assert_eq!(payment.fee_msat, 1170000);

    // Redeem funds.
    let address = bitcoind.get_new_address().await.unwrap();
    let request = RedeemOnchainFundsRequest {
        to_address: address.to_string(),
        sat_per_vbyte: 2,
    };
    info!("Redeeming on-chain funds to {address}");
    services.redeem_onchain_funds(request).await.unwrap();
    bitcoind.generate_blocks(1).await.unwrap();
    info!("Waiting for BreezEvent::NewBlock...");
    wait_for!(matches!(
        events.recv().await,
        Some(BreezEvent::NewBlock { .. })
    ));
    let node_info = services.node_info().unwrap();
    assert_eq!(node_info.channels_balance_msat, 0);
    assert_eq!(node_info.pending_onchain_balance_msat, 0);
    assert_eq!(node_info.onchain_balance_msat, 0);
    let balance = bitcoind.get_address_balance(&address).await.unwrap();
    assert!(balance.to_sat() > 5400);

    services.disconnect().await.unwrap();
    drop(services);
    assert!(events.is_closed());

    info!("Restoring a node with restore_only=Some(true)");
    let req = ConnectRequest {
        config,
        seed,
        restore_only: Some(true),
    };
    let (tx, mut events) = mpsc::channel(100);
    let services = BreezServices::connect(req, Box::new(EventListenerImpl::new(tx)))
        .await
        .unwrap();
    info!("Waiting for BreezEvent::Synced...");
    assert!(matches!(events.recv().await, Some(BreezEvent::Synced)));
    services.disconnect().await.unwrap();
    drop(services);
    assert!(events.is_closed());
}

trait Msats {
    fn to_msat(&self) -> u64;
}

impl Msats for Amount {
    fn to_msat(&self) -> u64 {
        self.to_sat() * 1000
    }
}
