use std::ops::Deref;

use crate::error::GraphStoreError;

type RocksKV = rocksdb::DBWithThreadMode<rocksdb::MultiThreaded>;

pub struct KvEngine {
    inner: RocksKV,
}

impl Deref for KvEngine {
    type Target = RocksKV;

    fn deref(&self) -> &RocksKV {
        &self.inner
    }
}

impl KvEngine {
    pub fn open(path: &str) -> Result<Self, GraphStoreError> {
        let mut opts = rocksdb::Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);

        let cf_descriptors = vec![
            rocksdb::ColumnFamilyDescriptor::new(cf_catalog::CF_NAME, rocksdb::Options::default()),
            rocksdb::ColumnFamilyDescriptor::new(cf_meta::CF_NAME, rocksdb::Options::default()),
            rocksdb::ColumnFamilyDescriptor::new(cf_topology::CF_NAME, rocksdb::Options::default()),
            rocksdb::ColumnFamilyDescriptor::new(cf_property::CF_NAME, rocksdb::Options::default()),
            rocksdb::ColumnFamilyDescriptor::new(cf_indexdata::CF_NAME, rocksdb::Options::default()),
        ];
        let db = match RocksKV::open_cf_descriptors(&opts, path, cf_descriptors) {
            Ok(db) => db,
            Err(_) => {
                let db = RocksKV::open(&opts, path)?;
                let cf_opts = rocksdb::Options::default();
                db.create_cf(cf_catalog::CF_NAME, &cf_opts)?;
                db.create_cf(cf_meta::CF_NAME, &cf_opts)?;
                db.create_cf(cf_topology::CF_NAME, &cf_opts)?;
                db.create_cf(cf_property::CF_NAME, &cf_opts)?;
                db.create_cf(cf_indexdata::CF_NAME, &cf_opts)?;
                db
            }
        };

        Ok(Self { inner: db })
    }
}

// graph catalog
pub(crate) mod cf_catalog {
    pub const CF_NAME: &str = "cf_catalog";
    // Constraint metadata: | prefix | constraint_name |
    pub const CONSTRAINT_META_PREFIX: u8 = 0x01;
    // Label to constraints mapping: | prefix | label_id | constraint_name |
    pub const LABEL_CONSTRAINT_PREFIX: u8 = 0x03;
}

// graph token
pub(crate) mod cf_meta {
    pub const CF_NAME: &str = "cf_meta";
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
    pub const CF_NAME: &str = "cf_topology";
    pub const REL_KEY_PREFIX: u8 = 0x01;
}

pub(crate) mod cf_property {
    // node property
    pub const CF_NAME: &str = "cf_property";
    pub const NODE_KEY_PREFIX: &[u8; 1] = &[0x01];
}

// index data
pub(crate) mod cf_indexdata {
    pub const CF_NAME: &str = "cf_indexdata";
    // Unique index: | prefix | label_id | prop_key_ids... | prop_values... |
    pub const UNIQUE_INDEX_PREFIX: u8 = 0x01;
}
