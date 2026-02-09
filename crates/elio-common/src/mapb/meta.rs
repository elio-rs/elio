// #[repr(C, packed(1))]
// struct PropertyFormatKey {
//     key_id: PropertyKeyId,
//     dtype: u8,
//     _padding: u8,
//     value_size_or_value: u64,
// }

// Entry meta info
// key_id ::= u16
// type_tag ::= u8
// padding  ::= u8
// value_offset_or_value ::= u64 (value size)
//                         | bool(u8) with padding
//                         | Integer(i64)
//                         | Float(f64)

use bytes::Buf;

use crate::scalar::{Date, LocalTime};
// #layout
// | key_id (u16) | type_tag (u8) | padding (u8) | value_offset_or_value (u64) |
// |      2       |        1      |       1      |           8               |
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct EntryMeta(pub(crate) [u8; 12]);

pub const NULL_TAG: u8 = 0x00;
pub const BOOL_TAG: u8 = 0x01;
pub const INTEGER_TAG: u8 = 0x02;
pub const FLOAT_TAG: u8 = 0x03;
pub const DATE_TAG: u8 = 0x04;
pub const LOCAL_TIME_TAG: u8 = 0x05;
pub const LOCAL_DATE_TIME_TAG: u8 = 0x06;
pub const ZONED_TIME_TAG: u8 = 0x07;
pub const ZONED_DATE_TIME_TAG: u8 = 0x08;
pub const STRING_TAG: u8 = 0x09;
pub const LIST_BOOL_TAG: u8 = 0x10;
pub const LIST_INTEGER_TAG: u8 = 0x12;
pub const LIST_FLOAT_TAG: u8 = 0x13;
pub const LIST_STRING_TAG: u8 = 0x14;

impl EntryMeta {
    pub fn with_key_id(mut self, key_id: u16) -> Self {
        self.0[0..2].copy_from_slice(&key_id.to_le_bytes());
        self
    }

    pub fn with_null(mut self) -> Self {
        self.0[2] = NULL_TAG;
        self
    }

    pub fn with_true(mut self) -> Self {
        self.0[2] = BOOL_TAG;
        self.0[4] = 0x01;
        self
    }

    pub fn with_false(mut self) -> Self {
        self.0[2] = BOOL_TAG;
        self.0[4] = 0x00;
        self
    }

    pub fn with_integer(mut self, value: i64) -> Self {
        self.0[2] = INTEGER_TAG;
        self.0[4..12].copy_from_slice(&value.to_le_bytes());
        self
    }

    pub fn with_float(mut self, value: f64) -> Self {
        self.0[2] = FLOAT_TAG;
        self.0[4..12].copy_from_slice(&value.to_le_bytes());
        self
    }

    pub fn with_date(mut self, value: Date) -> Self {
        self.0[2] = DATE_TAG;
        self.0[4..12].copy_from_slice(&value.to_le_bytes());
        self
    }

    pub fn with_local_time(mut self, value: LocalTime) -> Self {
        self.0[2] = LOCAL_TIME_TAG;
        self.0[4..12].copy_from_slice(&value.to_le_bytes());
        self
    }

    pub fn with_local_date_time(mut self) -> Self {
        self.0[2] = LOCAL_DATE_TIME_TAG;
        self
    }

    pub fn with_zoned_time(mut self) -> Self {
        self.0[2] = ZONED_TIME_TAG;
        self
    }

    pub fn with_zoned_date_time(mut self) -> Self {
        self.0[2] = ZONED_DATE_TIME_TAG;
        self
    }

    pub fn with_string(mut self) -> Self {
        self.0[2] = STRING_TAG;
        self
    }

    pub fn with_list_bool(mut self) -> Self {
        self.0[2] = LIST_BOOL_TAG;
        self
    }

    pub fn with_list_integer(mut self) -> Self {
        self.0[2] = LIST_INTEGER_TAG;
        self
    }

    pub fn with_list_float(mut self) -> Self {
        self.0[2] = LIST_FLOAT_TAG;
        self
    }

    pub fn with_list_string(mut self) -> Self {
        self.0[2] = LIST_STRING_TAG;
        self
    }

    pub fn set_offset(&mut self, offset: usize) {
        self.0[4..12].copy_from_slice(&offset.to_le_bytes());
    }
}

impl EntryMeta {
    pub fn type_tag(&self) -> u8 {
        self.0[2]
    }

    pub fn key_id(&self) -> u16 {
        u16::from_le_bytes(self.0[0..2].try_into().unwrap())
    }

    pub fn as_bool(&self) -> bool {
        self.0[4] == 0x01
    }

    pub fn as_integer(&self) -> i64 {
        (&self.0[4..12]).get_i64_le()
    }

    pub fn as_float(&self) -> f64 {
        (&self.0[4..12]).get_f64_le()
    }

    pub fn as_date(&self) -> Date {
        Date::from_le_bytes(self.0[4..12].try_into().unwrap())
    }

    pub fn as_local_time(&self) -> LocalTime {
        LocalTime::from_le_bytes(self.0[4..12].try_into().unwrap())
    }

    pub fn offset(&self) -> usize {
        (&self.0[4..12]).get_u64_le() as usize
    }

    pub fn is_inlined(&self) -> bool {
        matches!(
            self.type_tag(),
            NULL_TAG | BOOL_TAG | INTEGER_TAG | FLOAT_TAG | DATE_TAG | LOCAL_TIME_TAG
        )
    }
}
