use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use bitcoin::secp256k1::ecdsa::{RecoverableSignature, RecoveryId};
use bitcoin::secp256k1::{Message, PublicKey, Secp256k1};
use hex::decode;
use log::warn;

use crate::common::{
    build_signed_message, API_KEY_HEADER, REQUEST_TIME_HEADER, SIGNATURE_HEADER, USER_PUBKEY_HEADER,
};

#[derive(Debug, PartialEq, Eq)]
pub struct AuthenticationFailed;

pub fn authenticate(
    headers: &HashMap<String, String>,
    max_skew: Duration,
) -> Result<String, AuthenticationFailed> {
    let headers: HashMap<_, _> = headers
        .iter()
        .map(|(k, v)| (k.to_ascii_lowercase(), v))
        .collect();

    let pubkey_hex = find_header(&headers, USER_PUBKEY_HEADER)?;
    let pubkey_bytes = decode(pubkey_hex).map_err(|e| {
        warn!("Authentication failed: invalid user pubkey: {e}");
        AuthenticationFailed
    })?;
    let expected_pubkey = PublicKey::from_slice(&pubkey_bytes).map_err(|e| {
        warn!("Authentication failed: invalid user pubkey: {e}");
        AuthenticationFailed
    })?;

    let api_key = find_header(&headers, API_KEY_HEADER)?;

    let request_timestamp: u32 = find_header(&headers, REQUEST_TIME_HEADER)?
        .parse()
        .map_err(|e| {
            warn!("Authentication failed: invalid {REQUEST_TIME_HEADER} header: {e}");
            AuthenticationFailed
        })?;

    let request_time = UNIX_EPOCH + Duration::from_secs(request_timestamp as u64);
    let now = SystemTime::now();
    let skew = if now >= request_time {
        now.duration_since(request_time).map_err(|e| {
            warn!("Authentication failed: invalid {REQUEST_TIME_HEADER} header: {e}");
            AuthenticationFailed
        })?
    } else {
        request_time.duration_since(now).map_err(|e| {
            warn!("Authentication failed: invalid {REQUEST_TIME_HEADER} header: {e}");
            AuthenticationFailed
        })?
    };
    if skew > max_skew {
        warn!("Authentication failed: request time skewed");
        return Err(AuthenticationFailed);
    }

    let signature_header = find_header(&headers, SIGNATURE_HEADER)?;
    let signature_bytes = zbase32::decode_full_bytes(signature_header.as_bytes()).map_err(|e| {
        warn!("Authentication failed: {SIGNATURE_HEADER} header is not valid zbase32: {e}");
        AuthenticationFailed
    })?;
    if signature_bytes.len() != 65 {
        warn!(
            "Authentication failed: signature has invalid length (got {}, expected 65)",
            signature_bytes.len()
        );
        return Err(AuthenticationFailed);
    }
    let recovery_id = RecoveryId::from_i32(signature_bytes[64] as i32).map_err(|e| {
        warn!("Authentication failed: signature has invalid recovery id: {e}");
        AuthenticationFailed
    })?;
    let mut compact = [0u8; 64];
    compact.copy_from_slice(&signature_bytes[..64]);
    let signature = RecoverableSignature::from_compact(&compact, recovery_id).map_err(|e| {
        warn!("Authentication failed: signature could not be parsed: {e}");
        AuthenticationFailed
    })?;

    let digest = build_signed_message(api_key, request_timestamp);
    let msg = Message::from_digest_slice(&digest).map_err(|e| {
        warn!("Authentication failed: signed message digest is invalid: {e}");
        AuthenticationFailed
    })?;
    let secp = Secp256k1::verification_only();
    let recovered_pubkey = secp.recover_ecdsa(&msg, &signature).map_err(|e| {
        warn!("Authentication failed: could not recover pubkey from signature: {e}");
        AuthenticationFailed
    })?;
    if recovered_pubkey != expected_pubkey {
        warn!("Authentication failed: recovered pubkey does not match expected pubkey");
        return Err(AuthenticationFailed);
    }

    Ok(pubkey_hex.to_string())
}

fn find_header<'a>(
    headers: &HashMap<String, &'a String>,
    name: &'static str,
) -> Result<&'a String, AuthenticationFailed> {
    headers
        .get(&name.to_ascii_lowercase())
        .ok_or_else(|| {
            warn!("Authentication failed: missing {name} header");
            AuthenticationFailed
        })
        .copied()
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

        let result = authenticate(&headers, Duration::from_secs(30));

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

        let result = authenticate(&headers, Duration::from_secs(30));

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

        let result = authenticate(&headers, Duration::from_secs(30));

        assert_eq!(result, Err(AuthenticationFailed));
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

        let result = authenticate(&headers, Duration::from_secs(30));

        assert_eq!(result, Err(AuthenticationFailed));
    }
}
