use derive_more;

use crate::variable::VariableName;

#[derive(Clone, Debug, Copy, Default, PartialEq, Eq, derive_more::Display)]
pub enum SortDirection {
    #[default]
    Asc,
    Desc,
}

#[derive(Debug, Clone, derive_more::Display)]
#[display("{} {}", column, direction)]
pub struct ColumnOrder {
    pub column: VariableName,
    pub direction: SortDirection,
}
