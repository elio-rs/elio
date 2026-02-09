//! Encode datatypes into u8 bytes.
//! | b7 b6 b5 b4 b3 b2 b1 b0 |
//! | inner_type | outer_type  |
//! for nested type like list, the inner type should be encoded.

use elio_common::data_type::DataType;
use elio_common::store_types::StoreDataType;

const INNER_TYPE_SHIFT: u8 = 4;
const OUTER_TYPE_MASK: u8 = 0x0F;

macro_rules! register_type_id  {
    ($($constant_name:ident = $value:expr),* $(,)?) => {
        $(
            pub const $constant_name: u8 = $value;
        )*
    };
}

register_type_id!(
    TYPE_ID_NULL = 0,
    TYPE_ID_BOOLEAN = 1,
    TYPE_ID_INTEGER = 2,
    TYPE_ID_FLOAT = 3,
    TYPE_ID_STRING = 4,
    TYPE_ID_LIST = 5,
);

pub struct DataTypeCodec;

impl DataTypeCodec {
    // there's an requirment on whether the datatype can be stored as a property value
    pub fn storable(data_type: &DataType) -> bool {
        match data_type {
            DataType::Null | DataType::Bool | DataType::Integer | DataType::Float | DataType::String => true,
            DataType::List(data_type) => data_type.is_primitive(),
            DataType::Node | DataType::Rel | DataType::Path | DataType::Property | DataType::PropertyMap => false,
            DataType::U16 => todo!(),
            DataType::NodeId => todo!(),
            DataType::RelId => todo!(),
        }
    }

    pub fn encode(data_type: &StoreDataType) -> u8 {
        match data_type {
            StoreDataType::Null => TYPE_ID_NULL,
            StoreDataType::Boolean => TYPE_ID_BOOLEAN,
            StoreDataType::Integer => TYPE_ID_INTEGER,
            StoreDataType::Float => TYPE_ID_FLOAT,
            StoreDataType::String => TYPE_ID_STRING,
            StoreDataType::List(store_data_type) => {
                let inner_type = Self::encode(store_data_type);
                TYPE_ID_LIST | (inner_type << INNER_TYPE_SHIFT)
            }
        }
    }
}

impl DataTypeCodec {
    pub fn decode(data_type: u8) -> DataType {
        let outer_type = data_type & OUTER_TYPE_MASK;
        let inner_type = data_type >> INNER_TYPE_SHIFT;
        match outer_type {
            TYPE_ID_NULL => DataType::Null,
            TYPE_ID_BOOLEAN => DataType::Bool,
            TYPE_ID_INTEGER => DataType::Integer,
            TYPE_ID_FLOAT => DataType::Float,
            TYPE_ID_STRING => DataType::String,
            TYPE_ID_LIST => {
                let inner_type = Self::decode(inner_type);
                DataType::List(Box::new(inner_type))
            }
            _ => unreachable!("Invalid data type"),
        }
    }
}
