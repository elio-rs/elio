pub mod codec;
pub mod constraint;
pub mod dict;
pub mod error;
pub mod graph;
pub mod token;
pub mod transaction;

pub type KvEngine = rocksdb::DBWithThreadMode<rocksdb::MultiThreaded>;

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

pub(crate) mod cf_topology {
    pub const CF_NAME: &str = "cf_topology";
    pub const REL_KEY_PREFIX: u8 = 0x01;
}

pub(crate) mod cf_property {
    // node property
    pub const CF_NAME: &str = "cf_property";
    pub const NODE_KEY_PREFIX: &[u8; 1] = &[0x01];
}

pub(crate) mod cf_constraint {
    pub const CF_NAME: &str = "cf_constraint";
    // Constraint metadata: | prefix | constraint_name |
    pub const CONSTRAINT_META_PREFIX: u8 = 0x01;
    // Unique index: | prefix | label_id | prop_key_ids... | prop_values... |
    pub const UNIQUE_INDEX_PREFIX: u8 = 0x02;
    // Label to constraints mapping: | prefix | label_id | constraint_name |
    pub const LABEL_CONSTRAINT_PREFIX: u8 = 0x03;
}
