//! Index key encoding utilities.
//!
//! This module provides utilities for encoding scalar values into binary format
//! suitable for use as index keys. The encoding reuses the `mapb` binary format
//! for consistency with property storage.
//!
//! # Format
//!
//! Each value is encoded as: `[type_tag: 1B][data]`
//!
//! For variable-length types (string, list), a length prefix is included:
//! `[type_tag: 1B][length: 4B LE][data]`
//!
//! Composite keys are encoded by concatenating individual value encodings.

use bytes::{BufMut, BytesMut};

use super::meta::*;
use crate::scalar::ScalarRef;

/// Encoder for index keys.
///
/// This codec produces a compact binary representation suitable for use as
/// RocksDB keys. The format is compatible with the `mapb` property storage
/// format.
pub struct IndexKeyCodec;

impl IndexKeyCodec {
    /// Encode a single scalar value for use in an index key.
    pub fn encode_single(value: &ScalarRef) -> Vec<u8> {
        let mut buf = BytesMut::with_capacity(32);
        Self::encode_value(&mut buf, value);
        buf.to_vec()
    }

    /// Encode multiple values as a composite index key.
    ///
    /// Values are encoded in order and concatenated. Each value is self-describing
    /// via its type tag and (for variable-length types) length prefix.
    pub fn encode_composite(values: &[ScalarRef]) -> Vec<u8> {
        let mut buf = BytesMut::with_capacity(64);
        for value in values {
            Self::encode_value(&mut buf, value);
        }
        buf.to_vec()
    }

    /// Encode a single value into the buffer.
    fn encode_value(buf: &mut BytesMut, value: &ScalarRef) {
        match value {
            ScalarRef::Null => {
                buf.put_u8(NULL_TAG);
            }
            ScalarRef::Bool(b) => {
                buf.put_u8(BOOL_TAG);
                buf.put_u8(if *b { 1 } else { 0 });
            }
            ScalarRef::Integer(i) => {
                buf.put_u8(INTEGER_TAG);
                buf.put_i64_le(*i);
            }
            ScalarRef::Float(f) => {
                buf.put_u8(FLOAT_TAG);
                buf.put_f64_le(**f);
            }
            ScalarRef::Date(d) => {
                buf.put_u8(DATE_TAG);
                // Date is stored as days since epoch (i32)
                buf.put_slice(&d.to_le_bytes());
            }
            ScalarRef::LocalTime(t) => {
                buf.put_u8(LOCAL_TIME_TAG);
                buf.put_slice(&t.to_le_bytes());
            }
            ScalarRef::LocalDateTime(dt) => {
                buf.put_u8(LOCAL_DATE_TIME_TAG);
                buf.put_slice(&dt.to_le_bytes());
            }
            ScalarRef::ZonedDateTime(dt) => {
                buf.put_u8(ZONED_DATE_TIME_TAG);
                buf.put_slice(&dt.to_le_bytes());
            }
            ScalarRef::String(s) => {
                buf.put_u8(STRING_TAG);
                buf.put_u32_le(s.len() as u32);
                buf.put_slice(s.as_bytes());
            }
            // Node/Rel/Path/Duration/Struct types are not supported as index keys
            ScalarRef::VirtualNode(_)
            | ScalarRef::VirtualRel(_)
            | ScalarRef::Node(_)
            | ScalarRef::Rel(_)
            | ScalarRef::Duration(_)
            | ScalarRef::VirtualPath(_)
            | ScalarRef::Path(_)
            | ScalarRef::Struct(_) => {
                panic!("This type cannot be used as index key");
            }
            ScalarRef::List(list) => {
                // For list types, we encode the entire list
                // This is mainly for completeness; lists as index keys are unusual
                if let Some(integers) = list.as_integer_list() {
                    buf.put_u8(LIST_INTEGER_TAG);
                    buf.put_u32_le(integers.len() as u32);
                    for i in integers {
                        buf.put_i64_le(i);
                    }
                } else if let Some(floats) = list.as_float_list() {
                    buf.put_u8(LIST_FLOAT_TAG);
                    buf.put_u32_le(floats.len() as u32);
                    for f in floats {
                        buf.put_f64_le(*f);
                    }
                } else if let Some(strings) = list.as_string_list() {
                    buf.put_u8(LIST_STRING_TAG);
                    buf.put_u32_le(strings.len() as u32);
                    for s in strings {
                        buf.put_u32_le(s.len() as u32);
                        buf.put_slice(s.as_bytes());
                    }
                } else {
                    panic!("Unsupported list type for index key");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_type::F64;
    use crate::scalar::ScalarValue;

    #[test]
    fn test_encode_integer() {
        let value = ScalarValue::Integer(12345);
        let encoded = IndexKeyCodec::encode_single(&value.as_scalar_ref());
        assert_eq!(encoded[0], INTEGER_TAG);
        assert_eq!(encoded.len(), 9); // 1 tag + 8 bytes
    }

    #[test]
    fn test_encode_string() {
        let value = ScalarValue::String("hello".to_string());
        let encoded = IndexKeyCodec::encode_single(&value.as_scalar_ref());
        assert_eq!(encoded[0], STRING_TAG);
        assert_eq!(encoded.len(), 1 + 4 + 5); // tag + len + "hello"
    }

    #[test]
    fn test_encode_composite() {
        let v1 = ScalarValue::String("alice@example.com".to_string());
        let v2 = ScalarValue::Integer(30);
        let values = vec![v1.as_scalar_ref(), v2.as_scalar_ref()];
        let encoded = IndexKeyCodec::encode_composite(&values);

        // String: 1 + 4 + 17 = 22 bytes
        // Integer: 1 + 8 = 9 bytes
        // Total: 31 bytes
        assert_eq!(encoded.len(), 22 + 9);
    }

    #[test]
    fn test_encode_null() {
        let value = ScalarValue::Unknown;
        let encoded = IndexKeyCodec::encode_single(&value.as_scalar_ref());
        assert_eq!(encoded, vec![NULL_TAG]);
    }

    #[test]
    fn test_encode_bool() {
        let true_val = ScalarValue::Bool(true);
        let false_val = ScalarValue::Bool(false);

        let encoded_true = IndexKeyCodec::encode_single(&true_val.as_scalar_ref());
        let encoded_false = IndexKeyCodec::encode_single(&false_val.as_scalar_ref());

        assert_eq!(encoded_true, vec![BOOL_TAG, 1]);
        assert_eq!(encoded_false, vec![BOOL_TAG, 0]);
    }

    #[test]
    fn test_encode_float() {
        let value = ScalarValue::Float(F64::from(3.333));
        let encoded = IndexKeyCodec::encode_single(&value.as_scalar_ref());
        assert_eq!(encoded[0], FLOAT_TAG);
        assert_eq!(encoded.len(), 9); // 1 tag + 8 bytes
    }
}
