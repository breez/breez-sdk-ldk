use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use bitcoin::secp256k1::ecdsa::{RecoverableSignature, RecoveryId};
use bitcoin::secp256k1::{Message, PublicKey, Secp256k1};
use hex::decode;
use thiserror::Error;

use crate::common::{
    build_signed_message, API_KEY_HEADER, REQUEST_TIME_HEADER, SIGNATURE_HEADER, USER_PUBKEY_HEADER,
};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AuthValidationError {
    #[error("Missing header {0}")]
    MissingHeader(&'static str),
    #[error("Invalid header {0}")]
    InvalidHeader(&'static str),
    #[error("Invalid signature")]
    InvalidSignature,
    #[error("Request time outside allowed skew")]
    RequestTimeSkewed,
    #[error("Invalid api key")]
    InvalidApiKey,
}

pub fn authenticate(
    headers: &HashMap<String, String>,
    max_skew: Duration,
    expected_api_key: Option<&str>,
) -> Result<String, AuthValidationError> {
    let pubkey_hex = find_header(headers, USER_PUBKEY_HEADER)?;
    let pubkey_bytes =
        decode(pubkey_hex).map_err(|_| AuthValidationError::InvalidHeader(USER_PUBKEY_HEADER))?;
    let expected_pubkey = PublicKey::from_slice(&pubkey_bytes)
        .map_err(|_| AuthValidationError::InvalidHeader(USER_PUBKEY_HEADER))?;

    let api_key = find_header(headers, API_KEY_HEADER)?;
    if let Some(expected_api_key) = expected_api_key {
        if expected_api_key != api_key {
            return Err(AuthValidationError::InvalidApiKey);
        }
    }

    let request_timestamp: u32 = find_header(headers, REQUEST_TIME_HEADER)?
        .parse()
        .map_err(|_| AuthValidationError::InvalidHeader(REQUEST_TIME_HEADER))?;

    let request_time = UNIX_EPOCH + Duration::from_secs(request_timestamp as u64);
    let now = SystemTime::now();
    let skew = if now >= request_time {
        now.duration_since(request_time)
            .map_err(|_| AuthValidationError::InvalidHeader(REQUEST_TIME_HEADER))?
    } else {
        request_time
            .duration_since(now)
            .map_err(|_| AuthValidationError::InvalidHeader(REQUEST_TIME_HEADER))?
    };
    if skew > max_skew {
        return Err(AuthValidationError::RequestTimeSkewed);
    }

    let signature_header = find_header(headers, SIGNATURE_HEADER)?;
    let signature_bytes = zbase32::decode_full_bytes(signature_header.as_bytes())
        .map_err(|_| AuthValidationError::InvalidSignature)?;
    if signature_bytes.len() != 65 {
        return Err(AuthValidationError::InvalidSignature);
    }
    let recovery_id = RecoveryId::from_i32(signature_bytes[64] as i32)
        .map_err(|_| AuthValidationError::InvalidSignature)?;
    let mut compact = [0u8; 64];
    compact.copy_from_slice(&signature_bytes[..64]);
    let signature = RecoverableSignature::from_compact(&compact, recovery_id)
        .map_err(|_| AuthValidationError::InvalidSignature)?;

    let digest = build_signed_message(api_key, request_timestamp);
    let msg =
        Message::from_digest_slice(&digest).map_err(|_| AuthValidationError::InvalidSignature)?;
    let secp = Secp256k1::verification_only();
    let recovered_pubkey = secp
        .recover_ecdsa(&msg, &signature)
        .map_err(|_| AuthValidationError::InvalidSignature)?;
    if recovered_pubkey != expected_pubkey {
        return Err(AuthValidationError::InvalidSignature);
    }

    Ok(pubkey_hex.to_string())
}

fn find_header<'a>(
    headers: &'a HashMap<String, String>,
    name: &'static str,
) -> Result<&'a String, AuthValidationError> {
    headers
        .get(name)
        .or_else(|| headers.get(&name.to_ascii_lowercase()))
        .ok_or(AuthValidationError::MissingHeader(name))
}

#[cfg(all(test, feature = "signing"))]
mod tests {
    use super::*;
    use crate::signing::HeaderProvider;
    use bitcoin::Network;
    use vss_client_ng::headers::VssHeaderProvider;

    #[tokio::test]
    async fn validates_valid_headers() {
        let seed = [9u8; 64];
        let network = Network::Bitcoin;
        let api_key = "expected-api-key".to_string();
        let provider = HeaderProvider::new(&seed, network, api_key.clone()).unwrap();
        let headers = provider.get_headers(&[]).await.unwrap();

        let result = authenticate(&headers, Duration::from_secs(30), Some(api_key.as_str()));

        let result = result.unwrap();
        assert_eq!(result, provider.pubkey_hex().to_string());
    }

    #[tokio::test]
    async fn validates_lowercase_headers() {
        let seed = [9u8; 64];
        let network = Network::Bitcoin;
        let api_key = "expected-api-key".to_string();
        let provider = HeaderProvider::new(&seed, network, api_key.clone()).unwrap();
        let headers = provider.get_headers(&[]).await.unwrap();
        let headers = headers
            .into_iter()
            .map(|(k, v)| (k.to_ascii_lowercase(), v))
            .collect();

        let result = authenticate(&headers, Duration::from_secs(30), Some(api_key.as_str()));

        let result = result.unwrap();
        assert_eq!(result, provider.pubkey_hex().to_string());
    }

    #[tokio::test]
    async fn fails_on_invalid_signature() {
        let seed = [1u8; 64];
        let network = Network::Bitcoin;
        let provider = HeaderProvider::new(&seed, network, String::new()).unwrap();
        let mut headers = provider.get_headers(&[]).await.unwrap();
        // Corrupt signature.
        headers.insert(SIGNATURE_HEADER.to_string(), "invalidsig".into());

        let result = authenticate(&headers, Duration::from_secs(30), None);

        assert_eq!(result, Err(AuthValidationError::InvalidSignature));
    }

    #[tokio::test]
    async fn fails_on_stale_request() {
        let seed = [2u8; 64];
        let network = Network::Bitcoin;
        let api_key = "expected-api-key".to_string();
        let provider = HeaderProvider::new(&seed, network, api_key.clone()).unwrap();
        let stale_time = 1234;
        let signature = provider.sign_request(stale_time).expect("signing");
        let mut headers = HashMap::new();
        headers.insert(SIGNATURE_HEADER.to_string(), signature);
        headers.insert(REQUEST_TIME_HEADER.to_string(), stale_time.to_string());
        headers.insert(API_KEY_HEADER.to_string(), api_key);
        headers.insert(
            USER_PUBKEY_HEADER.to_string(),
            provider.pubkey_hex().to_string(),
        );

        let result = authenticate(&headers, Duration::from_secs(30), None);

        assert_eq!(result, Err(AuthValidationError::RequestTimeSkewed));
    }

    #[tokio::test]
    async fn fails_on_api_key_mismatch() {
        let seed = [3u8; 64];
        let network = Network::Bitcoin;
        let provider = HeaderProvider::new(&seed, network, "wrong".into()).unwrap();
        let headers = provider.get_headers(&[]).await.unwrap();

        let result = authenticate(&headers, Duration::from_secs(30), Some("expected"));

        assert_eq!(result, Err(AuthValidationError::InvalidApiKey));
    }
}
