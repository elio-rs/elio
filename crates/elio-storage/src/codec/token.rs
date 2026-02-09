use elio_common::TokenKind;

use crate::cf_meta::{LABEL_KEY_PREFIX, PROPERTY_KEY_PREFIX, RELTYPE_KEY_PREFIX};

/// Storage
///   - key   := <LABEL_PREFIX> <label>
///   - value := <label_id>
///
/// NextId [deleted]
///   we do not need to record the next id here, since the next id can be recovered from the data part
///   - key   := <LABEL_PREFIX> <TOKEN_NEXT_ID>
///   - value := <next_id>
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
}

impl TokenCodec {
    #[inline]
    pub fn read_token(buf: &[u8]) -> u16 {
        TokenCodec::decode(buf)
    }

    #[inline]
    pub fn encode_data_value(id: u16) -> [u8; 2] {
        id.to_le_bytes()
    }

    #[inline]
    pub fn decode(buffer: &[u8]) -> u16 {
        u16::from_le_bytes([buffer[0], buffer[1]])
    }
}
