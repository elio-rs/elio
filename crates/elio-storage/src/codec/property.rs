//! PropertyFormat
//! All entities properties are stored in an continous block
//!
//! The block is composed of three parts:
//!   - header: 2B, u16, which contains the number of keys in the block
//!   - keys: 8B * num_keys, which contains the key_id(u16) and the size of the value(u32)
//!
//! keys ::= key_id key_type padding value_bytes
//! key_id ::= u16
//! key_type ::= u8
//! padding ::= u8
//! value_bytes ::= u64
//!                 | bool(u8) with padding
//!                 | Integer(i64)
//!                 | Float(f64)
//!
//! value_data ::= string_data
//!                | boolean_list_data
//!                | integer_list_data
//!                | float_list_data
//!                | string_list_data
//! TODO(power): maybe we should have an StorageDataTypes, to make it easier to extend
//! ```text
//!       2B             12*num_keys B             variable size          
//!   +-----------++------------------------++--------------------------+
//!   |keys_header||key_and_value_size_array||value_data                |
//!   +---+-------++------------------------++--------------------------+
//!       |        |                        |
//!       v        |                        |
//!    num_keys    |                        |
//!             +--+                        +--------------+
//!             v------------------++------------------+   v
//!             |key_id value_bytes||key_id value_bytes|....
//!             +------------------++------------------+
//! ```
//!
//! For fixed size value, we embed the value in the key_and_value_size_array
//! For variable size value, we store the value in the value_data

use bytes::{BufMut, BytesMut};
use elio_common::PropertyKeyId;
use elio_common::store_types::PropertyValue;

use crate::codec::DataTypeCodec;

const KEY_HEADER_BYTES: usize = 2;
const KEY_ITEM_BYTES: usize = 12;

pub struct PropertyFormat {}

impl PropertyFormat {
    #[inline]
    pub fn header_and_keys_bytes(num_keys: usize) -> usize {
        KEY_HEADER_BYTES + KEY_ITEM_BYTES * num_keys
    }

    #[inline]
    pub fn keys_bytes(num_keys: usize) -> usize {
        KEY_ITEM_BYTES * num_keys
    }
}

pub struct PropertyValueCodec;

impl PropertyValueCodec {
    // if value is fixed sized and able to embed into key, return embeded u64
    // if value can not be embeded, write to buf and return the size
    pub fn write(buf: &mut BytesMut, value: &PropertyValue) -> u64 {
        match value {
            PropertyValue::Null => 0,
            PropertyValue::Boolean(b) => *b as u64,
            PropertyValue::Integer(i) => u64::from_le_bytes(i.to_le_bytes()),
            PropertyValue::Float(f) => f.to_bits(),
            PropertyValue::String(s) => {
                let len = s.len();
                buf.put_slice(s.as_bytes());
                len as u64
            }
            PropertyValue::ListBool(items) => {
                // size | data0 | data1 | ...
                let len = items.len();
                buf.put_u32_le(len as u32);
                for item in items {
                    buf.put_u8(*item as u8);
                }
                (size_of::<u32>() + len) as u64
            }
            PropertyValue::ListInteger(items) => {
                // size | data0 | data1 |...
                let len = items.len();
                buf.put_u32_le(len as u32);
                for item in items {
                    buf.put_i64_le(*item);
                }
                (size_of::<u32>() + len * size_of::<i64>()) as u64
            }
            PropertyValue::ListFloat(items) => {
                // size | data0 | data1 |...
                let len = items.len();
                buf.put_u32_le(len as u32);
                for item in items {
                    buf.put_f64_le(*item);
                }
                (size_of::<u32>() + len * size_of::<f64>()) as u64
            }
            PropertyValue::ListString(items) => {
                // size | data0 | data1 |...
                let len = items.len();
                let mut size = 0;
                buf.put_u32_le(len as u32);
                for item in items {
                    buf.put_u32_le(item.len() as u32);
                    buf.put_slice(item.as_bytes());
                    size += size_of::<u32>() + item.len();
                }
                (size_of::<u32>() + size) as u64
            }
        }
    }
}

#[repr(C, packed(1))]
struct PropertyFormatKey {
    key_id: PropertyKeyId,
    dtype: u8,
    _padding: u8,
    value_size_or_value: u64,
}

pub struct PropertyWriter<'a> {
    buf: &'a mut BytesMut,
    num_keys: usize,
    curr_key: usize, // current key index
    keys: *mut PropertyFormatKey,
    offset: usize,
}

impl<'a> PropertyWriter<'a> {
    pub fn new(buf: &'a mut BytesMut, num_keys: usize) -> Self {
        let offset = buf.len();
        buf.reserve(PropertyFormat::header_and_keys_bytes(num_keys));
        // put header
        buf.put_u16_le(num_keys as u16);
        // init keys
        buf.put_bytes(0, PropertyFormat::keys_bytes(num_keys));

        let keys = {
            buf[offset + KEY_HEADER_BYTES..offset + PropertyFormat::keys_bytes(num_keys)].as_ptr()
                as *mut PropertyFormatKey
        };

        Self {
            buf,
            num_keys,
            curr_key: 0,
            keys,
            offset,
        }
    }

    pub fn add_property(&mut self, key_id: &PropertyKeyId, value: &PropertyValue) {
        assert!(self.curr_key < self.num_keys);
        let dtype = value.data_type();
        // put value
        let value_size_or_value = PropertyValueCodec::write(self.buf, value);
        // put key
        unsafe {
            let ptr = self.keys.add(self.curr_key);
            *ptr = PropertyFormatKey {
                key_id: *key_id,
                dtype: DataTypeCodec::encode(&dtype),
                _padding: 0,
                value_size_or_value,
            }
        }
        // advance
        self.curr_key += 1;
    }

    pub fn finish(self) -> usize {
        self.buf.len() - self.offset
    }
}
