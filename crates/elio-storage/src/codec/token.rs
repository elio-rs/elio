use elio_common::TokenKind;

use crate::kv::meta_keys::{LABEL_KEY_PREFIX, PROPERTY_KEY_PREFIX, RELTYPE_KEY_PREFIX};

pub struct TokenCodec;

impl TokenCodec {
    pub fn data_key(kind: &TokenKind, token: &str) -> Vec<u8> {
        let mut key = Vec::with_capacity(1 + token.len());
        match kind {
            TokenKind::Label => {
                key.extend_from_slice(&[LABEL_KEY_PREFIX]);
            }
            TokenKind::RelationshipType => {
                key.extend_from_slice(&[RELTYPE_KEY_PREFIX]);
            }
            TokenKind::PropertyKey => {
                key.extend_from_slice(&[PROPERTY_KEY_PREFIX]);
            }
        };
        key.extend_from_slice(token.as_bytes());
        key
    }

    /// Used for prefix scan
    pub fn data_key_prefix(kind: &TokenKind) -> Vec<u8> {
        match kind {
            TokenKind::Label => vec![LABEL_KEY_PREFIX],
            TokenKind::RelationshipType => vec![RELTYPE_KEY_PREFIX],
            TokenKind::PropertyKey => vec![PROPERTY_KEY_PREFIX],
        }
    }

    pub fn decode_data_key(key: &[u8]) -> (TokenKind, String) {
        let kind = match key[0] {
            LABEL_KEY_PREFIX => TokenKind::Label,
            RELTYPE_KEY_PREFIX => TokenKind::RelationshipType,
            PROPERTY_KEY_PREFIX => TokenKind::PropertyKey,
            kind => panic!("invalid token kind {}", kind),
        };
        let token = String::from_utf8_lossy(&key[1..]).to_string();
        (kind, token)
    }

    pub fn decode_data_value(val: &[u8]) -> u16 {
        u16::from_le_bytes([val[0], val[1]])
    }

    #[inline]
    pub fn encode_data_value(id: u16) -> [u8; 2] {
        id.to_le_bytes()
    }
}
