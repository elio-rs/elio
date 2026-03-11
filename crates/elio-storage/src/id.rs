use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use elio_common::{NodeId, RelationshipId};

use crate::error::GraphStoreError;
use crate::kv::{self, CfKind, KvEngine};

// Number of ids to allocate from storage
const ID_BATCH_SIZE: u64 = 1000;

/// In charge of allocating node and relationship ids.
pub struct IdStore {
    node_id: IdGenerator,
    rel_id: IdGenerator,
}

impl IdStore {
    pub fn new(db: Arc<KvEngine>) -> Result<Self, GraphStoreError> {
        let node_id = IdGenerator::new(db.clone(), (*crate::kv::cf_meta::MAX_NODE_ID_KEY).into())?;
        let rel_id = IdGenerator::new(db.clone(), (*crate::kv::cf_meta::MAX_REL_ID_KEY).into())?;
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

    // key in storage
    key: Arc<[u8]>,
    db: Arc<KvEngine>,

    // refill from storage lock
    // only one write can access db
    refill_lock: Mutex<()>,
}

impl IdGenerator {
    pub fn new(db: Arc<KvEngine>, key: Arc<[u8]>) -> Result<Self, GraphStoreError> {
        let start_val = match kv::bf_read(db.tree(CfKind::Meta), &key)? {
            Some(val) => {
                let mut bytes = [0u8; 8];
                bytes.copy_from_slice(&val);
                u64::from_le_bytes(bytes)
            }
            None => 0,
        };
        Ok(Self {
            current: AtomicU64::new(start_val),
            // force refill when initialize
            max: AtomicU64::new(start_val),
            key,
            db: db.clone(),
            refill_lock: Mutex::new(()),
        })
    }

    pub fn next_id(&self) -> Result<u64, GraphStoreError> {
        loop {
            // get from in memory id first
            let current = self.current.load(Ordering::Relaxed);
            let max = self.max.load(Ordering::Relaxed);

            if current < max {
                // allocate ok
                if self
                    .current
                    .compare_exchange(current, current + 1, Ordering::Acquire, Ordering::Relaxed)
                    .is_ok()
                {
                    return Ok(current + 1);
                }
                // allocate failed due to race, try again
                continue;
            }

            // refill from storage
            let _guard = self.refill_lock.lock().unwrap();

            // other may refill when we're waiting for the lock,
            // so double check again
            if self.current.load(Ordering::Relaxed) < self.max.load(Ordering::Relaxed) {
                continue;
            }
            // refill from storage
            self.refill_from_db()?;
        }
    }

    fn refill_from_db(&self) -> Result<(), GraphStoreError> {
        let tree = self.db.tree(CfKind::Meta);

        // load old value from storage
        let old_max = match kv::bf_read(tree, &self.key)? {
            Some(val) => {
                let mut bytes = [0u8; 8];
                bytes.copy_from_slice(&val);
                u64::from_le_bytes(bytes)
            }
            None => 0,
        };

        let new_max = old_max + ID_BATCH_SIZE;

        // write new_max to storage
        kv::bf_insert(tree, self.key.as_ref(), &new_max.to_le_bytes())?;

        // update in memory state
        self.current.store(old_max, Ordering::Release);
        self.max.store(new_max, Ordering::Release);

        Ok(())
    }
}
