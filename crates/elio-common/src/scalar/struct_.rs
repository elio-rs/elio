use super::*;

#[derive(Debug, Clone, Default, Eq, PartialEq, Hash, derive_more::Display)]
#[display("{{{}}}", fields.iter().map(|(k, v)| format!("{}: {}", k, v)).collect::<Vec<_>>().join(", "))]
pub struct StructValue {
    // fields must be unique and ordered
    fields: Vec<(Arc<str>, ScalarValue)>,
}

impl ScalarVTable for StructValue {
    type RefType<'a> = StructValueRef<'a>;

    fn as_scalar_ref(&self) -> Self::RefType<'_> {
        StructValueRef::Value { value: self }
    }
}

impl ScalarPartialOrd for StructValue {
    fn scalar_partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.as_scalar_ref().scalar_partial_cmp(&other.as_scalar_ref())
    }
}

impl StructValue {
    pub fn new(fields: Vec<(Arc<str>, ScalarValue)>) -> Self {
        Self { fields }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Arc<str>, &ScalarValue)> {
        self.fields.iter().map(|(k, v)| (k, v))
    }

    pub fn field_at_pos(&self, idx: usize) -> Option<&(Arc<str>, ScalarValue)> {
        self.fields.get(idx)
    }

    pub fn field_at(&self, name: &str) -> Option<ScalarRef<'_>> {
        self.fields
            .iter()
            .find(|(k, _)| **k == *name)
            .map(|(_, v)| v.as_scalar_ref())
    }

    pub fn as_scalar_ref(&self) -> StructValueRef<'_> {
        StructValueRef::Value { value: self }
    }

    pub fn len(&self) -> usize {
        self.fields.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[derive(Debug, Clone, Copy, derive_more::Display)]
#[display("{{{}}}", self.iter().map(|(k, v)| format!("{}: {}", k, v)).collect::<Vec<_>>().join(", "))]
pub enum StructValueRef<'a> {
    Index { array: &'a StructArray, idx: usize },
    Value { value: &'a StructValue },
}

impl<'a> PartialEq for StructValueRef<'a> {
    fn eq(&self, other: &Self) -> bool {
        self.iter().eq(other.iter())
    }
}

impl<'a> Eq for StructValueRef<'a> {}

impl<'a> std::hash::Hash for StructValueRef<'a> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.iter().for_each(|(k, v)| {
            k.hash(state);
            v.hash(state);
        })
    }
}

impl<'a> ScalarPartialOrd for StructValueRef<'a> {
    fn scalar_partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        // first compare length then compare fields
        match self.len().partial_cmp(&other.len()) {
            Some(std::cmp::Ordering::Equal) => {
                let iter1 = self.iter();
                let iter2 = other.iter();
                for ((k1, v1), (k2, v2)) in iter1.zip(iter2) {
                    match k1.cmp(k2) {
                        std::cmp::Ordering::Equal => match v1.scalar_partial_cmp(&v2) {
                            Some(std::cmp::Ordering::Equal) => continue,
                            ord => return ord,
                        },
                        ord => return Some(ord),
                    }
                }
                Some(std::cmp::Ordering::Equal)
            }
            ord => ord,
        }
    }
}

impl<'a> StructValueRef<'a> {
    pub fn field_at(&self, name: &str) -> Option<ScalarRef<'a>> {
        match self {
            StructValueRef::Index { array, idx } => array.field_at(name).and_then(|arr| arr.get(*idx)),
            StructValueRef::Value { value } => value.field_at(name),
        }
    }

    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        match self {
            StructValueRef::Index { array, .. } => array.fields().len(),
            StructValueRef::Value { value } => value.fields.len(),
        }
    }

    pub fn iter(&self) -> StructValueRefIter<'a> {
        StructValueRefIter {
            struct_ref: *self,
            pos: 0,
            len: self.len(),
        }
    }
}

impl<'a> ScalarRefVTable<'a> for StructValueRef<'a> {
    type ScalarType = StructValue;

    fn to_owned_scalar(&self) -> Self::ScalarType {
        match self {
            StructValueRef::Index { array, idx } => {
                let mut fields = vec![];
                for (name, array) in array.fields() {
                    let value = array.get(*idx).unwrap().to_owned_scalar();
                    fields.push((name.clone(), value));
                }
                StructValue { fields }
            }
            StructValueRef::Value { value } => (*value).clone(),
        }
    }
}

pub struct StructValueRefIter<'a> {
    struct_ref: StructValueRef<'a>,
    pos: usize,
    len: usize,
}

impl<'a> Iterator for StructValueRefIter<'a> {
    type Item = (&'a Arc<str>, ScalarRef<'a>);

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos < self.len {
            let item = match self.struct_ref {
                StructValueRef::Index { array, idx } => array
                    .field_at_pos(self.pos)
                    .map(|(name, array)| (name, array.get(idx).unwrap_or(ScalarRef::Null))),
                StructValueRef::Value { value } => value
                    .field_at_pos(self.pos)
                    .map(|(name, value)| (name, value.as_scalar_ref())),
            };
            self.pos += 1;
            item
        } else {
            None
        }
    }
}

impl<'a> ExactSizeIterator for StructValueRefIter<'a> {
    fn len(&self) -> usize {
        self.len
    }
}
