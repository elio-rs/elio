use std::collections::HashMap;
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::{Arc, RwLock};

use arc_swap::ArcSwap;
use elio_common::{LabelId, PropertyKeyId, RelationshipTypeId, TokenId, TokenKind};

use crate::codec::TokenCodec;
use crate::error::GraphStoreError;
use crate::kv::{self, CfKind, KvEngine};

/// Lock-free reverse lookup table: token_id (u16) → token string.
/// Reads are wait-free via `ArcSwap::load`. Writes clone-on-write the inner Vec.
struct IdToStr(ArcSwap<Vec<Arc<str>>>);

impl IdToStr {
    fn new() -> Self {
        Self(ArcSwap::from_pointee(Vec::new()))
    }

    /// O(1) lookup by index — no hashing, no locking.
    fn get(&self, id: u16) -> Option<Arc<str>> {
        let v = self.0.load();
        v.get(id as usize).cloned()
    }

    /// Insert at position `id`, growing the vec if needed.
    fn insert(&self, id: u16, val: Arc<str>) {
        let mut v = (*self.0.load_full()).clone();
        let idx = id as usize;
        if idx >= v.len() {
            v.resize(idx + 1, Arc::from(""));
        }
        v[idx] = val;
        self.0.store(Arc::new(v));
    }

    /// Bulk load from (token, id) pairs.
    fn load_from(&self, entries: &[(String, u16)]) {
        let max_id = entries.iter().map(|(_, id)| *id as usize).max();
        let mut v = match max_id {
            Some(m) => vec![Arc::from(""); m + 1],
            None => Vec::new(),
        };
        for (token, id) in entries {
            v[*id as usize] = Arc::from(token.as_str());
        }
        self.0.store(Arc::new(v));
    }
}

pub struct TokenStore {
    db: Arc<KvEngine>,
    // in memory cache
    next_label_id: AtomicU16,
    next_reltype_id: AtomicU16,
    next_property_key_id: AtomicU16,
    labels: RwLock<HashMap<String, LabelId>>,
    reltypes: RwLock<HashMap<String, RelationshipTypeId>>,
    property_keys: RwLock<HashMap<String, PropertyKeyId>>,

    id2label: IdToStr,
    id2reltype: IdToStr,
    id2property_key: IdToStr,
}

impl TokenStore {
    pub fn new(db: Arc<KvEngine>) -> Result<Self, GraphStoreError> {
        let mut store = Self {
            db,
            next_label_id: AtomicU16::new(0),
            next_reltype_id: AtomicU16::new(0),
            next_property_key_id: AtomicU16::new(0),
            labels: RwLock::new(HashMap::new()),
            reltypes: RwLock::new(HashMap::new()),
            property_keys: RwLock::new(HashMap::new()),
            id2label: IdToStr::new(),
            id2reltype: IdToStr::new(),
            id2property_key: IdToStr::new(),
        };
        store.load_from_db()?;
        Ok(store)
    }

    fn load_from_db(&mut self) -> Result<(), GraphStoreError> {
        self.load_token_dict(TokenKind::Label)?;
        self.load_token_dict(TokenKind::RelationshipType)?;
        self.load_token_dict(TokenKind::PropertyKey)?;
        Ok(())
    }

    pub fn get_token_id(&self, token: &str, token_kind: TokenKind) -> Option<TokenId> {
        match token_kind {
            TokenKind::Label => self.get_label_id(token),
            TokenKind::RelationshipType => self.get_reltype_id(token),
            TokenKind::PropertyKey => self.get_property_key_id(token),
        }
    }

    pub fn get_label_id(&self, label: &str) -> Option<LabelId> {
        let labels = self.labels.read().unwrap();
        labels.get(label).cloned()
    }

    pub fn get_reltype_id(&self, reltype: &str) -> Option<RelationshipTypeId> {
        let reltypes = self.reltypes.read().unwrap();
        reltypes.get(reltype).cloned()
    }

    pub fn get_property_key_id(&self, property_key: &str) -> Option<PropertyKeyId> {
        let property_keys = self.property_keys.read().unwrap();
        property_keys.get(property_key).cloned()
    }

    pub fn get_or_create_label_id(&self, label: &str) -> Result<LabelId, GraphStoreError> {
        self.get_or_create_token(label, TokenKind::Label)
    }

    pub fn get_or_create_reltype_id(&self, reltype: &str) -> Result<RelationshipTypeId, GraphStoreError> {
        self.get_or_create_token(reltype, TokenKind::RelationshipType)
    }

    pub fn get_or_create_property_key_id(&self, property_key: &str) -> Result<PropertyKeyId, GraphStoreError> {
        self.get_or_create_token(property_key, TokenKind::PropertyKey)
    }
}

impl TokenStore {
    pub fn get_or_create_token(&self, token: &str, token_kind: TokenKind) -> Result<u16, GraphStoreError> {
        let mut tokens = match token_kind {
            TokenKind::Label => self.labels.write().unwrap(),
            TokenKind::RelationshipType => self.reltypes.write().unwrap(),
            TokenKind::PropertyKey => self.property_keys.write().unwrap(),
        };
        if let Some(token_id) = tokens.get(token) {
            return Ok(*token_id);
        }

        // create token if not exists
        let token_id = match token_kind {
            TokenKind::Label => self.next_label_id.fetch_add(1, Ordering::Relaxed),
            TokenKind::RelationshipType => self.next_reltype_id.fetch_add(1, Ordering::Relaxed),
            TokenKind::PropertyKey => self.next_property_key_id.fetch_add(1, Ordering::Relaxed),
        };
        tokens.insert(token.to_string(), token_id);

        let id2str = match token_kind {
            TokenKind::Label => &self.id2label,
            TokenKind::RelationshipType => &self.id2reltype,
            TokenKind::PropertyKey => &self.id2property_key,
        };
        id2str.insert(token_id, Arc::from(token));

        // write to db
        let tree = self.db.tree(CfKind::Meta);
        {
            let key = TokenCodec::data_key(&token_kind, token);
            let value = TokenCodec::encode_data_value(token_id);
            kv::bf_insert(tree, &key, &value)?;
        }
        Ok(token_id)
    }

    pub fn get_token_val(&self, id: TokenId, kind: TokenKind) -> Result<Arc<str>, GraphStoreError> {
        let id2str = match kind {
            TokenKind::Label => &self.id2label,
            TokenKind::RelationshipType => &self.id2reltype,
            TokenKind::PropertyKey => &self.id2property_key,
        };
        id2str.get(id).ok_or(GraphStoreError::Token(id.to_string()))
    }

    fn load_token_dict(&mut self, token_kind: TokenKind) -> Result<(), GraphStoreError> {
        let tokens = self.db_get_all_token(token_kind)?;
        let dict = match token_kind {
            TokenKind::Label => &mut self.labels,
            TokenKind::RelationshipType => &mut self.reltypes,
            TokenKind::PropertyKey => &mut self.property_keys,
        };
        // update next id
        let next_id = match tokens.iter().map(|(_, id)| id).max() {
            Some(max_id) => max_id + 1,
            None => 0,
        };
        match token_kind {
            TokenKind::Label => self.next_label_id.store(next_id, Ordering::Relaxed),
            TokenKind::RelationshipType => self.next_reltype_id.store(next_id, Ordering::Relaxed),
            TokenKind::PropertyKey => self.next_property_key_id.store(next_id, Ordering::Relaxed),
        }

        // Bulk load id→str
        let id2str = match token_kind {
            TokenKind::Label => &self.id2label,
            TokenKind::RelationshipType => &self.id2reltype,
            TokenKind::PropertyKey => &self.id2property_key,
        };
        id2str.load_from(&tokens);

        // Load str→id
        {
            let mut dict = dict.write().unwrap();
            dict.clear();
            dict.reserve(tokens.len());
            for (token, id) in tokens {
                dict.insert(token, id);
            }
        }
        Ok(())
    }

    fn db_get_all_token(&self, token_kind: TokenKind) -> Result<Vec<(String, u16)>, GraphStoreError> {
        let tree = self.db.tree(CfKind::Meta);
        let prefix = TokenCodec::data_key_prefix(&token_kind);
        let entries = kv::bf_prefix_scan_iter(tree, &prefix)?;

        let mut tokens = Vec::new();
        for (key, value) in entries {
            let (_, token) = TokenCodec::decode_data_key(&key);
            let token_id = TokenCodec::decode_data_value(&value);
            tokens.push((token, token_id));
        }
        Ok(tokens)
    }
}
