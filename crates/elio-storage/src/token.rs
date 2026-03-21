use std::collections::HashMap;
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::{Arc, RwLock};

use elio_common::{LabelId, PropertyKeyId, RelationshipTypeId, TokenId, TokenKind};

use crate::codec::TokenCodec;
use crate::error::GraphStoreError;
use crate::kv::KvEngine;

pub struct TokenStore {
    engine: Arc<KvEngine>,
    // in memory cache
    next_label_id: AtomicU16,
    next_reltype_id: AtomicU16,
    next_property_key_id: AtomicU16,
    labels: RwLock<HashMap<String, LabelId>>,
    reltypes: RwLock<HashMap<String, RelationshipTypeId>>,
    property_keys: RwLock<HashMap<String, PropertyKeyId>>,

    id2label: RwLock<HashMap<LabelId, String>>,
    id2reltype: RwLock<HashMap<RelationshipTypeId, String>>,
    id2property_key: RwLock<HashMap<PropertyKeyId, String>>,
}

impl TokenStore {
    pub fn new(engine: Arc<KvEngine>) -> Result<Self, GraphStoreError> {
        let mut store = Self {
            engine,
            next_label_id: AtomicU16::new(0),
            next_reltype_id: AtomicU16::new(0),
            next_property_key_id: AtomicU16::new(0),
            labels: RwLock::new(HashMap::new()),
            reltypes: RwLock::new(HashMap::new()),
            property_keys: RwLock::new(HashMap::new()),
            id2label: RwLock::new(HashMap::new()),
            id2reltype: RwLock::new(HashMap::new()),
            id2property_key: RwLock::new(HashMap::new()),
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
        let (mut tokens, mut id2str) = match token_kind {
            TokenKind::Label => (self.labels.write().unwrap(), self.id2label.write().unwrap()),
            TokenKind::RelationshipType => (self.reltypes.write().unwrap(), self.id2reltype.write().unwrap()),
            TokenKind::PropertyKey => (
                self.property_keys.write().unwrap(),
                self.id2property_key.write().unwrap(),
            ),
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
        id2str.insert(token_id, token.to_string());

        // write to meta env with independent short RwTxn
        {
            let mut wtxn = self.engine.meta.env.write_txn().map_err(GraphStoreError::Heed)?;
            let key = TokenCodec::data_key(&token_kind, token);
            let value = TokenCodec::encode_data_value(token_id);
            self.engine
                .meta
                .tokens
                .put(&mut wtxn, &key, &value)
                .map_err(GraphStoreError::Heed)?;
            wtxn.commit().map_err(GraphStoreError::Heed)?;
        }
        Ok(token_id)
    }

    pub fn get_token_val(&self, id: TokenId, kind: TokenKind) -> Result<Arc<str>, GraphStoreError> {
        let id2str = match kind {
            TokenKind::Label => &self.id2label,
            TokenKind::RelationshipType => &self.id2reltype,
            TokenKind::PropertyKey => &self.id2property_key,
        };
        let id2str = id2str.read().unwrap();
        id2str
            .get(&id)
            .cloned()
            .map(Arc::from)
            .ok_or(GraphStoreError::Token(id.to_string()))
    }

    fn load_token_dict(&mut self, token_kind: TokenKind) -> Result<(), GraphStoreError> {
        let tokens = self.db_get_all_token(token_kind)?;
        let (dict, id2str) = match token_kind {
            TokenKind::Label => (&mut self.labels, &mut self.id2label),
            TokenKind::RelationshipType => (&mut self.reltypes, &mut self.id2reltype),
            TokenKind::PropertyKey => (&mut self.property_keys, &mut self.id2property_key),
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

        {
            let mut dict = dict.write().unwrap();
            dict.clear();
            let mut id2str = id2str.write().unwrap();
            id2str.clear();

            dict.reserve(tokens.len());
            id2str.reserve(tokens.len());

            for (token, id) in tokens {
                dict.insert(token.clone(), id);
                id2str.insert(id, token);
            }
        }
        Ok(())
    }

    fn db_get_all_token(&self, token_kind: TokenKind) -> Result<Vec<(String, u16)>, GraphStoreError> {
        let rtxn = self.engine.meta.env.read_txn().map_err(GraphStoreError::Heed)?;
        let prefix = TokenCodec::data_key_prefix(&token_kind);

        let mut tokens = Vec::new();
        let iter = self.engine.meta.tokens.iter(&rtxn).map_err(GraphStoreError::Heed)?;

        for result in iter {
            let (key, value) = result.map_err(GraphStoreError::Heed)?;
            if !key.starts_with(&prefix) {
                continue;
            }
            let (_, token) = TokenCodec::decode_data_key(key);
            let token_id = TokenCodec::decode_data_value(value);
            tokens.push((token, token_id));
        }
        Ok(tokens)
    }
}
