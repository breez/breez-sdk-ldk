mod event_listener;

use bitcoin::Amount;
use breez_sdk_core::{
    BreezEvent, BreezServices, Config, ConnectRequest, GreenlightNodeConfig, LnPaymentDetails,
    NodeConfig, PaymentDetails, ReceivePaymentRequest, SendPaymentRequest,
    SendSpontaneousPaymentRequest,
};
use rand::Rng;
use rstest::*;
use sdk_itest::environment::Environment;
use sdk_itest::wait_for;
use testdir::testdir;
use tokio::sync::mpsc;
use tokio::try_join;
use tracing::info;

use crate::event_listener::EventListenerImpl;

#[rstest]
#[tokio::test(flavor = "multi_thread")]
#[test_log::test]
async fn test_node_receive_payments() {
    let env = Environment::default();
    let (esplora, mempool, vss, lsp, lnd, rgs) = try_join!(
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

    let node_config = NodeConfig::Greenlight {
        config: GreenlightNodeConfig {
            partner_credentials: None,
            invite_code: None,
        },
    };
    let mut config = Config::regtest(String::new(), node_config);
    config.working_dir = testdir!().to_string_lossy().to_string();
    config.mempoolspace_url = Some(mempool.external_endpoint());
    config.esplora_url = esplora.external_endpoint();
    config.vss_url = vss.external_endpoint();
    config.rgs_url = rgs.external_endpoint();
    config.lsps2_address = lsp;

    let seed = rand::rng().random::<[u8; 64]>().to_vec();
    let req = ConnectRequest {
        config,
        seed,
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

    // Paying BOLT-11 invoice.
    let amount = Amount::from_sat(1000);
    let bolt11 = lnd.receive(&amount).await.unwrap();
    let payment = services
        .send_payment(SendPaymentRequest {
            bolt11,
            use_trampoline: false,
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

    // Paying open amount BOLT-11 invoice.
    let bolt11 = lnd.receive(&Amount::ZERO).await.unwrap();
    let amount = Amount::from_sat(1100);
    let payment = services
        .send_payment(SendPaymentRequest {
            bolt11,
            use_trampoline: false,
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
