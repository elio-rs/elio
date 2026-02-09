use crate::ir::query::{Bindings, IrSingleQuery, IrSingleQueryPart};

pub struct IrSingleQueryBuilder {
    parts: Vec<IrSingleQueryPart>,
    // imported variables are stored in bctx::outer_scopes
    // we do not have imported variables here,
    // since when binding variables we need the symbol name, not variablename
}

impl Default for IrSingleQueryBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl IrSingleQueryBuilder {
    pub fn new() -> Self {
        Self {
            parts: vec![IrSingleQueryPart::new(Bindings::default())],
        }
    }

    pub fn new_tail(&mut self, input_binding: Bindings) {
        self.parts.push(IrSingleQueryPart::new(input_binding));
    }

    pub fn tail_mut(&mut self) -> Option<&mut IrSingleQueryPart> {
        self.parts.last_mut()
    }

    pub fn build(self) -> IrSingleQuery {
        IrSingleQuery { parts: self.parts }
    }
}
