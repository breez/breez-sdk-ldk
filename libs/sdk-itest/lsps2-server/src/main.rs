use std::env;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::{Result, anyhow};
use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use ldk_node::bip39::Mnemonic;
use ldk_node::bitcoin::Network;
use ldk_node::lightning::ln::msgs::SocketAddress;
use ldk_node::liquidity::LSPS2ServiceConfig;
use ldk_node::{Builder, Node};
use log::{LevelFilter, error, info};
use serde::Serialize;
use tokio::signal::ctrl_c;
use tokio::signal::unix::{SignalKind, signal};
use tokio::sync::oneshot;

#[derive(Clone)]
struct AppState {
    node: Arc<Node>,
}

async fn getid(State(state): State<AppState>) -> String {
    state.node.node_id().to_string()
}

async fn newaddr(State(state): State<AppState>) -> String {
    match state.node.onchain_payment().new_address() {
        Ok(addr) => addr.to_string(),
        Err(e) => format!("Failed to get new address: {e}"),
    }
}

async fn sync(State(state): State<AppState>) -> String {
    match state.node.sync_wallets() {
        Ok(()) => "Synced".to_string(),
        Err(e) => format!("Failed to sync wallets: {e}"),
    }
}

async fn tip(State(state): State<AppState>) -> String {
    state.node.status().current_best_block.height.to_string()
}

#[derive(Serialize)]
struct Balance {
    total_onchain_sats: u64,
    spendable_onchain_sats: u64,
    lightning_sats: u64,
}

async fn balance(State(state): State<AppState>) -> Json<Balance> {
    let balances = state.node.list_balances();
    Json(Balance {
        total_onchain_sats: balances.total_onchain_balance_sats,
        spendable_onchain_sats: balances.spendable_onchain_balance_sats,
        lightning_sats: balances.total_lightning_balance_sats,
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format(|buf, record| {
            use std::io::Write;
            let ts = buf.timestamp_millis();
            writeln!(
                buf,
                "[{ts} {} {}:{}] {}",
                record.level(),
                record.target(),
                record.line().unwrap_or(0),
                record.args()
            )
        })
        .filter_module("hyper", LevelFilter::Info)
        .filter_module("reqwest", LevelFilter::Info)
        .filter_module("tracing", LevelFilter::Info)
        .init();

    info!("Building node...");
    let node = build()?;
    info!("Node built");

    info!("Starting node...");
    node.start()?;
    info!("Node started");

    let node = Arc::new(node);
    let state = AppState {
        node: Arc::clone(&node),
    };

    let rpc_address = env("RPC_LISTEN_ADDRESS")?;
    info!("Starting RPC server at {rpc_address}...");
    let app = Router::new()
        .route("/getid", get(getid))
        .route("/newaddr", get(newaddr))
        .route("/newaddr", post(newaddr))
        .route("/sync", get(sync))
        .route("/sync", post(sync))
        .route("/tip", get(tip))
        .route("/balance", get(balance))
        .with_state(state);
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let shutdown_signal = async move {
        shutdown_rx.await.ok();
    };
    let http_server = tokio::spawn(async move {
        let listener = tokio::net::TcpListener::bind(rpc_address).await.unwrap();
        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown_signal)
            .await
            .unwrap();
    });

    let mut sigterm = signal(SignalKind::terminate())?;
    tokio::select! {
        _ = async {
            loop {
                let event = node.next_event_async().await;
                info!("Event: {event:?}");
                if let Err(e) = node.event_handled() {
                    error!("Failed to mark event as handled: {e}");
                }
            }
        } => (),
        _ = ctrl_c() => {
            info!("Received Ctrl-C, shutting down...");
        }
        _ = sigterm.recv() => {
            info!("Received sigterm, shutting down...");
        }
    }

    info!("Stopping RPC server...");
    let _ = shutdown_tx.send(());

    info!("Stopping node...");
    node.stop()?;
    info!("Node stopped");

    let _ = http_server.await;

    Ok(())
}

fn build() -> Result<Node> {
    let config = ldk_node::config::Config::default();
    let mut builder = Builder::from_config(config);

    let mnemonic = env("MNEMONIC")?;
    let mnemonic = Mnemonic::from_str(&mnemonic)?;
    builder.set_entropy_bip39_mnemonic(mnemonic, None);
    builder.set_log_facade_logger();
    let storage_dir_path = env("STORAGE_PATH")?;
    builder.set_storage_dir_path(storage_dir_path);

    let network = env("NETWORK")?;
    let network = Network::from_str(&network.to_lowercase())?;
    builder.set_network(network);

    let lsp_token = env::var("LSP_TOKEN").ok();
    let service_config = LSPS2ServiceConfig {
        require_token: lsp_token,
        advertise_service: false,
        channel_opening_fee_ppm: 40_000,          // 4%
        channel_over_provisioning_ppm: 1_000_000, // 100%
        min_channel_opening_fee_msat: 1_000_000,
        min_channel_lifetime: 100_000,
        max_client_to_self_delay: 10000,
        min_payment_size_msat: 1_000_000,
        max_payment_size_msat: 100_000_000,
        client_trusts_lsp: false,
    };
    builder.set_liquidity_provider_lsps2(service_config);

    let listening_address = env("LISTENING_ADDRESS")?;
    let listening_address =
        SocketAddress::from_str(&listening_address).map_err(anyhow::Error::msg)?;
    builder.set_listening_addresses(vec![listening_address])?;

    let esplora_url = env("ESPLORA_URL")?;
    builder.set_chain_source_esplora(esplora_url, None);

    builder.set_node_alias("lsps2".to_string())?;

    let node = builder.build()?;
    Ok(node)
}

fn env(key: &str) -> Result<String> {
    env::var(key).map_err(|_| anyhow!("{key} is not set"))
}
