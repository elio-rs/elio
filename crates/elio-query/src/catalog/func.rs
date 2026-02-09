use crate::function::scalar::sig::FuncDef;

#[derive(Clone, Debug)]
pub struct FunctionCatalog {
    pub name: String,
    pub func: FuncDef,
    // TODO(pgao): other fields here
}

impl FunctionCatalog {
    pub fn new(name: String, func: FuncDef) -> Self {
        Self { name, func }
    }
}
