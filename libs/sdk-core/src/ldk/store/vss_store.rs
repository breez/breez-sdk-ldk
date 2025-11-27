use bitcoin::hashes::{sha256, Hash, HashEngine, Hmac, HmacEngine};
use rand::RngCore;
use sdk_common::ensure_sdk;
use tonic::async_trait;
use vss_client_ng::client::VssClient;
use vss_client_ng::error::VssError;
use vss_client_ng::prost::Message;
use vss_client_ng::types::{
    DeleteObjectRequest, GetObjectRequest, GetObjectResponse, KeyValue, ListKeyVersionsRequest,
    PutObjectRequest, Storable,
};
use vss_client_ng::util::key_obfuscator::KeyObfuscator;
use vss_client_ng::util::retry::RetryPolicy;
use vss_client_ng::util::storable_builder::{EntropySource, StorableBuilder};

use crate::ldk::store::versioned_store::{Error, VersionedStore};

pub struct VssStore<P: RetryPolicy<E = VssError> + Send + Sync> {
    client: VssClient<P>,
    store_id: String,
    storable_builder: StorableBuilder<RandEntropySource>,
    key_obfuscator: KeyObfuscator,
    data_encryption_key: [u8; 32],
}

impl<P: RetryPolicy<E = VssError> + Send + Sync> VssStore<P> {
    pub fn new(client: VssClient<P>, store_id: String, vss_seed: [u8; 32]) -> Self {
        let (data_encryption_key, obfuscation_master_key) =
            derive_data_encryption_and_obfuscation_keys(&vss_seed);
        let key_obfuscator = KeyObfuscator::new(obfuscation_master_key);
        let storable_builder = StorableBuilder::new(RandEntropySource);

        Self {
            client,
            store_id,
            storable_builder,
            key_obfuscator,
            data_encryption_key,
        }
    }

    fn obfuscate_key(&self, key: &str) -> String {
        self.key_obfuscator.obfuscate(key)
    }

    fn deobfuscate_key(&self, key: &str) -> Result<String, Error> {
        self.key_obfuscator
            .deobfuscate(key)
            .map_err(|e| Error::Internal(format!("Failed to deobfuscate key: {e}")))
    }

    /// Persist the payload and commit to the version and the key to cross-check
    /// against the VSS metadata returned on reads.
    fn construct_storable(&self, key: &str, value: Vec<u8>, version: i64) -> Vec<u8> {
        self.storable_builder
            .build(
                value,
                version + 1, // VSS server will increment the version.
                &self.data_encryption_key,
                key.as_bytes(),
            )
            .encode_to_vec()
    }

    fn deconstruct_storable(&self, key: &str, bytes: &[u8]) -> Result<(Vec<u8>, i64), Error> {
        let key = self.obfuscate_key(key);
        let storable = Storable::decode(bytes).map_err(|e| {
            Error::Internal(format!(
                "Failed to decode encrypted value for key `{key}`: {e}"
            ))
        })?;
        self.storable_builder
            .deconstruct(storable, &self.data_encryption_key, key.as_bytes())
            .map_err(|e| Error::Internal(format!("Failed to decrypt value for key `{key}`: {e}")))
    }
}

#[async_trait]
impl<P: RetryPolicy<E = VssError> + Send + Sync> VersionedStore for VssStore<P> {
    async fn get(&self, key: String) -> Result<Option<(Vec<u8>, i64)>, Error> {
        let request = GetObjectRequest {
            store_id: self.store_id.clone(),
            key: self.obfuscate_key(&key),
        };

        match self.client.get_object(&request).await {
            Ok(GetObjectResponse { value: Some(kv) }) => {
                let (value, stored_version) = self.deconstruct_storable(&key, &kv.value)?;
                ensure_sdk!(stored_version == kv.version,
                    Error::Internal(format!(
                        "Version mismatch for key `{key}`: decrypted version={stored_version} but metadata version={}",
                        kv.version
                    )));
                Ok(Some((value, kv.version)))
            }
            Ok(GetObjectResponse { value: None }) => Ok(None),
            Err(VssError::NoSuchKeyError(_)) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    async fn put(&self, key: String, value: Vec<u8>, version: i64) -> Result<(), Error> {
        let key_value = KeyValue {
            key: self.obfuscate_key(&key),
            version,
            value: self.construct_storable(&key, value, version),
        };
        let request = PutObjectRequest {
            store_id: self.store_id.clone(),
            transaction_items: vec![key_value],
            ..Default::default()
        };

        self.client.put_object(&request).await?;
        Ok(())
    }

    async fn delete(&self, key: String) -> Result<(), Error> {
        let key_value = KeyValue {
            key: self.obfuscate_key(&key),
            version: -1,
            value: Vec::new(),
        };

        let request = DeleteObjectRequest {
            store_id: self.store_id.clone(),
            key_value: Some(key_value),
        };

        self.client.delete_object(&request).await?;
        Ok(())
    }

    async fn list(&self) -> Result<Vec<(String, i64)>, Error> {
        let mut request = ListKeyVersionsRequest {
            store_id: self.store_id.clone(),
            ..Default::default()
        };
        let mut versions = Vec::new();
        loop {
            let mut response = self.client.list_key_versions(&request).await?;
            versions.append(&mut response.key_versions);
            if response
                .next_page_token
                .as_deref()
                .unwrap_or_default()
                .is_empty()
            {
                break;
            }
            request.page_token = response.next_page_token;
        }

        let versions = versions
            .into_iter()
            .map(|kv| {
                let key = self.deobfuscate_key(&kv.key)?;
                Ok((key, kv.version))
            })
            .collect::<Result<Vec<_>, Error>>()?;
        Ok(versions)
    }
}

// Copied from https://github.com/lightningdevkit/ldk-node/blob/37045f4708a0721f14bcebe704803418c7c15203/src/io/vss_store.rs#L670
fn derive_data_encryption_and_obfuscation_keys(vss_seed: &[u8; 32]) -> ([u8; 32], [u8; 32]) {
    let hkdf = |initial_key_material: &[u8], salt: &[u8]| -> [u8; 32] {
        let mut engine = HmacEngine::<sha256::Hash>::new(salt);
        engine.input(initial_key_material);
        Hmac::from_engine(engine).to_byte_array()
    };

    let prk = hkdf(vss_seed, b"pseudo_random_key");
    let k1 = hkdf(&prk, b"data_encryption_key");
    let k2 = hkdf(&prk, &[&k1[..], b"obfuscation_key"].concat());
    (k1, k2)
}

struct RandEntropySource;

impl EntropySource for RandEntropySource {
    fn fill_bytes(&self, buffer: &mut [u8]) {
        rand::thread_rng().fill_bytes(buffer);
    }
}

impl From<VssError> for Error {
    fn from(err: VssError) -> Self {
        match err {
            VssError::NoSuchKeyError(_) => {
                Error::Internal("Received VssError::NoSuchKeyError".to_string())
            }
            VssError::ConflictError(e) => Error::Conflict(e),
            _ => Error::Internal(format!("{err:?}")),
        }
    }
}
