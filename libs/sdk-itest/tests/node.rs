mod event_listener;

use breez_sdk_core::{
    BreezEvent, BreezServices, Config, ConnectRequest, GreenlightNodeConfig, NodeConfig,
    ReceivePaymentRequest,
};
use rand::Rng;
use rstest::*;
use sdk_itest::environment::Environment;
use sdk_itest::wait_for;
use testdir::testdir;
use tokio::sync::mpsc;
use tokio::try_join;

use crate::event_listener::EventListenerImpl;

#[rstest]
#[tokio::test(flavor = "multi_thread")]
#[test_log::test]
async fn test_node_receive_payments() {
    let env = Environment::default();
    let (esplora, mempool, vss, lsp, lnd) = try_join!(
        env.esplora_api(),
        env.mempool_api(),
        env.vss_api(),
        env.lsp_external_address(),
        env.lnd_with_channel()
    )
    .unwrap();
    println!("Esplora is running: {}", esplora.external_endpoint());
    println!("Mempool is running: {}", mempool.external_endpoint());
    println!("    VSS is running: {}", vss.external_endpoint());
    println!("    LSP is running: {lsp}");
    println!("    LND is running");

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
    config.rgs_url = "http://localhost:9".to_string();
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

    println!("Waiting for BreezEvent::Synced...");
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
    println!("Invoice created: {bolt11}");

    lnd.pay(bolt11).await.unwrap();
    println!("Waiting for BreezEvent::InvoicePaid...");
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
    println!("Invoice created: {bolt11}");
    lnd.pay(bolt11).await.unwrap();
    println!("Waiting for BreezEvent::InvoicePaid...");
    wait_for!(matches!(
        events.recv().await,
        Some(BreezEvent::InvoicePaid { .. })
    ));

    services.disconnect().await.unwrap();
    drop(services);
    assert!(events.is_closed());
}
