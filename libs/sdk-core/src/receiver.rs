use std::sync::Arc;

use sdk_common::ensure_sdk;
use sdk_common::invoice::parse_invoice;

use crate::error::ReceivePaymentError;
use crate::models::{
    LspAPI, OpeningFeeParams, ReceivePaymentRequest, ReceivePaymentResponse,
    INVOICE_PAYMENT_FEE_EXPIRY_SECONDS,
};
use crate::node_api::{CreateInvoiceRequest, NodeAPI};

#[cfg_attr(test, mockall::automock)]
#[tonic::async_trait]
pub trait Receiver: Send + Sync {
    fn open_channel_needed(&self, amount_msat: u64) -> Result<bool, ReceivePaymentError>;
    async fn receive_payment(
        &self,
        req: ReceivePaymentRequest,
    ) -> Result<ReceivePaymentResponse, ReceivePaymentError>;
}

pub(crate) struct PaymentReceiver {
    node_api: Arc<dyn NodeAPI>,
    lsp_api: Arc<dyn LspAPI>,
}

impl PaymentReceiver {
    pub(crate) fn new(node_api: Arc<dyn NodeAPI>, lsp_api: Arc<dyn LspAPI>) -> Self {
        Self { node_api, lsp_api }
    }

    async fn load_default_opening_fee_params(
        &self,
        expiry: u32,
    ) -> Result<OpeningFeeParams, ReceivePaymentError> {
        let node_pubkey = self.node_api.node_id().await?;
        self.lsp_api
            .list_lsps(node_pubkey)
            .await
            .map_err(|e| ReceivePaymentError::Generic { err: e.to_string() })?
            .into_iter()
            .next()
            .ok_or_else(|| ReceivePaymentError::Generic {
                err: "Empty LSP list".to_string(),
            })?
            .cheapest_open_channel_fee(expiry)
            .cloned()
            .map_err(Into::into)
    }
}

#[tonic::async_trait]
impl Receiver for PaymentReceiver {
    fn open_channel_needed(&self, amount_msat: u64) -> Result<bool, ReceivePaymentError> {
        Ok(self.node_api.max_receivable_single_payment_msat()? < amount_msat)
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

        let ReceivePaymentRequest {
            amount_msat,
            description,
            preimage,
            opening_fee_params: requested_opening_fee_params,
            use_description_hash,
            expiry,
            cltv: _,
        } = req;

        let expiry = expiry.unwrap_or(INVOICE_PAYMENT_FEE_EXPIRY_SECONDS);
        let open_channel_needed = self.open_channel_needed(amount_msat)?;

        let opening_fee_params = match (open_channel_needed, requested_opening_fee_params) {
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

        let bolt11 = self
            .node_api
            .create_invoice(CreateInvoiceRequest {
                amount_msat,
                description,
                use_description_hash,
                preimage,
                opening_fee_msat,
                expiry,
            })
            .await?;

        Ok(ReceivePaymentResponse {
            ln_invoice: parse_invoice(&bolt11)?,
            opening_fee_params,
            opening_fee_msat,
        })
    }
}
