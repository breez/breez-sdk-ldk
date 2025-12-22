use bitcoin::hashes::sha256d::Hash as Sha256d;
use bitcoin::hashes::Hash;

pub(crate) const API_KEY_HEADER: &str = "X-Api-Key";
pub(crate) const REQUEST_TIME_HEADER: &str = "X-Realtimesync-Request-Time";
pub(crate) const SIGNATURE_HEADER: &str = "X-Realtimesync-Signature";
pub(crate) const USER_PUBKEY_HEADER: &str = "X-Realtimesync-Pubkey";

const SIGNED_MSG_PREFIX: &[u8] = b"realtimesync:";

pub(crate) fn build_signed_message(api_key: &str, request_timestamp: u32) -> [u8; 32] {
    let mut message = Vec::with_capacity(SIGNED_MSG_PREFIX.len() + 4 + api_key.len());
    message.extend_from_slice(SIGNED_MSG_PREFIX);
    message.extend_from_slice(&request_timestamp.to_be_bytes());
    message.extend_from_slice(api_key.as_bytes());
    let digest = Sha256d::hash(&message);
    digest.to_byte_array()
}
