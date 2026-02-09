use super::*;

#[derive(Debug, Clone, Default, Eq, PartialEq, Hash, derive_more::Display)]
#[display("[{}]", values.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(", "))]
pub struct ListValue {
    values: Vec<ScalarValue>,
}

impl ScalarVTable for ListValue {
    type RefType<'a> = ListValueRef<'a>;

    fn as_scalar_ref(&self) -> Self::RefType<'_> {
        ListValueRef::Slice(&self.values)
    }
}

impl ScalarPartialOrd for ListValue {
    fn scalar_partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.as_scalar_ref().scalar_partial_cmp(&other.as_scalar_ref())
    }
}

impl ListValue {
    pub fn new(values: Vec<ScalarValue>) -> Self {
        Self { values }
    }
}

#[derive(Debug, Clone, Copy, derive_more::Display)]
#[display("[{}]", self.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(", "))]
pub enum ListValueRef<'a> {
    Index {
        child: &'a ArrayImpl,
        start: usize,
        end: usize,
    },
    Slice(&'a [ScalarValue]),
}

impl<'a> ListValueRef<'a> {
    pub fn from_array(array: &'a ArrayImpl, start: usize, end: usize) -> Self {
        Self::Index {
            child: array,
            start,
            end,
        }
    }

    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        match self {
            ListValueRef::Index { child: _, start, end } => end - start,
            ListValueRef::Slice(scalar_values) => scalar_values.len(),
        }
    }

    pub fn iter(&self) -> ListValueIter<'a> {
        ListValueIter { list: *self, idx: 0 }
    }

    pub fn as_integer_list(&self) -> Option<Vec<i64>> {
        let mut vec = vec![];
        for item in self.iter() {
            if let ScalarRef::Integer(b) = item {
                vec.push(b);
            } else {
                return None;
            }
        }
        Some(vec)
    }

    pub fn as_float_list(&self) -> Option<Vec<F64>> {
        let mut vec = vec![];
        for item in self.iter() {
            if let ScalarRef::Float(b) = item {
                vec.push(b);
            } else {
                return None;
            }
        }
        Some(vec)
    }

    pub fn as_string_list(&self) -> Option<Vec<String>> {
        let mut vec = vec![];
        for item in self.iter() {
            if let ScalarRef::String(b) = item {
                vec.push(b.to_owned());
            } else {
                return None;
            }
        }
        Some(vec)
    }
}

impl<'a> PartialEq for ListValueRef<'a> {
    fn eq(&self, other: &Self) -> bool {
        self.iter().eq(other.iter())
    }
}

impl<'a> Eq for ListValueRef<'a> {}

impl<'a> std::hash::Hash for ListValueRef<'a> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        for item in self.iter() {
            item.hash(state);
        }
    }
}

pub struct ListValueIter<'a> {
    list: ListValueRef<'a>,
    idx: usize,
}

impl<'a> Iterator for ListValueIter<'a> {
    // TODO(pgao): maybe we should use option here?
    type Item = ScalarRef<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.list {
            ListValueRef::Index { child, start, end } => {
                if self.idx >= end - start {
                    None
                } else {
                    let item = child.get(start + self.idx).unwrap_or(ScalarRef::Null);
                    self.idx += 1;
                    Some(item)
                }
            }
            ListValueRef::Slice(scalar_values) => {
                if self.idx >= scalar_values.len() {
                    None
                } else {
                    let item = &scalar_values[self.idx];
                    self.idx += 1;
                    Some(item.as_scalar_ref())
                }
            }
        }
    }
}

impl<'a> ScalarRefVTable<'a> for ListValueRef<'a> {
    type ScalarType = ListValue;

    fn to_owned_scalar(&self) -> Self::ScalarType {
        ListValue {
            values: self.iter().map(|x| x.to_owned_scalar()).collect_vec(),
        }
    }
}

impl<'a> ScalarPartialOrd for ListValueRef<'a> {
    fn scalar_partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        for (v1, v2) in self.iter().zip(other.iter()) {
            match v1.scalar_partial_cmp(&v2) {
                Some(std::cmp::Ordering::Equal) => (),
                ord => return ord,
            }
        }
        self.len().partial_cmp(&other.len())
    }
}
