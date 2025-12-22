use core::str::FromStr;
use std::collections::HashSet;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use chrono::{DateTime, Utc};
use hex::ToHex;
use ldk_node::bitcoin::hashes::sha256::Hash as Sha256;
use ldk_node::bitcoin::hashes::Hash;
use ldk_node::bitcoin::secp256k1::PublicKey;
use ldk_node::lightning::ln::msgs::SocketAddress;
use ldk_node::lightning::routing::router::{
    RouteParametersConfig, DEFAULT_MAX_TOTAL_CLTV_EXPIRY_DELTA,
};
use ldk_node::lightning::util::persist::KVStoreSync;
use ldk_node::lightning_invoice::{Bolt11InvoiceDescription, Description};
use ldk_node::lightning_types::payment::{PaymentHash, PaymentPreimage};
use ldk_node::{Builder, CustomTlvRecord, DynStore, Event, Node};
use rand::Rng;
use sdk_common::ensure_sdk;
use sdk_common::invoice::parse_invoice;
use sdk_common::prelude::Network;
use serde_json::Value;
use tokio::sync::{broadcast, mpsc, watch};
use tokio_stream::wrappers::errors::BroadcastStreamRecvError::Lagged;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::{Stream, StreamExt};

use crate::bitcoin::bip32::{ChildNumber, Xpriv};
use crate::bitcoin::secp256k1::Secp256k1;
use crate::breez_services::{OpenChannelParams, Receiver};
use crate::error::{ReceivePaymentError, SdkError, SdkResult};
use crate::grpc;
use crate::ldk::event_handling::{start_event_handling, wait_for_payment_success};
use crate::ldk::node_state::convert_payment;
use crate::ldk::restore_state::RestoreStateTracker;
use crate::ldk::store_builder::{build_mirroring_store, build_vss_store};
use crate::lightning_invoice::RawBolt11Invoice;
use crate::models::{
    Config, LspAPI, OpeningFeeParams, OpeningFeeParamsMenu, ReceivePaymentRequest,
    ReceivePaymentResponse, INVOICE_PAYMENT_FEE_EXPIRY_SECONDS,
};
use crate::node_api::{
    CreateInvoiceRequest, FetchBolt11Result, IncomingPayment, NodeAPI, NodeError, NodeResult,
};
use crate::{
    CustomMessage, LspInformation, MaxChannelAmount, Payment, PaymentResponse,
    PrepareRedeemOnchainFundsRequest, PrepareRedeemOnchainFundsResponse, RouteHint, RouteHintHop,
    SyncResponse, TlvEntry,
};

pub(crate) type KVStore = Arc<DynStore>;

pub(crate) const PREIMAGES_PRIMARY_NS: &str = "preimages";
pub(crate) const PREIMAGES_SECONDARY_NS: &str = "";

pub(crate) fn preimage_store_key(payment_hash: &PaymentHash) -> String {
    payment_hash.0.encode_hex()
}

pub(crate) struct Ldk {
    config: Config,
    seed: [u8; 64],
    node: Arc<Node>,
    incoming_payments_tx: broadcast::Sender<IncomingPayment>,
    events_tx: broadcast::Sender<Event>,
    kv_store: KVStore,
    remote_lock_shutdown_tx: mpsc::Sender<()>,
}

impl Ldk {
    pub async fn build(
        config: Config,
        seed: &[u8],
        restore_only: Option<bool>,
    ) -> NodeResult<Self> {
        debug!("Building LDK Node");
        ensure_sdk!(
            matches!(config.network, Network::Regtest | Network::Signet),
            NodeError::generic("Only Regtest or Signet modes are supported for now")
        );

        let (lsp_id, lsp_address) = get_lsp(&config)?;

        // Allow anchor channels from the LSP without having on-chain funds available.
        let ldk_node_config = ldk_node::config::Config {
            anchor_channels_config: Some(ldk_node::config::AnchorChannelsConfig {
                trusted_peers_no_reserve: vec![lsp_id],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut builder = Builder::from_config(ldk_node_config);

        let mut bytes = [0u8; 64];
        bytes.copy_from_slice(seed);
        let seed = bytes;
        builder.set_entropy_seed_bytes(seed);
        builder.set_log_facade_logger();
        builder.set_network(to_ldk_network(&config.network));

        builder.set_chain_source_esplora(config.esplora_url.clone(), None);
        builder.set_gossip_source_rgs(config.rgs_url.clone());

        builder.set_liquidity_source_lsps2(lsp_id, lsp_address, None);

        let vss_store = build_vss_store(&config, &seed, "ldk_node")?;

        // It is not possible to use oneshot here, because `oneshot::Sender::send()`
        // consumes itself, not allowing to call `closed()` method after.
        let (remote_lock_shutdown_tx, remote_lock_shutdown_rx) = mpsc::channel(1);
        let mirroring_store =
            build_mirroring_store(&config.working_dir, vss_store, remote_lock_shutdown_rx).await?;
        let kv_store: KVStore = Arc::new(mirroring_store);

        let restore_state_tracker = RestoreStateTracker::new(Arc::clone(&kv_store));
        let was_initialized = restore_state_tracker.is_initialized()?;
        if restore_only.unwrap_or(false) && !was_initialized {
            return Err(NodeError::RestoreOnly(
                "restore_only requested but no persisted node state was found".to_string(),
            ));
        }

        let node = builder
            .build_with_store(Arc::clone(&kv_store))
            .map_err(|e| NodeError::Generic(format!("Fail to build LDK Node: {e}")))?;
        let node = Arc::new(node);
        debug!("LDK Node was built");
        if !was_initialized {
            restore_state_tracker.mark_initialized()?;
        }

        let (incoming_payments_tx, _) = broadcast::channel(10);
        let (events_tx, _) = broadcast::channel(10);

        Ok(Self {
            config,
            seed,
            node,
            incoming_payments_tx,
            events_tx,
            kv_store,
            remote_lock_shutdown_tx,
        })
    }

    async fn load_default_opening_fee_params(&self, expiry: u32) -> SdkResult<OpeningFeeParams> {
        self.list_lsps(self.node.node_id().to_string())
            .await?
            .into_iter()
            .next()
            .ok_or(SdkError::generic("Empty LSP list"))?
            .cheapest_open_channel_fee(expiry)
            .cloned()
            .map_err(Into::into)
    }

    fn create_invoice(
        &self,
        amount_msat: u64,
        opening_fee_msat: Option<u64>,
        description: Bolt11InvoiceDescription,
        preimage: Option<PaymentPreimage>,
        expiry: u32,
    ) -> NodeResult<String> {
        let preimage =
            preimage.unwrap_or_else(|| PaymentPreimage(rand::thread_rng().gen::<[u8; 32]>()));
        let payment_hash: PaymentHash = preimage.into();
        let key = preimage_store_key(&payment_hash);
        KVStoreSync::write(
            self.kv_store.as_ref(),
            PREIMAGES_PRIMARY_NS,
            PREIMAGES_SECONDARY_NS,
            &key,
            preimage.0.to_vec(),
        )?;

        let payments = self.node.bolt11_payment();
        let invoice = match opening_fee_msat {
            Some(opening_fee_msat) => payments.receive_via_jit_channel_for_hash(
                amount_msat,
                &description,
                expiry,
                Some(opening_fee_msat),
                payment_hash,
            ),
            None => payments.receive_for_hash(amount_msat, &description, expiry, payment_hash),
        }?;
        Ok(invoice.to_string())
    }
}

#[tonic::async_trait]
impl NodeAPI for Ldk {
    async fn configure_node(&self, _close_to_address: Option<String>) -> NodeResult<()> {
        Err(NodeError::generic("LDK implementation not yet available"))
    }

    async fn create_invoice(&self, _request: CreateInvoiceRequest) -> NodeResult<String> {
        Err(NodeError::generic(
            "NodeAPI::create_invoice() must not be called directly for LDK implementation",
        ))
    }

    async fn delete_invoice(&self, _bolt11: String) -> NodeResult<()> {
        Err(NodeError::generic("LDK implementation not yet available"))
    }

    async fn fetch_bolt11(&self, _payment_hash: Vec<u8>) -> NodeResult<Option<FetchBolt11Result>> {
        Err(NodeError::generic("LDK implementation not yet available"))
    }

    async fn pull_changed(
        &self,
        _sync_state: Option<Value>,
        _match_local_balance: bool,
    ) -> NodeResult<SyncResponse> {
        self.node.sync_wallets()?;
        let node = &*self.node;
        let local_node_id = node.node_id();
        let payments = node
            .list_payments()
            .into_iter()
            .map(|p| convert_payment(p, &local_node_id))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(SyncResponse {
            sync_state: Value::Null,
            node_state: node.into(),
            payments,
            channels: Vec::new(),
        })
    }

    async fn send_payment(&self, bolt11: String, amount_msat: Option<u64>) -> NodeResult<Payment> {
        let invoice = ldk_node::lightning_invoice::Bolt11Invoice::from_str(&bolt11)?;
        let payments = self.node.bolt11_payment();
        let events = self.events_tx.subscribe(); // Subscribe before we try to send.
        let params = Some(RouteParametersConfig {
            max_total_routing_fee_msat: None,
            max_total_cltv_expiry_delta: DEFAULT_MAX_TOTAL_CLTV_EXPIRY_DELTA,
            max_path_count: 3,
            max_channel_saturation_power_of_half: 2,
        });
        let payment_id = match amount_msat {
            Some(amount_msat) => payments.send_using_amount(&invoice, amount_msat, params),
            None => payments.send(&invoice, params),
        }?;

        let payment = wait_for_payment_success(&self.node, events, payment_id).await?;
        convert_payment(payment, &self.node.node_id())
    }

    async fn send_spontaneous_payment(
        &self,
        node_id: String,
        amount_msat: u64,
        extra_tlvs: Option<Vec<TlvEntry>>,
    ) -> NodeResult<Payment> {
        let node_id = PublicKey::from_str(&node_id)
            .map_err(|e| NodeError::Generic(format!("Invalid public key: {e}")))?;

        let events = self.events_tx.subscribe(); // Subscribe before we try to send.
        let payments = self.node.spontaneous_payment();
        let payment_id = match extra_tlvs {
            Some(extra_tlvs) => {
                let custom_tlvs = extra_tlvs
                    .into_iter()
                    .map(|tlv| CustomTlvRecord {
                        type_num: tlv.field_number,
                        value: tlv.value,
                    })
                    .collect();
                payments.send_with_custom_tlvs(amount_msat, node_id, None, custom_tlvs)
            }
            None => payments.send(amount_msat, node_id, None),
        }?;

        let payment = wait_for_payment_success(&self.node, events, payment_id).await?;
        convert_payment(payment, &self.node.node_id())
    }

    async fn node_id(&self) -> NodeResult<String> {
        Ok(self.node.node_id().to_string())
    }

    async fn send_pay(&self, _bolt11: String, _max_hops: u32) -> NodeResult<PaymentResponse> {
        Err(NodeError::generic("LDK implementation not yet available"))
    }

    async fn max_sendable_amount<'a>(
        &self,
        _payee_node_id: Option<Vec<u8>>,
        _max_hops: u32,
        _last_hop: Option<&'a RouteHintHop>,
    ) -> NodeResult<Vec<MaxChannelAmount>> {
        Err(NodeError::generic("LDK implementation not yet available"))
    }

    async fn redeem_onchain_funds(
        &self,
        _to_address: String,
        _sat_per_vbyte: u32,
    ) -> NodeResult<Vec<u8>> {
        Err(NodeError::generic("LDK implementation not yet available"))
    }

    async fn prepare_redeem_onchain_funds(
        &self,
        _req: PrepareRedeemOnchainFundsRequest,
    ) -> NodeResult<PrepareRedeemOnchainFundsResponse> {
        Err(NodeError::generic("LDK implementation not yet available"))
    }

    async fn start(&self, shutdown: mpsc::Receiver<()>) {
        debug!("Starting LDK Node");
        if let Err(e) = self.node.start() {
            error!("Failed to start LDK Node: {e}");
            return;
        }
        debug!("LDK Node started");

        debug!("Starting event handling");
        start_event_handling(
            Arc::clone(&self.node),
            self.events_tx.clone(),
            Arc::clone(&self.kv_store),
            self.incoming_payments_tx.clone(),
            shutdown,
        )
        .await;
        info!("Event handling stopped");

        debug!("Stopping LDK Node");
        if let Err(e) = self.node.stop() {
            error!("Error on stopping LDK Node: {e}");
        }
        debug!("LDK Node stopped");

        debug!("Stopping remote lock refreshing");
        let _ = self.remote_lock_shutdown_tx.send(()).await;
        debug!("Waiting for remote lock refreshing stopped");
        self.remote_lock_shutdown_tx.closed().await;

        debug!("Exiting Ldk::start()");
    }

    async fn start_keep_alive(&self, _shutdown: watch::Receiver<()>) {
        // No-op for LDK Node.
    }

    async fn connect_peer(&self, node_id: String, addr: String) -> NodeResult<()> {
        let node_id = PublicKey::from_str(&node_id)
            .map_err(|e| NodeError::Generic(format!("Invalid LSP public key: {e}")))?;
        let address = SocketAddress::from_str(&addr)
            .map_err(|e| NodeError::Generic(format!("Invalid LSP address: {e}")))?;
        let persist = false;
        self.node.connect(node_id, address, persist)?;
        Ok(())
    }

    async fn sign_invoice(&self, _invoice: RawBolt11Invoice) -> NodeResult<String> {
        Err(NodeError::generic("LDK implementation not yet available"))
    }

    async fn close_all_channels(&self) -> NodeResult<()> {
        for channel_id in self.node.list_channels() {
            self.node
                .close_channel(&channel_id.user_channel_id, channel_id.counterparty_node_id)?;
        }
        Ok(())
    }

    async fn stream_incoming_payments(
        &self,
    ) -> NodeResult<Pin<Box<dyn Stream<Item = IncomingPayment> + Send>>> {
        let stream = BroadcastStream::new(self.incoming_payments_tx.subscribe()).filter_map(|r| {
            r.map_err(|Lagged(n)| warn!("Incoming payments stream missed {n} events"))
                .ok()
        });
        Ok(Box::pin(stream))
    }

    async fn stream_log_messages(&self) -> NodeResult<Pin<Box<dyn Stream<Item = String> + Send>>> {
        // LDK Node is configured with facade logger.
        Ok(Box::pin(futures::stream::empty()))
    }

    async fn static_backup(&self) -> NodeResult<Vec<String>> {
        Err(NodeError::generic("LDK implementation not yet available"))
    }

    async fn execute_command(&self, _command: String) -> NodeResult<Value> {
        Err(NodeError::generic("LDK implementation not yet available"))
    }

    async fn generate_diagnostic_data(&self) -> NodeResult<Value> {
        Err(NodeError::generic("LDK implementation not yet available"))
    }

    async fn sign_message(&self, _message: &str) -> NodeResult<String> {
        Err(NodeError::generic("LDK implementation not yet available"))
    }

    async fn check_message(
        &self,
        _message: &str,
        _pubkey: &str,
        _signature: &str,
    ) -> NodeResult<bool> {
        Err(NodeError::generic("LDK implementation not yet available"))
    }

    async fn send_custom_message(&self, _message: CustomMessage) -> NodeResult<()> {
        Err(NodeError::generic("LDK implementation not yet available"))
    }

    async fn stream_custom_messages(
        &self,
    ) -> NodeResult<Pin<Box<dyn Stream<Item = anyhow::Result<CustomMessage>> + Send>>> {
        Err(NodeError::generic("LDK implementation not yet available"))
    }

    async fn derive_bip32_key(&self, path: Vec<ChildNumber>) -> NodeResult<Xpriv> {
        let bitcoin_network: crate::bitcoin::Network = self.config.network.into();
        Ok(
            Xpriv::new_master(bitcoin_network, &self.seed)?
                .derive_priv(&Secp256k1::new(), &path)?,
        )
    }

    async fn legacy_derive_bip32_key(&self, path: Vec<ChildNumber>) -> NodeResult<Xpriv> {
        // Using the main implementation, because legacy way was never used for LDK.
        self.derive_bip32_key(path).await
    }

    async fn get_routing_hints(
        &self,
        _lsp_info: &LspInformation,
    ) -> NodeResult<(Vec<RouteHint>, bool)> {
        Err(NodeError::generic("LDK implementation not yet available"))
    }

    async fn get_open_peers(&self) -> NodeResult<HashSet<Vec<u8>>> {
        Err(NodeError::generic("LDK implementation not yet available"))
    }
}

#[tonic::async_trait]
impl LspAPI for Ldk {
    async fn list_lsps(&self, _node_pubkey: String) -> SdkResult<Vec<LspInformation>> {
        // TODO: Load data dynamically from LSP.
        let (pubkey, address) = get_lsp(&self.config)?;
        let lsp = match self.config.network {
            Network::Regtest => regtest_lsp(pubkey, address),
            Network::Signet => signet_lsp(pubkey, address),
            _ => return Err(SdkError::generic("Unsupported network")),
        };
        Ok(vec![lsp])
    }

    async fn list_used_lsps(&self, node_pubkey: String) -> SdkResult<Vec<LspInformation>> {
        self.list_lsps(node_pubkey).await
    }

    async fn register_payment_notifications(
        &self,
        _lsp_id: String,
        _lsp_pubkey: Vec<u8>,
        _webhook_url: String,
        _webhook_url_signature: String,
    ) -> SdkResult<grpc::RegisterPaymentNotificationResponse> {
        Err(SdkError::generic("LDK implementation not yet available"))
    }

    async fn unregister_payment_notifications(
        &self,
        _lsp_id: String,
        _lsp_pubkey: Vec<u8>,
        _webhook_url: String,
        _webhook_url_signature: String,
    ) -> SdkResult<grpc::RemovePaymentNotificationResponse> {
        Err(SdkError::generic("LDK implementation not yet available"))
    }

    async fn register_payment(
        &self,
        _lsp_id: String,
        _lsp_pubkey: Vec<u8>,
        _payment_info: grpc::PaymentInformation,
    ) -> SdkResult<grpc::RegisterPaymentReply> {
        Ok(Default::default())
    }
}

#[tonic::async_trait]
impl Receiver for Ldk {
    fn open_channel_needed(&self, amount_msat: u64) -> Result<bool, ReceivePaymentError> {
        let max_receivable_single_payment_amount_msat: u64 = self
            .node
            .list_channels()
            .iter()
            .map(|c| c.inbound_capacity_msat)
            .sum();
        Ok(max_receivable_single_payment_amount_msat < amount_msat)
    }

    async fn receive_payment(
        &self,
        req: ReceivePaymentRequest,
    ) -> Result<ReceivePaymentResponse, ReceivePaymentError> {
        ensure_sdk!(
            req.amount_msat > 0,
            ReceivePaymentError::InvalidAmount {
                err: "Receive amount must be more than 0".into()
            }
        );
        let amount_msat = req.amount_msat;
        let expiry = req.expiry.unwrap_or(INVOICE_PAYMENT_FEE_EXPIRY_SECONDS);
        let open_channel_needed = self.open_channel_needed(amount_msat)?;
        let opening_fee_params = match (open_channel_needed, req.opening_fee_params) {
            (true, Some(opening_fee_params)) => Some(opening_fee_params),
            (true, None) => Some(self.load_default_opening_fee_params(expiry).await?),
            (false, _) => None,
        };
        let opening_fee_msat = opening_fee_params
            .as_ref()
            .map(|p| p.get_channel_fees_msat_for(amount_msat));
        if let Some(opening_fee_msat) = opening_fee_msat {
            ensure_sdk!(
                amount_msat >= opening_fee_msat + 1000,
                ReceivePaymentError::InvalidAmount {
                    err: format!(
							"Amount should be more than the minimum fees {opening_fee_msat} msat, but is {amount_msat} msat"
                        )
                }
            );
        }

        let description = if req.use_description_hash.unwrap_or(false) {
            let hash = Sha256::hash(req.description.as_bytes());
            Bolt11InvoiceDescription::Hash(ldk_node::lightning_invoice::Sha256(hash))
        } else {
            let description =
                Description::new(req.description).map_err(|e| ReceivePaymentError::Generic {
                    err: format!("Failed to create invoice description: {e}"),
                })?;
            Bolt11InvoiceDescription::Direct(description)
        };

        let preimage = match req.preimage.map(|p| p.as_slice().try_into()) {
            Some(Ok(preimage)) => Some(PaymentPreimage(preimage)),
            Some(Err(e)) => {
                return Err(ReceivePaymentError::Generic {
                    err: format!("Invalid preimage given: {e}"),
                })
            }
            None => None,
        };

        let invoice =
            self.create_invoice(amount_msat, opening_fee_msat, description, preimage, expiry)?;
        info!("Invoice created {invoice}");
        let ln_invoice = parse_invoice(&invoice)?;

        Ok(ReceivePaymentResponse {
            ln_invoice,
            opening_fee_params,
            opening_fee_msat,
        })
    }

    async fn wrap_node_invoice(
        &self,
        invoice: &str,
        _params: Option<OpenChannelParams>,
        _lsp_info: Option<LspInformation>,
    ) -> Result<String, ReceivePaymentError> {
        Ok(invoice.to_string())
    }
}

fn to_ldk_network(network: &Network) -> ldk_node::bitcoin::network::Network {
    match network {
        Network::Bitcoin => ldk_node::bitcoin::network::Network::Bitcoin,
        Network::Testnet => ldk_node::bitcoin::network::Network::Testnet,
        Network::Signet => ldk_node::bitcoin::network::Network::Signet,
        Network::Regtest => ldk_node::bitcoin::network::Network::Regtest,
    }
}

fn get_lsp(config: &Config) -> NodeResult<(PublicKey, SocketAddress)> {
    match config.lsps2_address.split_once('@') {
        None => Err(NodeError::generic(
            "Invalid lsps2_address, does not containt @",
        )),
        Some((id, address)) => {
            let id = id
                .parse()
                .map_err(|e| NodeError::Generic(format!("Invalid LSP public key: {e}")))?;
            let address = SocketAddress::from_str(address)
                .map_err(|e| NodeError::Generic(format!("Invalid LSP address: {e}")))?;
            Ok((id, address))
        }
    }
}

fn regtest_lsp(pubkey: PublicKey, address: SocketAddress) -> LspInformation {
    let year = Duration::from_secs(60 * 60 * 24 * 365);
    let in_one_year = SystemTime::now() + year;
    let in_one_year: DateTime<Utc> = in_one_year.into();
    let opening_fee_params = OpeningFeeParams {
        min_msat: 1_000_000,
        proportional: 40_000,
        valid_until: in_one_year.to_rfc3339(),
        max_idle_time: 0,
        max_client_to_self_delay: 10_000,
        promise: "I promise".to_string(),
    };
    LspInformation {
        id: pubkey.to_string(),
        name: "Breez SDK Regtest LSPS2".to_string(),
        widget_url: "http://widget.example.com".to_string(),
        pubkey: pubkey.to_string(),
        host: address.to_string(),
        base_fee_msat: 1_000,
        fee_rate: 0.0,
        time_lock_delta: 72,
        min_htlc_msat: 1,
        lsp_pubkey: pubkey.serialize().to_vec(),
        opening_fee_params_list: OpeningFeeParamsMenu {
            values: vec![opening_fee_params],
        },
    }
}

fn signet_lsp(pubkey: PublicKey, address: SocketAddress) -> LspInformation {
    // TODO: Hard-code values for Megalith.
    let year = Duration::from_secs(60 * 60 * 24 * 365);
    let in_one_year = SystemTime::now() + year;
    let in_one_year: DateTime<Utc> = in_one_year.into();
    let opening_fee_params = OpeningFeeParams {
        min_msat: 1_000_000,
        proportional: 40_000,
        valid_until: in_one_year.to_rfc3339(),
        max_idle_time: 0,
        max_client_to_self_delay: 10_000,
        promise: "I promise".to_string(),
    };
    LspInformation {
        id: pubkey.to_string(),
        name: "Megalith LSPS2".to_string(),
        widget_url: "http://widget.example.com".to_string(),
        pubkey: pubkey.to_string(),
        host: address.to_string(),
        base_fee_msat: 1_000,
        fee_rate: 0.0,
        time_lock_delta: 72,
        min_htlc_msat: 1,
        lsp_pubkey: pubkey.serialize().to_vec(),
        opening_fee_params_list: OpeningFeeParamsMenu {
            values: vec![opening_fee_params],
        },
    }
}
