use std::collections::HashMap;
use std::ops::Deref;
use std::sync::{Arc, Mutex};

use ldk_node::bitcoin::io::ErrorKind;
use ldk_node::lightning::io;
use ldk_node::lightning::util::async_poll::AsyncResult;
use ldk_node::lightning::util::persist::{KVStore, KVStoreSync};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{params, Connection, Error as SqlError, OptionalExtension};
use tokio::runtime::Handle;

use crate::ldk::store::time_lock::PreviousHolder;
use crate::ldk::store::versioned_store::{Error as RemoteError, VersionedStore};
use crate::node_api::NodeError;
use crate::persist::error::PersistError;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Local pool error: {0}")]
    LocalPool(#[from] r2d2::Error),
    #[error("Local sql error: {0}")]
    LocalSql(#[from] SqlError),
    #[error("Remote error: {0}")]
    Remote(#[from] RemoteError),
}

impl From<Error> for NodeError {
    fn from(err: Error) -> Self {
        match err {
            Error::LocalPool(e) => {
                PersistError::Sql(format!("Mirroring store local pool error: {e}")).into()
            }
            Error::LocalSql(e) => {
                PersistError::Sql(format!("Mirroring store local sql error: {e}")).into()
            }
            Error::Remote(e) => {
                NodeError::ServiceConnectivity(format!("Mirroring store remote error: {e}"))
            }
        }
    }
}

pub struct MirroringStore<S: Deref<Target = T>, T: VersionedStore + Send + Sync> {
    handle: Handle,
    remote_client: S,
    pool: Pool<SqliteConnectionManager>,
    key_locks: Mutex<HashMap<String, Arc<Mutex<()>>>>,
}

impl<S: Deref<Target = T>, T: VersionedStore + Send + Sync> MirroringStore<S, T> {
    pub async fn new(
        handle: Handle,
        pool: Pool<SqliteConnectionManager>,
        remote: S,
        previous_holder: PreviousHolder,
    ) -> Result<Self, Error> {
        let conn = &*pool.get()?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS store (
                primary_ns TEXT NOT NULL,
                secondary_ns TEXT NOT NULL,
                key TEXT NOT NULL,
                value BLOB NOT NULL,
                local_version INTEGER NOT NULL,
                remote_version INTEGER NOT NULL DEFAULT -1,
                removed INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (primary_ns, secondary_ns, key)
            )",
            [],
        )?;

        let is_dirty = is_dirty(conn)?;
        match (previous_holder, is_dirty) {
            (PreviousHolder::LocalInstance, false) => {
                info!("Local store is clean, nothing new on remote. Skipping reconciliation.");
            }
            (PreviousHolder::LocalInstance, true) => {
                info!("Local store is *dirty*, nothing new on remote. Uploading to remote...");
                upload(conn, &*remote).await?;
            }
            (PreviousHolder::RemoteInstance, false) => {
                info!("Local store is clean, something new on remote possible. Downloading from remote...");
                download(conn, &*remote).await?;
            }
            (PreviousHolder::RemoteInstance, true) => {
                info!("Local store is *dirty*, something new on remote possible. Downloading from remote...");
                download(conn, &*remote).await?;
            }
        };

        Ok(Self {
            handle,
            pool,
            remote_client: remote,
            key_locks: Default::default(),
        })
    }

    fn key_lock(&self, full_key: String) -> Arc<Mutex<()>> {
        let mut locks = self.key_locks.lock().unwrap();
        Arc::clone(locks.entry(full_key).or_default())
    }
}

impl<S: Deref<Target = T>, T: VersionedStore + Send + Sync> KVStoreSync for MirroringStore<S, T> {
    fn read(&self, primary_ns: &str, secondary_ns: &str, key: &str) -> io::Result<Vec<u8>> {
        let conn = self.pool.get().map_err(other)?;
        conn.query_row(
            "SELECT value FROM store WHERE primary_ns = ?1 AND secondary_ns = ?2 AND key = ?3 AND removed = 0",
            params![primary_ns, secondary_ns, key],
            |row| row.get(0),
        )
        .optional()
        .map_err(other)?
        .ok_or(io::Error::new(ErrorKind::NotFound, "Not Found"))
    }

    fn write(
        &self,
        primary_ns: &str,
        secondary_ns: &str,
        key: &str,
        value: Vec<u8>,
    ) -> io::Result<()> {
        let full_key = format!("{primary_ns}/{secondary_ns}/{key}");
        let mutex = self.key_lock(full_key.clone());
        let _lock = mutex.lock().unwrap();

        debug!("Writing {full_key} {} bytes", value.len());
        let conn = self.pool.get().map_err(other)?;

        let local_data: Option<(i64, Vec<u8>, bool)> = conn
            .query_row(
                "SELECT local_version, value, removed FROM store WHERE primary_ns = ?1 AND secondary_ns = ?2 AND key = ?3",
                params![primary_ns, secondary_ns, key],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .map_err(other)?;
        let next_version = match local_data {
            None => {
                let next_version = 0;
                conn.execute(
                    "INSERT INTO store (primary_ns, secondary_ns, key, value, local_version, remote_version, removed) VALUES (?1, ?2, ?3, ?4, ?5, -1, 0)",
                    params![primary_ns, secondary_ns, key, value, next_version],
                ).map_err(other)?;
                next_version
            }
            Some((_local_version, local_value, false)) if local_value == value => {
                trace!("Local value is the same, skipping writing");
                return Ok(());
            }
            Some((local_version, _local_value, _removed)) => {
                trace!("Local value is different, writing");
                let next_version = local_version + 1;
                conn.execute(
                    "UPDATE store SET value = ?1, local_version = ?2, removed = 0 WHERE primary_ns = ?3 AND secondary_ns = ?4 AND key = ?5",
                    params![value, next_version, primary_ns, secondary_ns, key],
                ).map_err(other)?;
                next_version
            }
        };

        tokio::task::block_in_place(|| {
            self.handle.block_on(self.remote_client.put(
                full_key.clone(),
                value.to_vec(),
                next_version,
            ))
        })
        .map_err(other)?;

        conn.execute(
            "UPDATE store SET remote_version = local_version WHERE primary_ns = ?1 AND secondary_ns = ?2 AND key = ?3",
            params![primary_ns, secondary_ns, key],
        ).map_err(other)?;

        debug!("Wrote {full_key}");
        Ok(())
    }

    fn remove(
        &self,
        primary_ns: &str,
        secondary_ns: &str,
        key: &str,
        _lazy: bool,
    ) -> io::Result<()> {
        let full_key = format!("{primary_ns}/{secondary_ns}/{key}");
        let mutex = self.key_lock(full_key.clone());
        let _lock = mutex.lock().unwrap();
        debug!("Removing {full_key}");

        let conn = self.pool.get().map_err(other)?;

        conn.execute(
            "UPDATE store SET removed = 1 WHERE primary_ns = ?1 AND secondary_ns = ?2 AND key = ?3",
            params![primary_ns, secondary_ns, key],
        )
        .map_err(other)?;

        tokio::task::block_in_place(|| {
            self.handle
                .block_on(self.remote_client.delete(full_key.clone()))
        })
        .map_err(other)?;

        conn.execute(
            "DELETE FROM store WHERE primary_ns = ?1 AND secondary_ns = ?2 AND key = ?3",
            params![primary_ns, secondary_ns, key],
        )
        .map_err(other)?;

        debug!("Removed {full_key}");
        Ok(())
    }

    fn list(&self, primary_ns: &str, secondary_ns: &str) -> io::Result<Vec<String>> {
        self.pool
            .get()
            .map_err(other)?
            .prepare("SELECT key FROM store WHERE primary_ns = ?1 AND secondary_ns = ?2 AND removed = 0 ORDER BY primary_ns, secondary_ns, key")
            .map_err(other)?
            .query_map(params![primary_ns, secondary_ns], |row| row.get(0))
            .map_err(other)?
            .collect::<Result<Vec<String>, _>>()
            .map_err(other)
    }
}

fn is_dirty(conn: &Connection) -> rusqlite::Result<bool> {
    let dirty_rows: i64 = conn.query_row(
        "SELECT count(1) FROM store WHERE local_version != remote_version OR removed = 1",
        [],
        |row| row.get(0),
    )?;
    Ok(dirty_rows > 0)
}

async fn download<S: VersionedStore>(conn: &Connection, remote: &S) -> Result<(), Error> {
    conn.execute("DELETE FROM store", [])?;

    for (full_key, version) in remote.list().await? {
        trace!("Downloading {full_key} @ {version} ...");
        let parts: Vec<&str> = full_key.splitn(3, '/').collect();
        let (primary, secondary, key) = match &parts[..] {
            [p, s, k] => (p.to_string(), s.to_string(), k.to_string()),
            _ => continue, // skip malformed keys
        };

        if let Some((value, version)) = remote.get(full_key).await? {
            trace!("Got {} bytes @ {version}", value.len());
            conn.execute(
                "INSERT INTO store (primary_ns, secondary_ns, key, value, local_version, remote_version, removed) VALUES (?1, ?2, ?3, ?4, ?5, ?5, 0)",
                params![primary, secondary, key, value, version - 1],
            )?;
        }
    }
    Ok(())
}

async fn upload<S: VersionedStore>(conn: &Connection, remote: &S) -> Result<(), Error> {
    // First, process deletions (tombstoned rows).
    {
        let mut statement =
            conn.prepare("SELECT primary_ns, secondary_ns, key FROM store WHERE removed = 1")?;
        let deleted_rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;

        for row in deleted_rows {
            let (primary_ns, secondary_ns, key) = row?;
            let full_key = format!("{primary_ns}/{secondary_ns}/{key}");
            trace!("Deleting remotely {full_key} ...");
            remote.delete(full_key).await?;
            conn.execute(
                "DELETE FROM store WHERE primary_ns = ?1 AND secondary_ns = ?2 AND key = ?3",
                params![primary_ns, secondary_ns, key],
            )?;
        }
    }

    // Then, upload modified values.
    let mut statement = conn.prepare(
        "SELECT primary_ns, secondary_ns, key, value, local_version FROM store WHERE local_version != remote_version AND removed = 0",
    )?;
    let outdated_rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Vec<u8>>(3)?,
            row.get::<_, i64>(4)?,
        ))
    })?;

    for row in outdated_rows {
        let (primary_ns, secondary_ns, key, value, local_version) = row?;
        let full_key = format!("{primary_ns}/{secondary_ns}/{key}");
        trace!("Uploading {full_key} @ {local_version} ...");
        remote.put(full_key, value, local_version).await?;

        conn.execute(
            "UPDATE store SET remote_version = local_version WHERE primary_ns = ?1 AND secondary_ns = ?2 AND key = ?3",
            params![primary_ns, secondary_ns, key],
        )?;
    }
    Ok(())
}

fn other<E>(err: E) -> io::Error
where
    E: Into<Box<dyn std::error::Error + Send + Sync + 'static>>,
{
    io::Error::new(ErrorKind::Other, err)
}

impl<S: Deref<Target = T>, T: VersionedStore + Send + Sync> KVStore for MirroringStore<S, T> {
    fn read(
        &self,
        primary_ns: &str,
        secondary_ns: &str,
        key: &str,
    ) -> AsyncResult<'static, Vec<u8>, io::Error> {
        let result = KVStoreSync::read(self, primary_ns, secondary_ns, key);
        Box::pin(async move { result })
    }

    fn write(
        &self,
        primary_ns: &str,
        secondary_ns: &str,
        key: &str,
        buf: Vec<u8>,
    ) -> AsyncResult<'static, (), io::Error> {
        let result = KVStoreSync::write(self, primary_ns, secondary_ns, key, buf);
        Box::pin(async move { result })
    }

    fn remove(
        &self,
        primary_ns: &str,
        secondary_ns: &str,
        key: &str,
        lazy: bool,
    ) -> AsyncResult<'static, (), io::Error> {
        let result = KVStoreSync::remove(self, primary_ns, secondary_ns, key, lazy);
        Box::pin(async move { result })
    }

    fn list(
        &self,
        primary_ns: &str,
        secondary_ns: &str,
    ) -> AsyncResult<'static, Vec<String>, io::Error> {
        let result = KVStoreSync::list(self, primary_ns, secondary_ns);
        Box::pin(async move { result })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ldk::store::mock_versioned_store::MockVersionedStore;
    use crate::ldk::store::time_lock::PreviousHolder;
    use r2d2_sqlite::SqliteConnectionManager;
    use rusqlite::backup::Backup;
    use rusqlite::Connection;
    use std::time::Duration;
    use tokio::runtime::Handle;

    fn create_in_memory_db() -> Pool<SqliteConnectionManager> {
        let manager = SqliteConnectionManager::memory();
        Pool::new(manager).unwrap()
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_mirroring_store_normal_flow() {
        let mock_store = MockVersionedStore::default();
        let store = MirroringStore::new(
            Handle::current().clone(),
            create_in_memory_db(),
            &mock_store,
            PreviousHolder::RemoteInstance,
        )
        .await
        .unwrap();

        // Write, read, list values, remove.
        let list = KVStoreSync::list(&store, "ns", "sub").unwrap();
        assert!(list.is_empty());

        KVStoreSync::write(&store, "ns", "sub", "key", b"value".to_vec()).unwrap();
        KVStoreSync::write(
            &store,
            "ns",
            "sub",
            "to_remove",
            b"to_remove_value".to_vec(),
        )
        .unwrap();
        KVStoreSync::remove(&store, "ns", "sub", "to_remove", false).unwrap();
        KVStoreSync::remove(&store, "ns", "sub", "does_not_exist", false).unwrap();

        let list = KVStoreSync::list(&store, "ns", "sub").unwrap();
        assert_eq!(list, vec!["key".to_string()]);
        let value = KVStoreSync::read(&store, "ns", "sub", "key").unwrap();
        assert_eq!(value, b"value");

        // Load a new instance.
        let store = MirroringStore::new(
            Handle::current().clone(),
            create_in_memory_db(),
            &mock_store,
            PreviousHolder::RemoteInstance,
        )
        .await
        .unwrap();
        // Data was loaded from remote.
        let list = KVStoreSync::list(&store, "ns", "sub").unwrap();
        assert_eq!(list, vec!["key".to_string()]);
        let value = KVStoreSync::read(&store, "ns", "sub", "key").unwrap();
        assert_eq!(value, b"value");

        // Update the value, write a new value..
        KVStoreSync::write(&store, "ns", "sub", "key", b"value2".to_vec()).unwrap();
        KVStoreSync::write(&store, "ns2", "sub2", "key2", b"value22".to_vec()).unwrap();
        let list = KVStoreSync::list(&store, "ns", "sub").unwrap();
        assert_eq!(list, vec!["key".to_string()]);
        let value = KVStoreSync::read(&store, "ns", "sub", "key").unwrap();
        assert_eq!(value, b"value2");
        let value = KVStoreSync::read(&store, "ns2", "sub2", "key2").unwrap();
        assert_eq!(value, b"value22");

        // No removed key.
        let err = KVStoreSync::read(&store, "ns", "sub", "to_remove").unwrap_err();
        assert_eq!(err.kind(), ErrorKind::NotFound);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_mirroring_store_remote_failure_handling() {
        // Simulate remote failure.
        let mut mock_store = MockVersionedStore {
            should_fail_put: true,
            ..Default::default()
        };

        let store = MirroringStore::new(
            Handle::current().clone(),
            create_in_memory_db(),
            &mock_store,
            PreviousHolder::LocalInstance,
        )
        .await
        .unwrap();

        // Try to write - should fail due to remote error.
        let err = KVStoreSync::write(&store, "ns", "sub", "key_dirty", b"value_dirty".to_vec())
            .unwrap_err();
        assert_eq!(err.kind(), ErrorKind::Other);
        // Dirty data is stored locally, though.
        let value = KVStoreSync::read(&store, "ns", "sub", "key_dirty").unwrap();
        assert_eq!(value, b"value_dirty");

        {
            // A new instance does not load this information.
            let store = MirroringStore::new(
                Handle::current().clone(),
                create_in_memory_db(),
                &mock_store,
                PreviousHolder::RemoteInstance,
            )
            .await
            .unwrap();
            let err = KVStoreSync::read(&store, "ns", "sub", "key_dirty").unwrap_err();
            assert_eq!(err.kind(), ErrorKind::NotFound);
        }

        {
            // Recovery of a dirty instance with another instance accessing the
            // store in between.
            let dirty_local_db = create_in_memory_db();
            clone_data(
                &store.pool.get().unwrap(),
                &mut dirty_local_db.get().unwrap(),
            );

            let store = MirroringStore::new(
                Handle::current().clone(),
                dirty_local_db,
                &mock_store,
                PreviousHolder::RemoteInstance,
            )
            .await
            .unwrap();
            let err = KVStoreSync::read(&store, "ns", "sub", "key_dirty").unwrap_err();
            assert_eq!(err.kind(), ErrorKind::NotFound);
        }

        {
            // Recovery of a dirty instance with *no* other instances accessing
            // the store in between.
            let dirty_local_db = create_in_memory_db();
            clone_data(
                &store.pool.get().unwrap(),
                &mut dirty_local_db.get().unwrap(),
            );
            mock_store.should_fail_put = false;

            let store = MirroringStore::new(
                Handle::current().clone(),
                dirty_local_db,
                &mock_store,
                PreviousHolder::LocalInstance,
            )
            .await
            .unwrap();
            let value = KVStoreSync::read(&store, "ns", "sub", "key_dirty").unwrap();
            assert_eq!(value, b"value_dirty");
            // Data was uploaded to remote.
            let data = mock_store.data.lock().unwrap();
            let value = data.get("ns/sub/key_dirty").unwrap().0.clone();
            assert_eq!(value, b"value_dirty");
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_mirroring_store_remote_failure_handling_remove() {
        // Simulate remote failure.
        let mut mock_store = MockVersionedStore {
            should_fail_delete: true,
            ..Default::default()
        };

        let dirty_local_db = {
            let store = MirroringStore::new(
                Handle::current().clone(),
                create_in_memory_db(),
                &mock_store,
                PreviousHolder::LocalInstance,
            )
            .await
            .unwrap();

            KVStoreSync::write(&store, "ns", "sub", "key_to_remove", b"remove_me".to_vec())
                .unwrap();
            let value = KVStoreSync::read(&store, "ns", "sub", "key_to_remove").unwrap();
            assert_eq!(value, b"remove_me");

            // Simulate remote delete failure.
            let err = KVStoreSync::remove(&store, "ns", "sub", "key_to_remove", false).unwrap_err();
            assert_eq!(err.kind(), ErrorKind::Other);

            // Locally, the key is tombstoned: not listed, not readable.
            let list = KVStoreSync::list(&store, "ns", "sub").unwrap();
            assert!(!list.contains(&"key_to_remove".to_string()));
            let err = KVStoreSync::read(&store, "ns", "sub", "key_to_remove").unwrap_err();
            assert_eq!(err.kind(), ErrorKind::NotFound);
            let dirty_local_db = create_in_memory_db();
            clone_data(
                &store.pool.get().unwrap(),
                &mut dirty_local_db.get().unwrap(),
            );
            dirty_local_db
        };

        {
            // A new instance sees the key still present, because remote deletion failed.
            let store_remote_first = MirroringStore::new(
                Handle::current().clone(),
                create_in_memory_db(),
                &mock_store,
                PreviousHolder::RemoteInstance,
            )
            .await
            .unwrap();
            let list_remote = KVStoreSync::list(&store_remote_first, "ns", "sub").unwrap();
            assert!(list_remote.contains(&"key_to_remove".to_string()));
            let value_remote =
                KVStoreSync::read(&store_remote_first, "ns", "sub", "key_to_remove").unwrap();
            assert_eq!(value_remote, b"remove_me");
        }

        {
            // Recovery of a dirty instance with *no* other instances accessing
            // the store in between.
            mock_store.should_fail_delete = false;
            let store_cleanup = MirroringStore::new(
                Handle::current().clone(),
                dirty_local_db,
                &mock_store,
                PreviousHolder::LocalInstance,
            )
            .await
            .unwrap();

            // After reconciliation, the key should be gone from remote and local.
            let data = mock_store.data.lock().unwrap();
            assert!(!data.contains_key("ns/sub/key_to_remove"));
            drop(data);

            let err = KVStoreSync::read(&store_cleanup, "ns", "sub", "key_to_remove").unwrap_err();
            assert_eq!(err.kind(), ErrorKind::NotFound);
            let list = KVStoreSync::list(&store_cleanup, "ns", "sub").unwrap();
            assert!(!list.contains(&"key_to_remove".to_string()));
        }
    }

    fn clone_data(src: &Connection, dst: &mut Connection) {
        Backup::new(src, dst)
            .unwrap()
            .run_to_completion(5, Duration::default(), None)
            .unwrap()
    }
}
