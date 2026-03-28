use std::collections::HashMap;

use crate::error::SdkResult;
use crate::models::LnPaymentInfo;
use crate::LnUrlInfo;

#[tonic::async_trait]
pub trait PaymentStore: Send + Sync {
    async fn set_ln_info(&self, payment_id: &str, info: &LnPaymentInfo) -> SdkResult<()>;
    async fn set_lnurl_info(&self, payment_id: &str, info: &LnUrlInfo) -> SdkResult<()>;
    async fn get_ln_info(&self, payment_ids: &[&str]) -> SdkResult<HashMap<String, LnPaymentInfo>>;
    async fn get_lnurl_info(&self, payment_ids: &[&str]) -> SdkResult<HashMap<String, LnUrlInfo>>;
}
