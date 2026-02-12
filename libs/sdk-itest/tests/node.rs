mod event_listener;

use std::time::Duration;

use bitcoin::Amount;
use breez_sdk_core::error::{ConnectError, SendPaymentError};
use breez_sdk_core::{
    BreezEvent, BreezServices, Config, ConnectRequest, InputType, ListPaymentsRequest,
    LnPaymentDetails, PaymentDetails, PaymentStatus, PaymentType, ReceivePaymentRequest,
    SendBolt12PaymentRequest, SendPaymentRequest, SendSpontaneousPaymentRequest, parse,
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
const UNPAYABLE_BOLT11: &str = "lnbcrt10u1p5h5g5kpp5asutj0mvuxr7g5asar2cu0l0mreyxp6a88mmerjuzk5r64zqpyxsdq9f38ygcqzzsxq97zvuqsp5hagpy8n954f86y7ca3kx5alr36a9nr4md6cyzfz9anmkf33nv63q9qxpqysgqrnjfrk9j6q6zl7alg287mhf8qfj5wawk6kk7n7rkgx82zd9y50sy8w4edmsetqatfpv5ezjkv7wxse2p7m63ax6mt7gkllwr3jmw0mcp9urhh9";

#[ignore = "Manual test for testing the environment itself"]
#[rstest]
#[tokio::test(flavor = "multi_thread")]
#[test_log::test]
async fn test_environment() {
    Environment::default().cln_with_channel().await.unwrap();
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
#[test_log::test]
async fn test_node_receive_payments() {
    let env = Environment::default();
    let (esplora, mempool, vss, lsp, lnd, rgs) = try_join!(
        env.esplora_api(),
        env.mempool_api(),
        env.vss_api(),
        env.lsp(),
        env.lnd_with_channel(),
        env.rgs()
    )
    .unwrap();
    let lsp_id = lsp.get_node_id().await.unwrap();
    let lsp_address = lsp.lightning_api.external_address();
    let lsp_address = format!("{lsp_id}@{lsp_address}");

    info!("Esplora is running: {}", esplora.external_endpoint());
    info!("Mempool is running: {}", mempool.external_endpoint());
    info!("    VSS is running: {}", vss.external_endpoint());
    info!("    LSP is running: {lsp_address}");
    info!("    LND is running");
    info!("    RGS is running: {}", rgs.external_endpoint());

    let mut config = Config::regtest(String::new());
    config.working_dir = testdir!().to_string_lossy().to_string();
    config.mempoolspace_url = Some(mempool.external_endpoint());
    config.esplora_url = esplora.external_endpoint();
    config.vss_url = vss.external_endpoint();
    config.rgs_url = rgs.external_endpoint();
    config.lsps2_address = lsp_address;

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

    let node_pubkey = services.node_info().await.id;
    let lnd_pubkey = lnd.get_id().await.unwrap();

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
    let invoice = response.ln_invoice;

    lnd.pay(invoice.bolt11.clone()).await.unwrap();
    info!("Waiting for BreezEvent::InvoicePaid...");
    wait_for!(matches!(
        events.recv().await,
        Some(BreezEvent::InvoicePaid { .. })
    ));
    let balance_msat = services.node_info().await.channels_balance_msat;
    assert_eq!(balance_msat, huge_amount_msat - opening_fee_msat);
    let payments = services.list_payments(Default::default()).await.unwrap();
    assert_eq!(payments.len(), 1);
    let payment = payments.first().cloned().unwrap();
    assert_eq!(payment.amount_msat, huge_amount_msat - opening_fee_msat);
    assert_eq!(payment.fee_msat, opening_fee_msat);
    assert_eq!(payment.payment_type, PaymentType::Received);
    assert_eq!(payment.description.unwrap(), "Init");
    if let PaymentDetails::Ln { data } = &payment.details {
        assert_eq!(data.bolt11, invoice.bolt11);
        assert_eq!(data.destination_pubkey, node_pubkey);
    } else {
        panic!("Expected LN payment details");
    }

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
    let invoice = response.ln_invoice;
    lnd.pay(invoice.bolt11.clone()).await.unwrap();
    info!("Waiting for BreezEvent::InvoicePaid...");
    wait_for!(matches!(
        events.recv().await,
        Some(BreezEvent::InvoicePaid { .. })
    ));
    let balance_msat = services.node_info().await.channels_balance_msat;
    assert_eq!(
        balance_msat,
        huge_amount_msat - opening_fee_msat + small_amount_msat
    );
    let payments = services.list_payments(Default::default()).await.unwrap();
    assert_eq!(payments.len(), 2);
    let payment = payments.first().cloned().unwrap();
    assert_eq!(payment.amount_msat, small_amount_msat);
    assert_eq!(payment.fee_msat, 0);
    assert_eq!(payment.payment_type, PaymentType::Received);
    assert_eq!(payment.description.unwrap(), "small");
    if let PaymentDetails::Ln { data } = &payment.details {
        assert_eq!(data.bolt11, invoice.bolt11);
        assert_eq!(data.destination_pubkey, node_pubkey);
    } else {
        panic!("Expected LN payment details");
    }

    // Ensure that the next payment does not occur at the same time (down to the second).
    sleep(SECOND).await;

    // Trying to pay an invoice from an unreachable node.
    let sent_payment = {
        let services = services.clone();
        tokio::spawn(async move {
            services
                .send_payment(SendPaymentRequest {
                    bolt11: UNPAYABLE_BOLT11.to_string(),
                    amount_msat: None,
                })
                .await
        })
    };
    info!("Waiting for seeing 3 payments...");
    wait_for!(
        services
            .list_payments(ListPaymentsRequest {
                include_failures: Some(true),
                ..Default::default()
            })
            .await
            .unwrap()
            .len()
            == 3
    );
    // The pending (or failed) payment is observed.
    let payments = services
        .list_payments(ListPaymentsRequest {
            include_failures: Some(true),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(payments.len(), 3);
    let payment = payments.first().cloned().unwrap();
    assert!(matches!(
        payment.status,
        PaymentStatus::Pending | PaymentStatus::Failed
    ));

    assert!(matches!(
        sent_payment.await.unwrap(),
        Err(SendPaymentError::PaymentFailed { .. })
    ));
    info!("Waiting for BreezEvent::PaymentFailed...");
    wait_for!(matches!(
        events.recv().await,
        Some(BreezEvent::PaymentFailed { .. })
    ));

    let payments = services
        .list_payments(ListPaymentsRequest {
            include_failures: Some(true),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(payments.len(), 3);
    let payment = payments.first().cloned().unwrap();
    assert_eq!(payment.status, PaymentStatus::Failed);

    // Paying BOLT-11 invoice.
    let amount = Amount::from_sat(1000);
    let bolt11 = lnd.receive(&amount).await.unwrap();

    let sent_payment = {
        let services = services.clone();
        let bolt11 = bolt11.clone();
        tokio::spawn(async move {
            services
                .send_payment(SendPaymentRequest {
                    bolt11,
                    amount_msat: None,
                })
                .await
        })
    };
    info!("Waiting for seeing 3 payments...");
    wait_for!(
        services
            .list_payments(Default::default())
            .await
            .unwrap()
            .len()
            == 3
    );
    // The pending (or complete) payment is observed.
    let payments = services.list_payments(Default::default()).await.unwrap();
    assert_eq!(payments.len(), 3);
    let payment = payments.first().cloned().unwrap();
    assert!(matches!(
        payment.status,
        PaymentStatus::Pending | PaymentStatus::Complete
    ));

    let payment = sent_payment.await.unwrap().unwrap().payment;
    assert_eq!(payment.status, PaymentStatus::Complete);
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
    assert_eq!(payment.description.unwrap(), "LND");
    if let PaymentDetails::Ln { data } = &payment.details {
        assert_eq!(data.bolt11, bolt11);
        assert_eq!(data.destination_pubkey, lnd_pubkey);
    } else {
        panic!("Expected LN payment details");
    }

    // Ensure that the next payment does not occur at the same time (down to the second).
    sleep(SECOND).await;

    // Paying open amount BOLT-11 invoice.
    let bolt11 = lnd.receive(&Amount::ZERO).await.unwrap();
    let amount = Amount::from_sat(1100);
    let payment = services
        .send_payment(SendPaymentRequest {
            bolt11: bolt11.clone(),
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
    assert_eq!(payment.description.unwrap(), "LND");
    if let PaymentDetails::Ln { data } = &payment.details {
        assert_eq!(data.bolt11, bolt11);
        assert_eq!(data.destination_pubkey, lnd_pubkey);
    } else {
        panic!("Expected LN payment details");
    }

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
    assert!(payment.description.is_none());
    if let PaymentDetails::Ln { data } = &payment.details {
        assert_eq!(data.bolt11, "");
        assert_eq!(data.destination_pubkey, "");
    } else {
        panic!("Expected LN payment details");
    }

    // Paying a BOLT-12 offer.
    let offer_amount = Amount::from_sat(10);
    let offer = lsp.get_offer(Some(offer_amount.to_msat())).await.unwrap();
    info!("Offer to pay: {offer}");
    let offer = match parse(&offer, None).await {
        Ok(InputType::Bolt12Offer { offer, .. }) => offer,
        result => panic!("Expected offer, got {result:?}"),
    };
    let req = SendBolt12PaymentRequest {
        offer,
        amount_msat: None,
        payer_note: None,
    };
    let payment = services.send_bolt12_payment(req).await.unwrap().payment;
    assert_eq!(payment.amount_msat, offer_amount.to_msat());
    assert_eq!(payment.fee_msat, 0);
    assert_eq!(payment.payment_type, PaymentType::Sent);
    assert_eq!(payment.status, PaymentStatus::Complete);
    wait_for!(matches!(
        events.recv().await,
        Some(BreezEvent::PaymentSucceed { .. })
    ));

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
