use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use elio_common::{NodeId, RelationshipId};

use crate::error::GraphStoreError;
use crate::kv::{KvEngine, meta_keys};

// Number of ids to allocate from LMDB
const ID_BATCH_SIZE: u64 = 1000;

/// In charge of allocating node and relationship ids.
pub struct IdStore {
    node_id: IdGenerator,
    rel_id: IdGenerator,
}

impl IdStore {
    pub fn new(engine: Arc<KvEngine>) -> Result<Self, GraphStoreError> {
        let node_id = IdGenerator::new(engine.clone(), meta_keys::MAX_NODE_ID_KEY)?;
        let rel_id = IdGenerator::new(engine.clone(), meta_keys::MAX_REL_ID_KEY)?;
        Ok(Self { node_id, rel_id })
    }
}

impl IdStore {
    pub fn next_node_id(&self) -> Result<NodeId, GraphStoreError> {
        self.node_id.next_id().map(|id| id.into())
    }

    pub fn next_rel_id(&self) -> Result<RelationshipId, GraphStoreError> {
        self.rel_id.next_id().map(|id| id.into())
    }

    pub fn batch_node_id(&self, count: usize) -> Result<Vec<NodeId>, GraphStoreError> {
        let ids = (0..count).map(|_| self.next_node_id()).collect::<Result<Vec<_>, _>>()?;
        Ok(ids)
    }

    pub fn batch_rel_id(&self, count: usize) -> Result<Vec<RelationshipId>, GraphStoreError> {
        let ids = (0..count).map(|_| self.next_rel_id()).collect::<Result<Vec<_>, _>>()?;
        Ok(ids)
    }
}

pub struct IdGenerator {
    // current available id in memory
    current: AtomicU64,
    // max available id in memory
    max: AtomicU64,

    // key in id_counters tree
    key: &'static [u8],
    engine: Arc<KvEngine>,

    // refill lock — only one thread can refill from LMDB
    refill_lock: Mutex<()>,
}

impl IdGenerator {
    pub fn new(engine: Arc<KvEngine>, key: &'static [u8]) -> Result<Self, GraphStoreError> {
        // Read current value from LMDB
        let start_val = {
            let rtxn = engine.meta.env.read_txn().map_err(GraphStoreError::Heed)?;
            match engine.meta.id_counters.get(&rtxn, key).map_err(GraphStoreError::Heed)? {
                Some(val) => {
                    let mut bytes = [0u8; 8];
                    bytes.copy_from_slice(val);
                    u64::from_le_bytes(bytes)
                }
                None => 0,
            }
        };

        Ok(Self {
            current: AtomicU64::new(start_val),
            max: AtomicU64::new(start_val), // force refill on first use
            key,
            engine,
            refill_lock: Mutex::new(()),
        })
    }

    pub fn next_id(&self) -> Result<u64, GraphStoreError> {
        loop {
            let current = self.current.load(Ordering::Relaxed);
            let max = self.max.load(Ordering::Relaxed);

            if current < max {
                if self
                    .current
                    .compare_exchange(current, current + 1, Ordering::Acquire, Ordering::Relaxed)
                    .is_ok()
                {
                    return Ok(current + 1);
                }
                continue;
            }

            // refill from LMDB
            let _guard = self.refill_lock.lock().unwrap();

            // double check after acquiring lock
            if self.current.load(Ordering::Relaxed) < self.max.load(Ordering::Relaxed) {
                continue;
            }
            self.refill_from_db()?;
        }
    }

    fn refill_from_db(&self) -> Result<(), GraphStoreError> {
        let mut wtxn = self.engine.meta.env.write_txn().map_err(GraphStoreError::Heed)?;

        let old_max = match self
            .engine
            .meta
            .id_counters
            .get(&wtxn, self.key)
            .map_err(GraphStoreError::Heed)?
        {
            Some(val) => {
                let mut bytes = [0u8; 8];
                bytes.copy_from_slice(val);
                u64::from_le_bytes(bytes)
            }
            None => 0,
        };

        let new_max = old_max + ID_BATCH_SIZE;

        self.engine
            .meta
            .id_counters
            .put(&mut wtxn, self.key, &new_max.to_le_bytes())
            .map_err(GraphStoreError::Heed)?;

        wtxn.commit().map_err(GraphStoreError::Heed)?;

        // update in memory state
        self.current.store(old_max, Ordering::Release);
        self.max.store(new_max, Ordering::Release);

        Ok(())
    }
}
