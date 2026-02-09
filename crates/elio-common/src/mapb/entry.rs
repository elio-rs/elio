use core::panic;
use std::marker::PhantomData;

use bytes::{Buf, BufMut, BytesMut};
use itertools::Itertools;

use crate::data_type::F64;
use crate::mapb::meta::*;
use crate::mapb::{DATE_TAG, LOCAL_DATE_TIME_TAG};
use crate::scalar::*;

pub struct EntryRef<'a> {
    // # layout
    // data: pointer to PropertyMap heap region
    // meta: pointer to this entry's meta region
    data: &'a [u8],
    meta: &'a EntryMeta,
}

impl<'a> EntryRef<'a> {
    pub fn new(data: &'a [u8], meta: &'a EntryMeta) -> Self {
        Self { data, meta }
    }

    pub fn key(&self) -> u16 {
        self.meta.key_id()
    }

    pub fn value(&self) -> EntryValueRef<'a> {
        match self.meta.type_tag() {
            NULL_TAG => EntryValueRef::Bool(false),
            BOOL_TAG => EntryValueRef::Bool(self.meta.as_bool()),
            INTEGER_TAG => EntryValueRef::Integer(self.meta.as_integer()),
            FLOAT_TAG => EntryValueRef::Float(self.meta.as_float()),
            DATE_TAG => EntryValueRef::Date(self.meta.as_date()),
            LOCAL_TIME_TAG => EntryValueRef::LocalTime(self.meta.as_local_time()),
            LOCAL_DATE_TIME_TAG => {
                let data_slice = &self.data[self.meta.offset()..];
                EntryValueRef::LocalDateTime(LocalDateTime::from_le_bytes(
                    data_slice[..LocalDateTime::STORAGE_BYTES].try_into().unwrap(),
                ))
            }
            ZONED_DATE_TIME_TAG => {
                let data_slice = &self.data[self.meta.offset()..];
                EntryValueRef::ZonedDateTime(ZonedDateTime::from_le_bytes(
                    data_slice[..ZonedDateTime::STORAGE_BYTES].try_into().unwrap(),
                ))
            }
            STRING_TAG => EntryValueRef::String(unsafe {
                let data_slice = &self.data[self.meta.offset()..];
                let len = (&data_slice[..4]).get_u32_le() as usize;
                std::str::from_utf8_unchecked(&data_slice[4..4 + len])
            }),
            LIST_BOOL_TAG => {
                let data_slice = &self.data[self.meta.offset()..];
                let len = (&data_slice[..4]).get_u32_le() as usize;
                let data_slice = &data_slice[4..];
                EntryValueRef::ListBool(unsafe { std::mem::transmute::<&[u8], &[bool]>(&data_slice[..len]) })
            }
            LIST_INTEGER_TAG => EntryValueRef::ListInteger(PrimitiveListRef::new(&self.data[self.meta.offset()..])),
            LIST_FLOAT_TAG => EntryValueRef::ListFloat(PrimitiveListRef::new(&self.data[self.meta.offset()..])),
            LIST_STRING_TAG => EntryValueRef::ListString(ListStringRef::new(&self.data[self.meta.offset()..])),
            _ => {
                panic!("data corruption")
            }
        }
    }
}

// referenced value in propertymap
pub enum EntryValueRef<'a> {
    Null,
    Bool(bool),
    Integer(i64),
    Float(f64),
    Date(Date),
    LocalTime(LocalTime),
    LocalDateTime(LocalDateTime),
    ZonedDateTime(ZonedDateTime),
    String(&'a str),
    ListBool(&'a [bool]),
    ListInteger(PrimitiveListRef<'a, i64>),
    ListFloat(PrimitiveListRef<'a, f64>),
    ListString(ListStringRef<'a>),
}

impl<'a> EntryValueRef<'a> {
    pub fn to_owned_scalar(&self) -> ScalarValue {
        match self {
            EntryValueRef::Null => ScalarValue::Unknown,
            EntryValueRef::Bool(b) => ScalarValue::Bool(*b),
            EntryValueRef::Integer(i) => ScalarValue::Integer(*i),
            EntryValueRef::Float(f) => ScalarValue::Float(F64::from(*f)),
            EntryValueRef::Date(d) => ScalarValue::Date(*d),
            EntryValueRef::LocalTime(t) => ScalarValue::LocalTime(*t),
            EntryValueRef::LocalDateTime(dt) => ScalarValue::LocalDateTime(*dt),
            EntryValueRef::ZonedDateTime(dt) => ScalarValue::ZonedDateTime(*dt),
            EntryValueRef::String(s) => ScalarValue::String(s.to_string()),
            EntryValueRef::ListBool(list) => ScalarValue::List(Box::new(ListValue::new(
                list.iter().map(|x| ScalarValue::Bool(*x)).collect_vec(),
            ))),
            EntryValueRef::ListInteger(list) => ScalarValue::List(Box::new(ListValue::new(
                list.iter().map(ScalarValue::Integer).collect_vec(),
            ))),
            EntryValueRef::ListFloat(list) => ScalarValue::List(Box::new(ListValue::new(
                list.iter().map(|x| ScalarValue::Float(F64::from(x))).collect_vec(),
            ))),
            EntryValueRef::ListString(list) => ScalarValue::List(Box::new(ListValue::new(
                list.iter().map(|x| ScalarValue::String(x.to_owned())).collect_vec(),
            ))),
        }
    }
}

impl<'a> EntryValueRef<'a> {
    pub fn pretty(&self) -> String {
        match self {
            EntryValueRef::Null => "NULL".to_string(),
            EntryValueRef::Bool(b) => b.to_string(),
            EntryValueRef::Integer(i) => i.to_string(),
            EntryValueRef::Float(f) => f.to_string(),
            EntryValueRef::Date(d) => d.to_string(),
            EntryValueRef::LocalTime(t) => t.to_string(),
            EntryValueRef::LocalDateTime(dt) => dt.to_string(),
            EntryValueRef::ZonedDateTime(dt) => dt.to_string(),
            EntryValueRef::String(s) => s.to_string(),
            EntryValueRef::ListBool(items) => format!(
                "[{}]",
                items.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(", ")
            ),
            EntryValueRef::ListInteger(primitive_list_ref) => format!(
                "[{}]",
                primitive_list_ref
                    .iter()
                    .map(|x| x.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            EntryValueRef::ListFloat(primitive_list_ref) => format!(
                "[{}]",
                primitive_list_ref
                    .iter()
                    .map(|x| x.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            EntryValueRef::ListString(list_string_ref) => format!(
                "[{}]",
                list_string_ref
                    .iter()
                    .map(|x| x.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }
}

pub struct ListStringRef<'a> {
    // # layout
    // | len (u32) | str_len(u32) | str_bytes    | ...
    // |    4      |    4         | str_len      |
    data: &'a [u8], // pointer to start of liststring
    len: usize,
}
impl<'a> ListStringRef<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            len: (&data[0..4]).get_u32_le() as usize,
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

pub struct ListStringRefIter<'a> {
    data: &'a [u8], // pointer to start of liststring
    pos: usize,     // current element offset from start of buf
    idx: usize,     // current element idx
    len: usize,     // number of strings
}

impl<'a> ListStringRefIter<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        let len = (&buf[0..4]).get_u32_le() as usize;
        Self {
            data: buf,
            pos: 4,
            idx: 0,
            len,
        }
    }
}

impl<'a> ExactSizeIterator for ListStringRefIter<'a> {
    fn len(&self) -> usize {
        let (lower, upper) = self.size_hint();
        // Note: This assertion is overly defensive, but it checks the invariant
        // guaranteed by the trait. If this trait were rust-internal,
        // we could use debug_assert!; assert_eq! will check all Rust user
        // implementations too.
        std::assert_eq!(upper, Some(lower));
        lower
    }
}

impl<'a> Iterator for ListStringRefIter<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        if self.idx >= self.len {
            return None;
        }
        let str_len = (&self.data[self.pos..self.pos + 4]).get_u32_le() as usize;
        self.pos += 4;
        let slice = &self.data[self.pos..self.pos + str_len];
        self.pos += str_len;
        self.idx += 1;
        Some(std::str::from_utf8(slice).unwrap())
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}

impl<'a> ListStringRef<'a> {
    pub fn iter(&self) -> impl Iterator<Item = &'a str> {
        ListStringRefIter::new(self.data)
    }
}

// # layout
// | len (u32) | values    | ...
// |    4      |           |
pub struct PrimitiveListRef<'a, T: Copy> {
    // pointer to start of list
    data: &'a [u8],
    len: usize,
    _marker: PhantomData<T>,
}

impl<'a, T: Copy> PrimitiveListRef<'a, T> {
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            len: (&data[0..4]).get_u32_le() as usize,
            _marker: PhantomData,
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl<'a> PrimitiveListRef<'a, i64> {
    pub fn iter(&self) -> impl ExactSizeIterator<Item = i64> {
        self.data[4..]
            .chunks_exact(size_of::<i64>())
            .map(|chunk| (&chunk[..size_of::<i64>()]).get_i64_le())
    }
}

impl<'a> PrimitiveListRef<'a, f64> {
    pub fn iter(&self) -> impl ExactSizeIterator<Item = f64> {
        self.data[4..]
            .chunks_exact(size_of::<f64>())
            .map(|chunk| (&chunk[..size_of::<f64>()]).get_f64_le())
    }
}

pub struct EntryValueMut {
    pub(crate) buffer: BytesMut,
}

impl EntryValueMut {
    pub fn with_capacity(capacity: usize) -> Self {
        let buffer = BytesMut::with_capacity(capacity);
        Self { buffer }
    }

    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn set_string(&mut self, value: &str) {
        self.buffer.put_u32_le(value.len() as u32);
        self.buffer.put_slice(value.as_bytes());
    }

    pub fn set_list_bool(&mut self, value: &[bool]) {
        // size | data0 | data1 | ...
        self.buffer.put_u32_le(value.len() as u32);
        value.iter().for_each(|v| self.buffer.put_u8(*v as u8));
    }

    pub fn set_list_integer(&mut self, value: &[i64]) {
        // size | data0 | data1 | ...
        self.buffer.put_u32_le(value.len() as u32);
        value.iter().for_each(|v| self.buffer.put_i64_le(*v));
    }

    pub fn set_list_float(&mut self, value: &[F64]) {
        // size | data0 | data1 | ...
        self.buffer.put_u32_le(value.len() as u32);
        value.iter().for_each(|v| self.buffer.put_f64_le(**v));
    }

    pub fn set_list_string(&mut self, value: &[String]) {
        // size | data0 | data1 | ...
        self.buffer.put_u32_le(value.len() as u32);
        value.iter().for_each(|v| {
            self.buffer.put_u32_le(v.len() as u32);
            self.buffer.put_slice(v.as_bytes());
        });
    }
}

pub struct EntryMut {
    pub(crate) key: EntryMeta,
    pub(crate) val: Option<EntryValueMut>,
}

impl EntryMut {
    pub fn null(key_id: u16) -> Self {
        let key = EntryMeta::default().with_key_id(key_id).with_null();
        Self { key, val: None }
    }

    pub fn bool(key_id: u16, value: bool) -> Self {
        let mut key = EntryMeta::default().with_key_id(key_id);
        key = match value {
            true => key.with_true(),
            false => key.with_false(),
        };
        Self { key, val: None }
    }

    pub fn integer(key_id: u16, value: i64) -> Self {
        let mut key = EntryMeta::default().with_key_id(key_id);
        key = key.with_integer(value);
        Self { key, val: None }
    }

    pub fn float(key_id: u16, value: f64) -> Self {
        let mut key = EntryMeta::default().with_key_id(key_id);
        key = key.with_float(value);
        Self { key, val: None }
    }

    pub fn date(key_id: u16, value: Date) -> Self {
        let mut key = EntryMeta::default().with_key_id(key_id);
        key = key.with_date(value);
        Self { key, val: None }
    }

    pub fn local_time(key_id: u16, value: LocalTime) -> Self {
        let mut key = EntryMeta::default().with_key_id(key_id);
        key = key.with_local_time(value);
        Self { key, val: None }
    }

    pub fn local_date_time(key_id: u16, value: LocalDateTime) -> Self {
        let mut key = EntryMeta::default().with_key_id(key_id);
        key = key.with_local_date_time();
        let mut val = EntryValueMut::with_capacity(LocalDateTime::STORAGE_BYTES);
        val.buffer.put_slice(&value.to_le_bytes());
        Self { key, val: Some(val) }
    }

    pub fn zoned_date_time(key_id: u16, value: ZonedDateTime) -> Self {
        let mut key = EntryMeta::default().with_key_id(key_id);
        key = key.with_zoned_date_time();
        let mut val = EntryValueMut::with_capacity(ZonedDateTime::STORAGE_BYTES);
        val.buffer.put_slice(&value.to_le_bytes());
        Self { key, val: Some(val) }
    }

    pub fn string(key_id: u16, value: &str) -> Self {
        let key = EntryMeta::default().with_key_id(key_id).with_string();
        let mut val = EntryValueMut::with_capacity(value.len());
        val.set_string(value);
        Self { key, val: Some(val) }
    }

    pub fn list_bool(key_id: u16, value: &[bool]) -> Self {
        let key = EntryMeta::default().with_key_id(key_id).with_list_bool();
        let mut val = EntryValueMut::with_capacity(value.len());
        val.set_list_bool(value);
        Self { key, val: Some(val) }
    }

    pub fn list_integer(key_id: u16, value: &[i64]) -> Self {
        let key = EntryMeta::default().with_key_id(key_id).with_list_integer();
        let mut val = EntryValueMut::with_capacity(std::mem::size_of_val(value));
        val.set_list_integer(value);
        Self { key, val: Some(val) }
    }

    pub fn list_float(key_id: u16, value: &[F64]) -> Self {
        let key = EntryMeta::default().with_key_id(key_id).with_list_float();
        let mut val = EntryValueMut::with_capacity(std::mem::size_of_val(value));
        val.set_list_float(value);
        Self { key, val: Some(val) }
    }

    // TODO(pgao): this is not a good repr,
    // we should have an list array type here.
    pub fn list_string(key_id: u16, value: &[String]) -> Self {
        let key = EntryMeta::default().with_key_id(key_id).with_list_string();
        let cap = size_of::<u32>() + value.iter().map(|x| x.len()).sum::<usize>() + size_of::<u32>() * value.len();
        let mut val = EntryValueMut::with_capacity(cap);
        val.set_list_string(value);
        Self { key, val: Some(val) }
    }

    pub fn key_id(&self) -> u16 {
        self.key.key_id()
    }

    // true on entry value is inlined in entry meta
    pub fn is_inlined(&self) -> bool {
        self.key.is_inlined()
    }

    pub fn estimated_size(&self) -> usize {
        size_of::<EntryMeta>() + self.val.as_ref().map_or(0, |x| x.buffer.len())
    }
}
