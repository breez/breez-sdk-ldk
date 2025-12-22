use std::collections::HashMap;
use std::convert::TryInto;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use bitcoin::bip32::{ChildNumber, Xpriv};
use bitcoin::secp256k1::{Message, PublicKey, Secp256k1, SecretKey, SignOnly};
use bitcoin::Network;
use hex::ToHex;
use vss_client_ng::headers::{VssHeaderProvider, VssHeaderProviderError};

use crate::common::{
    build_signed_message, API_KEY_HEADER, REQUEST_TIME_HEADER, SIGNATURE_HEADER, USER_PUBKEY_HEADER,
};

const VSS_SIGNING_CHILD_INDEX: u32 = 877;

pub struct HeaderProvider {
    secp: Secp256k1<SignOnly>,
    secret_key: SecretKey,
    pubkey_hex: String,
    api_key: String,
}

impl HeaderProvider {
    pub fn new(
        seed: &[u8],
        network: Network,
        api_key: String,
    ) -> Result<Self, VssHeaderProviderError> {
        let (private_key, pubkey_hex) = derive_signing_keys(seed, network)?;
        Ok(Self {
            secp: Secp256k1::signing_only(),
            secret_key: private_key,
            pubkey_hex,
            api_key,
        })
    }

    pub(crate) fn sign_request(
        &self,
        request_timestamp: u32,
    ) -> Result<String, VssHeaderProviderError> {
        let digest = build_signed_message(&self.api_key, request_timestamp);
        let msg = Message::from_digest_slice(&digest).map_err(|e| {
            VssHeaderProviderError::InternalError {
                error: e.to_string(),
            }
        })?;
        let sig = self.secp.sign_ecdsa_recoverable(&msg, &self.secret_key);
        let (recovery_id, compact) = sig.serialize_compact();
        let mut signature_bytes = [0u8; 65];
        signature_bytes[..64].copy_from_slice(&compact);
        signature_bytes[64] = recovery_id.to_i32() as u8;
        Ok(zbase32::encode_full_bytes(&signature_bytes))
    }

    #[cfg(test)]
    pub(crate) fn pubkey_hex(&self) -> &str {
        &self.pubkey_hex
    }
}

#[async_trait]
impl VssHeaderProvider for HeaderProvider {
    async fn get_headers(
        &self,
        _request: &[u8],
    ) -> Result<HashMap<String, String>, VssHeaderProviderError> {
        let request_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| VssHeaderProviderError::InternalError {
                error: e.to_string(),
            })?
            .as_secs() as u32;
        let signature = self.sign_request(request_timestamp)?;
        let mut headers = HashMap::new();
        headers.insert(SIGNATURE_HEADER.to_string(), signature);
        headers.insert(
            REQUEST_TIME_HEADER.to_string(),
            request_timestamp.to_string(),
        );
        headers.insert(USER_PUBKEY_HEADER.to_string(), self.pubkey_hex.clone());
        headers.insert(API_KEY_HEADER.to_string(), self.api_key.clone());
        Ok(headers)
    }
}

fn derive_signing_keys(
    seed: &[u8],
    network: bitcoin::Network,
) -> Result<(SecretKey, String), VssHeaderProviderError> {
    let seed_bytes: [u8; 64] =
        seed.try_into()
            .map_err(|_| VssHeaderProviderError::InternalError {
                error: "expected 64-byte seed".to_string(),
            })?;
    let secp = Secp256k1::new();
    let xprv = Xpriv::new_master(network, &seed_bytes).map_err(|e| {
        VssHeaderProviderError::InternalError {
            error: e.to_string(),
        }
    })?;
    let signing_xprv = xprv.derive_priv(
        &secp,
        &[ChildNumber::Hardened {
            index: VSS_SIGNING_CHILD_INDEX,
        }],
    );
    let signing_xprv = signing_xprv.map_err(|e| VssHeaderProviderError::InternalError {
        error: e.to_string(),
    })?;
    let private_key = signing_xprv.private_key;
    let pubkey = PublicKey::from_secret_key(&secp, &private_key);
    let pubkey_hex = pubkey.serialize().encode_hex::<String>();
    Ok((private_key, pubkey_hex))
}
