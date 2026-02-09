use elio_common::order::ColumnOrder;

#[derive(Debug, Clone)]
pub struct Ordering {
    pub items: Vec<ColumnOrder>,
}

impl Ordering {
    pub fn empty() -> Self {
        Self {
            items: Default::default(),
        }
    }
}
