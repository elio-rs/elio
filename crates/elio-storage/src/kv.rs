use std::path::Path;

use bf_tree::{BfTree, Config, LeafInsertResult, LeafReadResult, ScanIter, ScanReturnField};

use crate::error::GraphStoreError;

/// Default cache size per BfTree instance (64 MB).
const DEFAULT_CACHE_SIZE: usize = 64 * 1024 * 1024;

/// Maximum key length (256 bytes covers all key formats including unique indexes).
const MAX_KEY_LEN: usize = 256;

/// Leaf page size. Constraints: max 32768, leaf_page_size/min_record_size <= 4096.
const LEAF_PAGE_SIZE: usize = 8192;

/// Maximum record size (key + value). Constrained by leaf page size.
const MAX_RECORD_SIZE: usize = 3600;

/// BfTree does not allow empty values. Use this as a placeholder for key-only entries.
pub(crate) const EMPTY_VALUE_SENTINEL: &[u8] = &[0x00];

/// Logical column family kind — one BfTree instance per kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum CfKind {
    Catalog,
    Meta,
    Topology,
    Data,
    IndexData,
}

pub struct KvEngine {
    catalog: BfTree,
    meta: BfTree,
    topology: BfTree,
    data: BfTree,
    indexdata: BfTree,
}

// SAFETY: BfTree is Send + Sync per its documentation.
unsafe impl Send for KvEngine {}
unsafe impl Sync for KvEngine {}

impl KvEngine {
    fn make_config(file: &Path) -> Config {
        let mut config = Config::new(file, DEFAULT_CACHE_SIZE);
        config
            .cb_max_key_len(MAX_KEY_LEN)
            .cb_min_record_size(2)
            .cb_max_record_size(MAX_RECORD_SIZE)
            .leaf_page_size(LEAF_PAGE_SIZE)
            .read_record_cache(false);
        config
    }

    pub fn open(path: &str) -> Result<Self, GraphStoreError> {
        let base = Path::new(path);
        std::fs::create_dir_all(base).map_err(GraphStoreError::storage)?;

        let open_tree = |name: &str| -> Result<BfTree, GraphStoreError> {
            let file = base.join(name);
            let config = Self::make_config(&file);
            if file.exists() {
                // Recover from existing snapshot
                BfTree::new_from_snapshot(config, None).map_err(|e| GraphStoreError::storage(format!("{e:?}")))
            } else {
                BfTree::with_config(config, None).map_err(|e| GraphStoreError::storage(format!("{e:?}")))
            }
        };

        Ok(Self {
            catalog: open_tree("catalog")?,
            meta: open_tree("meta")?,
            topology: open_tree("topology")?,
            data: open_tree("data")?,
            indexdata: open_tree("indexdata")?,
        })
    }

    pub(crate) fn tree(&self, cf: CfKind) -> &BfTree {
        match cf {
            CfKind::Catalog => &self.catalog,
            CfKind::Meta => &self.meta,
            CfKind::Topology => &self.topology,
            CfKind::Data => &self.data,
            CfKind::IndexData => &self.indexdata,
        }
    }
}

impl Drop for KvEngine {
    fn drop(&mut self) {
        self.catalog.snapshot();
        self.meta.snapshot();
        self.topology.snapshot();
        self.data.snapshot();
        self.indexdata.snapshot();
    }
}

/// Read a key from a BfTree. Returns `Ok(Some(value))` if found, `Ok(None)` if
/// not found or deleted, and `Err` on invalid key.
pub(crate) fn bf_read(tree: &BfTree, key: &[u8]) -> Result<Option<Vec<u8>>, GraphStoreError> {
    // Start with a reasonable buffer; grow if needed.
    let mut buf = vec![0u8; 4096];
    match tree.read(key, &mut buf) {
        LeafReadResult::Found(len) => {
            let len = len as usize;
            if len <= buf.len() {
                buf.truncate(len);
                Ok(Some(buf))
            } else {
                // Value was larger than buffer — retry with exact size.
                buf.resize(len, 0);
                match tree.read(key, &mut buf) {
                    LeafReadResult::Found(len2) => {
                        buf.truncate(len2 as usize);
                        Ok(Some(buf))
                    }
                    other => Err(GraphStoreError::storage(format!(
                        "bf-tree retry read failed: {other:?}"
                    ))),
                }
            }
        }
        LeafReadResult::NotFound | LeafReadResult::Deleted => Ok(None),
        LeafReadResult::InvalidKey => Err(GraphStoreError::storage("bf-tree invalid key")),
    }
}

/// Insert a key-value pair into a BfTree.
pub(crate) fn bf_insert(tree: &BfTree, key: &[u8], value: &[u8]) -> Result<(), GraphStoreError> {
    match tree.insert(key, value) {
        LeafInsertResult::Success => Ok(()),
        LeafInsertResult::InvalidKV(msg) => Err(GraphStoreError::storage(format!("bf-tree insert: {msg}"))),
    }
}

/// Compute the successor prefix for range scans (increment last byte, handle carry).
pub(crate) fn prefix_successor(prefix: &[u8]) -> Option<Vec<u8>> {
    let mut succ = prefix.to_vec();
    // Increment from the last byte, carry over on overflow.
    while let Some(last) = succ.last_mut() {
        if *last < 0xFF {
            *last += 1;
            return Some(succ);
        }
        succ.pop();
    }
    None // prefix was all 0xFF — no upper bound
}

/// Lazy prefix scan iterator over a BfTree snapshot.
pub(crate) struct BfPrefixScanIter<'a> {
    inner: ScanIter<'a, 'a>,
    prefix: Vec<u8>,
    buf: Vec<u8>,
    done: bool,
}

impl<'a> BfPrefixScanIter<'a> {
    fn new(tree: &'a BfTree, prefix: &[u8]) -> Result<Self, GraphStoreError> {
        let inner = match prefix_successor(prefix) {
            Some(end_key) => tree.scan_with_end_key(prefix, &end_key, ScanReturnField::KeyAndValue),
            None => tree.scan_with_count(prefix, usize::MAX, ScanReturnField::KeyAndValue),
        }
        .map_err(|e| GraphStoreError::storage(format!("bf-tree prefix scan: {e:?}")))?;

        Ok(Self {
            inner,
            prefix: prefix.to_vec(),
            buf: vec![0u8; 8192],
            done: false,
        })
    }
}

impl Iterator for BfPrefixScanIter<'_> {
    type Item = (Vec<u8>, Vec<u8>);

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        loop {
            let Some((key_len, val_len)) = self.inner.next(&mut self.buf) else {
                self.done = true;
                return None;
            };
            let total = key_len + val_len;
            if total > self.buf.len() {
                self.buf.resize(total * 2, 0);
                continue;
            }

            if !self.buf[..key_len].starts_with(&self.prefix) {
                self.done = true;
                return None;
            }

            let key = self.buf[..key_len].to_vec();
            let value = self.buf[key_len..key_len + val_len].to_vec();
            return Some((key, value));
        }
    }
}

pub(crate) fn bf_prefix_scan_iter<'a>(
    tree: &'a BfTree,
    prefix: &[u8],
) -> Result<BfPrefixScanIter<'a>, GraphStoreError> {
    BfPrefixScanIter::new(tree, prefix)
}

/// Eagerly collect all key-value pairs matching a prefix from a BfTree.
pub(crate) fn bf_prefix_scan(tree: &BfTree, prefix: &[u8]) -> Vec<(Vec<u8>, Vec<u8>)> {
    let Ok(iter) = bf_prefix_scan_iter(tree, prefix) else {
        return Vec::new();
    };
    iter.collect()
}

// graph catalog
pub(crate) mod cf_catalog {
    // Constraint metadata: | prefix | constraint_name |
    pub const CONSTRAINT_META_PREFIX: u8 = 0x01;
    // Index metadata: | prefix | index_name |
    pub const INDEX_META_PREFIX: u8 = 0x02;
    // Label to constraints mapping: | prefix | label_id | constraint_name |
    pub const LABEL_CONSTRAINT_PREFIX: u8 = 0x03;
}

// graph token
pub(crate) mod cf_meta {
    // token -> token_id
    pub(crate) const LABEL_KEY_PREFIX: u8 = 0x01;
    pub(crate) const RELTYPE_KEY_PREFIX: u8 = 0x02;
    pub(crate) const PROPERTY_KEY_PREFIX: u8 = 0x03;
    // id allocation
    pub(crate) const MAX_NODE_ID_KEY: &[u8; 1] = &[0x04];
    pub(crate) const MAX_REL_ID_KEY: &[u8; 1] = &[0x05];
}

// graph data
pub(crate) mod cf_topology {
    pub const REL_KEY_PREFIX: u8 = 0x01;
}

pub(crate) mod cf_data {
    // node property
    pub const NODE_KEY_PREFIX: &[u8; 1] = &[0x01];
    // label index
    pub const LABEL_INDEX_PREFIX: u8 = 0x02;
}

// index data
pub(crate) mod cf_indexdata {
    // Unique index: | prefix | label_id | prop_key_ids... | prop_values... |
    pub const UNIQUE_INDEX_PREFIX: u8 = 0x01;
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{bf_insert, bf_prefix_scan_iter, BfPrefixScanIter, CfKind, KvEngine};

    #[test]
    fn prefix_scan_iter_stops_before_successor_prefix() {
        let dir = tempdir().unwrap();
        let db = KvEngine::open(dir.path().to_str().unwrap()).unwrap();
        let tree = db.tree(CfKind::Data);

        bf_insert(tree, &[0x01], b"a").unwrap();
        bf_insert(tree, &[0x01, 0x10], b"b").unwrap();
        bf_insert(tree, &[0x02], b"c").unwrap();

        let keys: Vec<_> = bf_prefix_scan_iter(tree, &[0x01])
            .unwrap()
            .map(|(key, _)| key)
            .collect();

        assert_eq!(keys, vec![vec![0x01], vec![0x01, 0x10]]);
    }

    #[test]
    fn prefix_scan_iter_handles_all_ff_prefix() {
        let dir = tempdir().unwrap();
        let db = KvEngine::open(dir.path().to_str().unwrap()).unwrap();
        let tree = db.tree(CfKind::Data);

        bf_insert(tree, &[0xFE, 0xFF], b"a").unwrap();
        bf_insert(tree, &[0xFF], b"b").unwrap();
        bf_insert(tree, &[0xFF, 0x00], b"c").unwrap();
        bf_insert(tree, &[0xFF, 0x10], b"d").unwrap();

        let keys: Vec<_> = bf_prefix_scan_iter(tree, &[0xFF])
            .unwrap()
            .map(|(key, _)| key)
            .collect();

        assert_eq!(keys, vec![vec![0xFF], vec![0xFF, 0x00], vec![0xFF, 0x10]]);
    }

    #[test]
    fn eager_prefix_scan_collects_from_iterator() {
        let dir = tempdir().unwrap();
        let db = KvEngine::open(dir.path().to_str().unwrap()).unwrap();
        let tree = db.tree(CfKind::Data);

        bf_insert(tree, &[0x01, 0x01], b"a").unwrap();
        bf_insert(tree, &[0x01, 0x02], b"b").unwrap();

        let eager = super::bf_prefix_scan(tree, &[0x01]);
        let lazy: Vec<_> = BfPrefixScanIter::new(tree, &[0x01]).unwrap().collect();

        assert_eq!(eager, lazy);
    }
}
