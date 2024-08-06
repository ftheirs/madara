//! Deoxys database

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::{fmt, fs};

use anyhow::{Context, Result};
use block_db::ChainInfo;
use bonsai_db::{BonsaiDb, DatabaseKeyMapping};
use bonsai_trie::id::BasicId;
use bonsai_trie::{BonsaiStorage, BonsaiStorageConfig};
use db_metrics::DbMetrics;
use rocksdb::backup::{BackupEngine, BackupEngineOptions};

pub mod block_db;
mod codec;
mod error;
use rocksdb::{
    BoundColumnFamily, ColumnFamilyDescriptor, DBCompressionType, DBWithThreadMode, Env, FlushOptions, MultiThreaded,
    Options, SliceTransform,
};
pub mod bonsai_db;
pub mod class_db;
pub mod contract_db;
pub mod db_block_id;
pub mod db_metrics;
pub mod storage_updates;

pub use error::{DeoxysStorageError, TrieType};
use starknet_types_core::hash::{Pedersen, Poseidon, StarkHash};
use tokio::sync::{mpsc, oneshot};

pub type DB = DBWithThreadMode<MultiThreaded>;

pub use rocksdb;
pub type WriteBatchWithTransaction = rocksdb::WriteBatchWithTransaction<false>;

const DB_UPDATES_BATCH_SIZE: usize = 1024;

pub(crate) async fn open_rocksdb(
    path: &Path,
    create: bool,
    backup_dir: Option<PathBuf>,
    restore_from_latest_backup: bool,
) -> Result<(Arc<DB>, Option<mpsc::Sender<BackupRequest>>)> {
    let mut opts = Options::default();
    opts.set_report_bg_io_stats(true);
    opts.set_use_fsync(false);
    opts.create_if_missing(create);
    opts.create_missing_column_families(true);
    opts.set_bytes_per_sync(1024 * 1024);
    opts.set_keep_log_file_num(1);
    opts.optimize_level_style_compaction(4096 * 1024 * 1024);
    opts.set_compression_type(DBCompressionType::Zstd);
    let cores = std::thread::available_parallelism().map(|e| e.get() as i32).unwrap_or(1);
    opts.increase_parallelism(cores);

    opts.set_atomic_flush(true);
    opts.set_manual_wal_flush(true);
    opts.set_max_subcompactions(cores as _);

    let mut env = Env::new().context("Creating rocksdb env")?;
    // env.set_high_priority_background_threads(cores); // flushes
    env.set_low_priority_background_threads(cores); // compaction

    opts.set_env(&env);

    let backup_hande = if let Some(backup_dir) = backup_dir {
        let (restored_cb_sender, restored_cb_recv) = oneshot::channel();

        let (sender, receiver) = mpsc::channel(1);
        let db_path = path.to_owned();
        std::thread::spawn(move || {
            spawn_backup_db_task(&backup_dir, restore_from_latest_backup, &db_path, restored_cb_sender, receiver)
                .expect("Database backup thread")
        });

        log::debug!("blocking on db restoration");
        restored_cb_recv.await.context("Restoring database")?;
        log::debug!("done blocking on db restoration");

        Some(sender)
    } else {
        None
    };

    log::debug!("opening db at {:?}", path.display());
    let db = DB::open_cf_descriptors(
        &opts,
        path,
        Column::ALL.iter().map(|col| ColumnFamilyDescriptor::new(col.rocksdb_name(), col.rocksdb_options())),
    )?;

    Ok((Arc::new(db), backup_hande))
}

/// This runs in anothr thread as the backup engine is not thread safe
fn spawn_backup_db_task(
    backup_dir: &Path,
    restore_from_latest_backup: bool,
    db_path: &Path,
    db_restored_cb: oneshot::Sender<()>,
    mut recv: mpsc::Receiver<BackupRequest>,
) -> Result<()> {
    let mut backup_opts = BackupEngineOptions::new(backup_dir).context("Creating backup options")?;
    let cores = std::thread::available_parallelism().map(|e| e.get() as i32).unwrap_or(1);
    backup_opts.set_max_background_operations(cores);

    let mut engine = BackupEngine::open(&backup_opts, &Env::new().context("Creating rocksdb env")?)
        .context("Opening backup engine")?;

    if restore_from_latest_backup {
        log::info!("⏳ Restoring latest backup...");
        log::debug!("restore path is {db_path:?}");
        fs::create_dir_all(db_path).with_context(|| format!("creating directories {:?}", db_path))?;

        let opts = rocksdb::backup::RestoreOptions::default();
        engine.restore_from_latest_backup(db_path, db_path, &opts).context("Restoring database")?;
        log::debug!("restoring latest backup done");
    }

    db_restored_cb.send(()).ok().context("Receiver dropped")?;

    while let Some(BackupRequest { callback, db }) = recv.blocking_recv() {
        engine.create_new_backup_flush(&db, true).context("Creating rocksdb backup")?;
        let _ = callback.send(());
    }

    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Column {
    Meta,

    // Blocks storage
    // block_n => Block info
    BlockNToBlockInfo,
    // block_n => Block inner
    BlockNToBlockInner,
    /// Many To One
    TxHashToBlockN,
    /// One To One
    BlockHashToBlockN,
    /// One To One
    BlockNToStateDiff,
    /// Meta column for block storage (sync tip, pending block)
    BlockStorageMeta,

    /// Contract class hash to class data
    ClassInfo,
    ClassCompiled,
    PendingClassInfo,
    PendingClassCompiled,

    // History of contract class hashes
    // contract_address history block_number => class_hash
    ContractToClassHashes,

    // History of contract nonces
    // contract_address history block_number => nonce
    ContractToNonces,

    // Class hash => compiled class hash
    ContractClassHashes,

    // Pending columns for contract db
    PendingContractToClassHashes,
    PendingContractToNonces,
    PendingContractStorage,

    // History of contract key => values
    // (contract_address, storage_key) history block_number => felt
    ContractStorage,
    /// Block number to state diff
    BlockStateDiff,

    // Each bonsai storage has 3 columns
    BonsaiContractsTrie,
    BonsaiContractsFlat,
    BonsaiContractsLog,

    BonsaiContractsStorageTrie,
    BonsaiContractsStorageFlat,
    BonsaiContractsStorageLog,

    BonsaiClassesTrie,
    BonsaiClassesFlat,
    BonsaiClassesLog,
}

impl fmt::Debug for Column {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.rocksdb_name())
    }
}

impl fmt::Display for Column {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.rocksdb_name())
    }
}

impl Column {
    pub const ALL: &'static [Self] = {
        use Column::*;
        &[
            Meta,
            BlockNToBlockInfo,
            BlockNToBlockInner,
            TxHashToBlockN,
            BlockHashToBlockN,
            BlockStorageMeta,
            BlockNToStateDiff,
            ClassInfo,
            ClassCompiled,
            PendingClassInfo,
            PendingClassCompiled,
            ContractToClassHashes,
            ContractToNonces,
            ContractClassHashes,
            ContractStorage,
            BlockStateDiff,
            BonsaiContractsTrie,
            BonsaiContractsFlat,
            BonsaiContractsLog,
            BonsaiContractsStorageTrie,
            BonsaiContractsStorageFlat,
            BonsaiContractsStorageLog,
            BonsaiClassesTrie,
            BonsaiClassesFlat,
            BonsaiClassesLog,
            PendingContractToClassHashes,
            PendingContractToNonces,
            PendingContractStorage,
        ]
    };
    pub const NUM_COLUMNS: usize = Self::ALL.len();

    pub(crate) fn rocksdb_name(&self) -> &'static str {
        use Column::*;
        match self {
            Meta => "meta",
            BlockNToBlockInfo => "block_n_to_block_info",
            BlockNToBlockInner => "block_n_to_block_inner",
            TxHashToBlockN => "tx_hash_to_block_n",
            BlockHashToBlockN => "block_hash_to_block_n",
            BlockStorageMeta => "block_storage_meta",
            BlockNToStateDiff => "block_n_to_state_diff",
            BonsaiContractsTrie => "bonsai_contracts_trie",
            BonsaiContractsFlat => "bonsai_contracts_flat",
            BonsaiContractsLog => "bonsai_contracts_log",
            BonsaiContractsStorageTrie => "bonsai_contracts_storage_trie",
            BonsaiContractsStorageFlat => "bonsai_contracts_storage_flat",
            BonsaiContractsStorageLog => "bonsai_contracts_storage_log",
            BonsaiClassesTrie => "bonsai_classes_trie",
            BonsaiClassesFlat => "bonsai_classes_flat",
            BonsaiClassesLog => "bonsai_classes_log",
            BlockStateDiff => "block_state_diff",
            ClassInfo => "class_info",
            ClassCompiled => "class_compiled",
            PendingClassInfo => "pending_class_info",
            PendingClassCompiled => "pending_class_compiled",
            ContractToClassHashes => "contract_to_class_hashes",
            ContractToNonces => "contract_to_nonces",
            ContractClassHashes => "contract_class_hashes",
            ContractStorage => "contract_storage",
            PendingContractToClassHashes => "pending_contract_to_class_hashes",
            PendingContractToNonces => "pending_contract_to_nonces",
            PendingContractStorage => "pending_contract_storage",
        }
    }

    /// Per column rocksdb options, like memory budget, compaction profiles, block sizes for hdd/sdd
    /// etc. TODO: add basic sensible defaults
    pub(crate) fn rocksdb_options(&self) -> Options {
        let mut opts = Options::default();
        match self {
            Column::ContractStorage => {
                opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(
                    contract_db::CONTRACT_STORAGE_PREFIX_EXTRACTOR,
                ));
            }
            Column::ContractToClassHashes => {
                opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(
                    contract_db::CONTRACT_CLASS_HASH_PREFIX_EXTRACTOR,
                ));
            }
            Column::ContractToNonces => {
                opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(
                    contract_db::CONTRACT_NONCES_PREFIX_EXTRACTOR,
                ));
            }
            _ => {}
        }
        opts
    }
}

pub trait DatabaseExt {
    fn get_column(&self, col: Column) -> Arc<BoundColumnFamily<'_>>;
}

impl DatabaseExt for DB {
    fn get_column(&self, col: Column) -> Arc<BoundColumnFamily<'_>> {
        let name = col.rocksdb_name();
        match self.cf_handle(name) {
            Some(column) => column,
            None => panic!("column {name} not initialized"),
        }
    }
}

/// Deoxys client database backend singleton.
#[derive(Debug)]
pub struct DeoxysBackend {
    backup_handle: Option<mpsc::Sender<BackupRequest>>,
    db: Arc<DB>,
    last_flush_time: Mutex<Option<Instant>>,
}

pub struct DatabaseService {
    handle: Arc<DeoxysBackend>,
}

impl DatabaseService {
    pub async fn new(
        base_path: &Path,
        backup_dir: Option<PathBuf>,
        restore_from_latest_backup: bool,
        chain_info: &ChainInfo,
    ) -> anyhow::Result<Self> {
        log::info!("💾 Opening database at: {}", base_path.display());

        let handle =
            DeoxysBackend::open(base_path.to_owned(), backup_dir.clone(), restore_from_latest_backup, chain_info)
                .await?;

        Ok(Self { handle })
    }

    pub fn backend(&self) -> &Arc<DeoxysBackend> {
        &self.handle
    }
}

struct BackupRequest {
    callback: oneshot::Sender<()>,
    db: Arc<DB>,
}

impl Drop for DeoxysBackend {
    fn drop(&mut self) {
        log::info!("⏳ Gracefully closing the database...");
    }
}

impl DeoxysBackend {
    /// Open the db.
    async fn open(
        db_config_dir: PathBuf,
        backup_dir: Option<PathBuf>,
        restore_from_latest_backup: bool,
        chain_info: &ChainInfo,
    ) -> Result<Arc<DeoxysBackend>> {
        let db_path = db_config_dir.join("db");

        let (db, backup_handle) = open_rocksdb(&db_path, true, backup_dir, restore_from_latest_backup).await?;

        let backend = Arc::new(Self { backup_handle, db, last_flush_time: Default::default() });
        backend.assert_chain_info(chain_info)?;
        Ok(backend)
    }

    pub fn maybe_flush(&self, force: bool) -> Result<bool> {
        let mut inst = self.last_flush_time.lock().expect("poisoned mutex");
        let should_flush = force
            || match *inst {
                Some(inst) => inst.elapsed() >= Duration::from_secs(5),
                None => true,
            };
        if should_flush {
            log::debug!("doing a db flush");
            let mut opts = FlushOptions::default();
            opts.set_wait(true);
            // we have to collect twice here :/
            let columns = Column::ALL.iter().map(|e| self.db.get_column(*e)).collect::<Vec<_>>();
            let columns = columns.iter().collect::<Vec<_>>();
            self.db.flush_cfs_opt(&columns, &opts).context("Flushing database")?;

            *inst = Some(Instant::now());
        }

        Ok(should_flush)
    }

    pub async fn backup(&self) -> Result<()> {
        let (callback_sender, callback_recv) = oneshot::channel();
        let _res = self
            .backup_handle
            .as_ref()
            .context("backups are not enabled")?
            .try_send(BackupRequest { callback: callback_sender, db: Arc::clone(&self.db) });
        callback_recv.await.context("Backups task died :(")?;
        Ok(())
    }

    // tries

    pub(crate) fn get_bonsai<H: StarkHash + Send + Sync>(
        &self,
        map: DatabaseKeyMapping,
    ) -> BonsaiStorage<BasicId, BonsaiDb<'_>, H> {
        let bonsai = BonsaiStorage::new(
            BonsaiDb::new(&self.db, map),
            BonsaiStorageConfig {
                max_saved_trie_logs: Some(0),
                max_saved_snapshots: Some(0),
                snapshot_interval: u64::MAX,
            },
        )
        // UNWRAP: function actually cannot panic
        .unwrap();

        bonsai
    }

    pub fn contract_trie(&self) -> BonsaiStorage<BasicId, BonsaiDb<'_>, Pedersen> {
        self.get_bonsai(DatabaseKeyMapping {
            flat: Column::BonsaiContractsFlat,
            trie: Column::BonsaiContractsTrie,
            log: Column::BonsaiContractsLog,
        })
    }

    pub fn contract_storage_trie(&self) -> BonsaiStorage<BasicId, BonsaiDb<'_>, Pedersen> {
        self.get_bonsai(DatabaseKeyMapping {
            flat: Column::BonsaiContractsStorageFlat,
            trie: Column::BonsaiContractsStorageTrie,
            log: Column::BonsaiContractsStorageLog,
        })
    }

    pub fn class_trie(&self) -> BonsaiStorage<BasicId, BonsaiDb<'_>, Poseidon> {
        self.get_bonsai(DatabaseKeyMapping {
            flat: Column::BonsaiClassesFlat,
            trie: Column::BonsaiClassesTrie,
            log: Column::BonsaiClassesLog,
        })
    }

    pub fn get_storage_size(&self, db_metrics: &DbMetrics) -> u64 {
        let mut storage_size = 0;

        for &column in Column::ALL.iter() {
            let cf_handle = self.db.get_column(column);
            let cf_metadata = self.db.get_column_family_metadata_cf(&cf_handle);
            let column_size = cf_metadata.size;
            storage_size += column_size;

            db_metrics.column_sizes.with_label_values(&[column.rocksdb_name()]).set(column_size as i64);
        }

        storage_size
    }

    #[cfg(feature = "testing")]
    pub fn new_in_memory(id: u64) -> Self {
        // Create an in-memory RocksDB instance for testing
        let mut opts = Options::default();
        opts.create_if_missing(true);
        let db = DB::open_default(format!(":memory:{}", id)).expect("Failed to create in-memory DB");

        Self { backup_handle: None, db: Arc::new(db), last_flush_time: Mutex::new(None) }
    }
}

pub mod bonsai_identifier {
    pub const CONTRACT: &[u8] = b"0xcontract";
    pub const CLASS: &[u8] = b"0xclass";
}
