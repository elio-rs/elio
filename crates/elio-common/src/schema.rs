use std::collections::HashMap;
use std::sync::Arc;

use crate::data_type::DataType;
use crate::variable::VariableName;

pub type Name2ColumnMap = HashMap<VariableName, usize>;

#[derive(Debug, Default, Clone, PartialEq, Eq, Hash)]
pub struct Schema {
    pub fields: Vec<Variable>,
}

impl Schema {
    pub fn empty() -> Self {
        Self { fields: Vec::new() }
    }

    pub fn from_arc(arc: Arc<Schema>) -> Self {
        (*arc).clone()
    }

    pub fn add_column(&mut self, var: Variable) {
        self.fields.push(var);
    }

    pub fn name_to_col_map(&self) -> Name2ColumnMap {
        let mut map = HashMap::new();
        for (i, var) in self.fields.iter().enumerate() {
            map.insert(var.name.clone(), i);
        }
        map
    }

    pub fn column(&self, idx: usize) -> &Variable {
        &self.fields[idx]
    }

    pub fn columns(&self) -> &[Variable] {
        &self.fields
    }

    pub fn column_by_name(&self, name: &VariableName) -> Option<&Variable> {
        self.fields.iter().find(|x| x.name == *name)
    }

    pub fn len(&self) -> usize {
        self.fields.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn iter(&self) -> impl Iterator<Item = &Variable> {
        self.fields.iter()
    }
}

impl FromIterator<Variable> for Schema {
    fn from_iter<T: IntoIterator<Item = Variable>>(iter: T) -> Self {
        Self {
            fields: iter.into_iter().collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Variable {
    pub name: VariableName,
    pub typ: DataType,
}

impl Variable {
    pub fn new(name: &VariableName, typ: &DataType) -> Self {
        Self {
            name: name.clone(),
            typ: typ.clone(),
        }
    }

    #[inline]
    pub fn is_node(&self) -> bool {
        self.typ.is_node()
    }

    #[inline]
    pub fn is_rel(&self) -> bool {
        self.typ.is_rel()
    }

    #[inline]
    pub fn is_entity(&self) -> bool {
        self.typ.is_entity()
    }
}
