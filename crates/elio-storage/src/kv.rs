use std::fs;
use std::path::Path;
use std::sync::Mutex;

use heed::types::Bytes;
use heed::{Database, Env, EnvOpenOptions, WithoutTls};

use crate::error::GraphStoreError;

/// Default map size: 10 GB (virtual address space only; physical pages allocated on demand)
const DEFAULT_MAP_SIZE: usize = 10 * 1024 * 1024 * 1024;

/// Meta environment: tokens + ID counters.
/// Separate from graph env to avoid write-lock contention (LMDB allows only one RwTxn per env).
pub struct MetaEnv {
    pub(crate) env: Env,
    pub(crate) tokens: Database<Bytes, Bytes>,
    pub(crate) id_counters: Database<Bytes, Bytes>,
}

/// Graph environment: graph data + indexes + catalog.
/// These need atomic writes (e.g. node + label index in one txn).
/// Uses `WithoutTls` so that `RoTxn` is `Send` (required for cross-thread read transactions).
pub struct GraphEnv {
    pub(crate) env: Env<WithoutTls>,
    pub(crate) graph: Database<Bytes, Bytes>,
    pub(crate) index: Database<Bytes, Bytes>,
    pub(crate) catalog: Database<Bytes, Bytes>,
    /// Lock to serialize map_size resize operations
    resize_lock: Mutex<()>,
}

impl GraphEnv {
    /// Try to resize the graph env when MapFull is encountered.
    pub(crate) fn try_resize(&self) -> Result<(), GraphStoreError> {
        let _guard = self.resize_lock.lock().unwrap();
        let current = self.env.info().map_size;
        let new_size = current * 2;
        unsafe {
            self.env.resize(new_size).map_err(GraphStoreError::Heed)?;
        }
        tracing::info!(old_size = current, new_size, "resized graph env map_size");
        Ok(())
    }
}

pub struct KvEngine {
    pub(crate) meta: MetaEnv,
    pub(crate) graph: GraphEnv,
}

impl KvEngine {
    pub fn open(path: &str) -> Result<Self, GraphStoreError> {
        let base = Path::new(path);
        fs::create_dir_all(base).map_err(|e| GraphStoreError::Internal(format!("failed to create db dir: {e}")))?;

        // --- Meta env ---
        let meta_path = base.join("meta.mdb");
        fs::create_dir_all(&meta_path)
            .map_err(|e| GraphStoreError::Internal(format!("failed to create meta dir: {e}")))?;

        let meta_env = unsafe {
            EnvOpenOptions::new()
                .max_dbs(2)
                .map_size(DEFAULT_MAP_SIZE)
                .open(&meta_path)
                .map_err(GraphStoreError::Heed)?
        };

        let (tokens, id_counters) = {
            let mut wtxn = meta_env.write_txn().map_err(GraphStoreError::Heed)?;
            let tokens = meta_env
                .create_database(&mut wtxn, Some("tokens"))
                .map_err(GraphStoreError::Heed)?;
            let id_counters = meta_env
                .create_database(&mut wtxn, Some("id_counters"))
                .map_err(GraphStoreError::Heed)?;
            wtxn.commit().map_err(GraphStoreError::Heed)?;
            (tokens, id_counters)
        };

        // --- Graph env ---
        let graph_path = base.join("graph.mdb");
        fs::create_dir_all(&graph_path)
            .map_err(|e| GraphStoreError::Internal(format!("failed to create graph dir: {e}")))?;

        let graph_env = unsafe {
            EnvOpenOptions::new()
                .read_txn_without_tls()
                .max_dbs(3)
                .map_size(DEFAULT_MAP_SIZE)
                .open(&graph_path)
                .map_err(GraphStoreError::Heed)?
        };

        let (graph, index, catalog) = {
            let mut wtxn = graph_env.write_txn().map_err(GraphStoreError::Heed)?;
            let graph = graph_env
                .create_database(&mut wtxn, Some("graph"))
                .map_err(GraphStoreError::Heed)?;
            let index = graph_env
                .create_database(&mut wtxn, Some("index"))
                .map_err(GraphStoreError::Heed)?;
            let catalog = graph_env
                .create_database(&mut wtxn, Some("catalog"))
                .map_err(GraphStoreError::Heed)?;
            wtxn.commit().map_err(GraphStoreError::Heed)?;
            (graph, index, catalog)
        };

        Ok(Self {
            meta: MetaEnv {
                env: meta_env,
                tokens,
                id_counters,
            },
            graph: GraphEnv {
                env: graph_env,
                graph,
                index,
                catalog,
                resize_lock: Mutex::new(()),
            },
        })
    }
}

// Key constants for meta env
pub(crate) mod meta_keys {
    // Token kind prefixes (used in tokens tree)
    pub const LABEL_KEY_PREFIX: u8 = 0x01;
    pub const RELTYPE_KEY_PREFIX: u8 = 0x02;
    pub const PROPERTY_KEY_PREFIX: u8 = 0x03;
    // ID counter keys (used in id_counters tree)
    pub const MAX_NODE_ID_KEY: &[u8; 1] = &[0x04];
    pub const MAX_REL_ID_KEY: &[u8; 1] = &[0x05];
}

// Key constants for graph env
pub(crate) mod graph_keys {
    // Graph tree segments
    pub const SEGMENT_HEADER: u8 = 0x00;
    pub const SEGMENT_OUT_EDGES: u8 = 0x01;
    pub const SEGMENT_IN_EDGES: u8 = 0x02;

    // Index tree prefixes
    pub const LABEL_INDEX_PREFIX: u8 = 0x01;
    pub const UNIQUE_INDEX_PREFIX: u8 = 0x02;

    // Catalog tree prefixes
    pub const CONSTRAINT_META_PREFIX: u8 = 0x01;
    pub const INDEX_META_PREFIX: u8 = 0x02;
    pub const LABEL_CONSTRAINT_PREFIX: u8 = 0x03;
}

// Adaptive node storage flags
pub(crate) mod node_flags {
    pub const PACKED: u8 = 0x00;
    pub const SPLIT: u8 = 0x01;
}

/// Threshold for packed vs split mode (4 KB)
pub(crate) const ADAPTIVE_THRESHOLD: usize = 4096;
