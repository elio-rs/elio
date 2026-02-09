use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::mapb::entry::{EntryMut, EntryRef, EntryValueRef};
use crate::mapb::meta::EntryMeta;
use crate::scalar::*;

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct PropertyMap {
    // #layout
    // | len(u16)  | entry * len | value heap |
    // |    2      | 0..len      |            |
    pub(crate) data: Bytes,
}

impl PropertyMap {
    pub fn from_bytes(bytes: Bytes) -> Self {
        Self { data: bytes }
    }

    // number of entries in map
    pub fn len(&self) -> usize {
        (&self.data[0..2]).get_u16_le() as usize
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn as_ref(&self) -> PropertyMapRef<'_> {
        PropertyMapRef::new(self.data.as_ref())
    }

    pub fn bytes(&self) -> usize {
        self.data.len()
    }

    pub fn write<T: BufMut>(&self, buf: &mut T) {
        buf.put_slice(&self.data);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PropertyMapRef<'a> {
    // pointer to start of property map
    data: &'a [u8],
    // number of entries in map
    len: usize,
}

impl<'a> PropertyMapRef<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            len: (&data[0..2]).get_u16_le() as usize,
        }
    }

    pub fn bytes(&self) -> usize {
        self.data.len()
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn heap(&self) -> &[u8] {
        &self.data[2 + self.len * size_of::<EntryMeta>()..]
    }

    pub fn meta(&self) -> &[EntryMeta] {
        unsafe {
            let meta_bytes = &self.data[2..2 + self.len * size_of::<EntryMeta>()];
            let ptr = meta_bytes.as_ptr() as *const EntryMeta;
            std::slice::from_raw_parts(ptr, self.len)
        }
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = EntryRef<'_>> {
        self.meta().iter().map(|meta| EntryRef::new(self.heap(), meta))
    }

    pub fn to_owned(&self) -> PropertyMap {
        PropertyMap {
            data: self.data.to_vec().into(),
        }
    }

    // TODO(pgao): use binary search
    pub fn get(&self, key_id: u16) -> Option<EntryValueRef<'_>> {
        self.iter().find(|x| x.key() == key_id).map(|x| x.value())
    }
}

// In ProeprtyMapMut, entry meta's offset is actually the size of entry value.
// when freeze, the meta's offset will be the offset
pub struct PropertyMapMut {
    entries: Vec<EntryMut>,
}

impl PropertyMapMut {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: Vec::with_capacity(capacity),
        }
    }

    /// Return failed when property map value does not support given data type
    pub fn insert(&mut self, key_id: u16, datum: Option<&ScalarRef<'_>>) -> Result<(), String> {
        if datum.is_none() {
            self.entries.push(EntryMut::null(key_id));
            return Ok(());
        }

        let datum = datum.unwrap();
        let entry = match datum {
            ScalarRef::Null => EntryMut::null(key_id),
            ScalarRef::Bool(b) => EntryMut::bool(key_id, *b),
            ScalarRef::Integer(i) => EntryMut::integer(key_id, *i),
            ScalarRef::Float(ordered_float) => EntryMut::float(key_id, **ordered_float),
            ScalarRef::Date(date) => EntryMut::date(key_id, *date),
            ScalarRef::LocalTime(time) => EntryMut::local_time(key_id, *time),
            ScalarRef::LocalDateTime(dt) => EntryMut::local_date_time(key_id, *dt),
            ScalarRef::ZonedDateTime(dt) => EntryMut::zoned_date_time(key_id, *dt),
            ScalarRef::String(s) => EntryMut::string(key_id, s),
            ScalarRef::VirtualNode(_) | ScalarRef::VirtualRel(_) | ScalarRef::Node(_) | ScalarRef::Rel(_) => {
                return Err("node and rel cannot be property value".to_owned());
            }
            ScalarRef::List(list) => {
                // only integer, float, string list property are supported
                if let Some(i) = list.as_integer_list() {
                    EntryMut::list_integer(key_id, &i)
                } else if let Some(f) = list.as_float_list() {
                    EntryMut::list_float(key_id, &f)
                } else if let Some(s) = list.as_string_list() {
                    EntryMut::list_string(key_id, &s)
                } else {
                    return Err("list property must be integer, float, or string".to_owned());
                }
            }
            unsupported => return Err(format!("{:?} property type not supported", unsupported)),
        };
        self.entries.push(entry);
        Ok(())
    }

    pub fn insert_bool(&mut self, key_id: u16, value: bool) {
        self.entries.push(EntryMut::bool(key_id, value));
    }

    pub fn insert_string(&mut self, key_id: u16, value: &str) {
        self.entries.push(EntryMut::string(key_id, value));
    }

    pub fn insert_list_string(&mut self, key_id: u16, value: &[String]) {
        self.entries.push(EntryMut::list_string(key_id, value));
    }

    fn sort(&mut self) {
        self.entries.sort_by_key(|entry| entry.key_id());
    }

    // sort keys and serialize to buffer
    // serialized into
    // |num_entries(u16)| + |Entry(12B) * num_entries| + | value heap |
    pub fn freeze(mut self) -> PropertyMap {
        self.sort();
        let cap = size_of::<u16>() + self.entries.iter().map(|x| x.estimated_size()).sum::<usize>();
        let mut buf = BytesMut::with_capacity(cap);
        // u16
        buf.put_u16_le(self.entries.len() as u16);
        let mut offset = 0;
        // entry
        self.entries.iter_mut().for_each(|entry| {
            if !entry.is_inlined() {
                entry.key.set_offset(offset);
            }
            offset += entry.val.as_ref().map_or(0, |x| x.len());
            buf.put_slice(entry.key.0.as_slice());
        });
        // value heap
        self.entries.iter().for_each(|entry| {
            if let Some(ref val) = entry.val {
                buf.put_slice(&val.buffer);
            }
        });
        PropertyMap::from_bytes(buf.freeze())
    }
}
