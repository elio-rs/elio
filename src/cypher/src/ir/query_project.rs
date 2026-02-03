use std::sync::Arc;

use elio_common::data_type::DataType;
use elio_common::schema::Variable;
use elio_common::variable::VariableName;
use enum_as_inner::EnumAsInner;
use indexmap::{IndexMap, IndexSet};
use pretty_xmlish::{Pretty, XmlNode};

use crate::expr::{Expr, ExprNode, FilterExprs};
use crate::ir::order::SortItem;
use crate::pretty_utils::{pretty_order_items, pretty_project_items};

// Rename to QueryProjection
pub enum QueryProjection {
    Unwind(Unwind),
    Project(Projection),
    Load(Load),
}

impl QueryProjection {
    pub fn xmlnode(&self) -> XmlNode<'_> {
        match self {
            QueryProjection::Unwind(u) => u.xmlnode(),
            QueryProjection::Project(p) => p.xmlnode(),
            QueryProjection::Load(l) => l.xmlnode(),
        }
    }
}

impl QueryProjection {
    pub fn set_order_by(&mut self, order_by: Vec<SortItem>) {
        match self {
            QueryProjection::Unwind(_) => unreachable!(),
            QueryProjection::Project(q) => q.set_order_by(order_by),
            QueryProjection::Load(_l) => unreachable!(),
        }
    }

    pub fn set_pagination(&mut self, pagination: Pagination) {
        match self {
            QueryProjection::Unwind(_) => unreachable!(),
            QueryProjection::Project(q) => q.set_pagination(pagination),
            QueryProjection::Load(_) => unreachable!(),
        }
    }

    pub fn set_filter(&mut self, filter: FilterExprs) {
        match self {
            QueryProjection::Unwind(_) => unreachable!(),
            QueryProjection::Project(q) => q.set_filter(filter),
            QueryProjection::Load(_) => unreachable!(),
        }
    }
}

impl std::default::Default for QueryProjection {
    fn default() -> Self {
        Self::Project(Projection::empty())
    }
}

impl QueryProjection {
    pub fn empty() -> Self {
        Self::default()
    }
}

pub enum Projection {
    Regular(RegularProjection),
    Aggregate(AggregateProjection),
    Distinct(DistinctProjection),
}
impl Projection {
    pub fn xmlnode(&self) -> XmlNode<'_> {
        match self {
            Projection::Regular(r) => r.xmlnode(),
            Projection::Aggregate(a) => a.xmlnode(),
            Projection::Distinct(d) => d.xmlnode(),
        }
    }
}

impl Projection {
    pub fn empty() -> Self {
        Self::Regular(RegularProjection::default())
    }

    pub fn set_order_by(&mut self, order_by: Vec<SortItem>) {
        match self {
            Projection::Regular(r) => r.order_by = order_by,
            Projection::Aggregate(a) => a.order_by = order_by,
            Projection::Distinct(d) => d.order_by = order_by,
        }
    }

    pub fn set_pagination(&mut self, pagination: Pagination) {
        match self {
            Projection::Regular(r) => r.pagination = pagination,
            Projection::Aggregate(a) => a.pagination = pagination,
            Projection::Distinct(d) => d.pagination = pagination,
        }
    }

    pub fn set_filter(&mut self, filter: FilterExprs) {
        match self {
            Projection::Regular(r) => r.filter = filter,
            Projection::Aggregate(a) => a.filter = filter,
            Projection::Distinct(d) => d.filter = filter,
        }
    }
}

pub struct Pagination {
    pub offset: u64,
    pub limit: u64,
}

impl std::default::Default for Pagination {
    fn default() -> Self {
        Self {
            offset: 0,
            limit: u64::MAX,
        }
    }
}

impl std::fmt::Display for Pagination {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.offset != 0 {
            write!(f, "offset: {}", self.offset)?;
        }
        if self.limit != u64::MAX {
            write!(f, " limit: {}", self.limit)?;
        }
        Ok(())
    }
}

impl Pagination {
    pub fn is_empty(&self) -> bool {
        self.offset == 0 && self.limit == u64::MAX
    }
}

pub struct Unwind {
    pub variable: Variable,
    pub expr: Expr,
}

impl Unwind {
    pub fn xmlnode(&self) -> XmlNode<'_> {
        XmlNode::simple_record(
            "UnwindProjection",
            vec![
                ("variable", Pretty::display(&self.variable.name)),
                ("expr", self.expr.pretty().into()),
            ],
            vec![],
        )
    }
}

#[derive(Default)]
pub struct RegularProjection {
    pub items: IndexMap<VariableName, Expr>,
    pub order_by: Vec<SortItem>,
    pub pagination: Pagination,
    pub filter: FilterExprs,
    // final output varaibles
    // TODO(pgao): maybe we should just put an extra_project here
    pub output: IndexSet<VariableName>,
}

impl RegularProjection {
    // true on items contains variable not in output
    pub fn needs_extra_proj(&self) -> bool {
        self.items.iter().any(|(name, _)| !self.output.contains(name))
    }

    pub fn extra_project(&self) -> Option<IndexMap<VariableName, Expr>> {
        if !self.needs_extra_proj() {
            return None;
        }
        let mut extra_proj = IndexMap::new();
        for out_item in self.output.iter() {
            if let Some(expr) = self.items.get(out_item) {
                extra_proj.insert(
                    out_item.clone(),
                    Expr::from_variable(&Variable {
                        name: out_item.clone(),
                        typ: expr.typ(),
                    }),
                );
            } else {
                unreachable!()
            }
        }
        Some(extra_proj)
    }

    pub fn xmlnode(&self) -> XmlNode<'_> {
        let mut fields = vec![];
        fields.push(("items", pretty_project_items(self.items.iter())));
        if self.needs_extra_proj() {
            fields.push((
                "output",
                Pretty::Array(self.output.iter().map(Pretty::display).collect()),
            ));
        }
        add_common_projection_fields(&mut fields, &self.order_by, &self.pagination, &self.filter);

        XmlNode::simple_record("Project", fields, vec![])
    }
}

pub struct AggregateProjection {
    pub group_by: IndexMap<VariableName, Expr>,
    pub aggregate: IndexMap<VariableName, Expr>,
    /// post-aggregation projection (built from return items)
    pub post_projection: IndexMap<VariableName, Expr>,
    pub order_by: Vec<SortItem>,
    pub pagination: Pagination,
    pub filter: FilterExprs,
}

impl AggregateProjection {
    // true on post_projection are not just pass through variables
    pub fn needs_extra_proj(&self) -> bool {
        for (k, v) in self.post_projection.iter() {
            match (k, v.as_variable_ref()) {
                (k, Some(v)) if k == &v.name => continue,
                _ => return true,
            }
        }
        false
    }

    pub fn extra_project(&self) -> Option<IndexMap<VariableName, Expr>> {
        if !self.needs_extra_proj() {
            return None;
        }
        Some(self.post_projection.clone())
    }

    pub fn xmlnode(&self) -> XmlNode<'_> {
        let mut fields = vec![];
        if !self.group_by.is_empty() {
            fields.push(("group_by", pretty_project_items(self.group_by.iter())));
        };
        if !self.aggregate.is_empty() {
            fields.push(("aggregate", pretty_project_items(self.aggregate.iter())));
        };
        if !self.post_projection.is_empty() {
            fields.push(("post_projection", pretty_project_items(self.post_projection.iter())));
        }
        add_common_projection_fields(&mut fields, &self.order_by, &self.pagination, &self.filter);

        XmlNode::simple_record("Aggregate", fields, vec![])
    }
}

pub struct DistinctProjection {
    pub group_by: IndexMap<VariableName, Expr>,
    pub order_by: Vec<SortItem>,
    pub pagination: Pagination,
    pub filter: FilterExprs,
}

impl DistinctProjection {
    pub fn xmlnode(&self) -> XmlNode<'_> {
        let mut fields = vec![];
        if !self.group_by.is_empty() {
            fields.push(("group_by", pretty_project_items(self.group_by.iter())));
        };
        add_common_projection_fields(&mut fields, &self.order_by, &self.pagination, &self.filter);

        XmlNode::simple_record("Distinct", fields, vec![])
    }
}

fn add_common_projection_fields<'a>(
    fields: &mut Vec<(&'a str, Pretty<'a>)>,
    order_by: &'a [SortItem],
    pagination: &'a Pagination,
    filter: &'a FilterExprs,
) {
    if !order_by.is_empty() {
        fields.push(("order_by", pretty_order_items(order_by)));
    }
    if !pagination.is_empty() {
        fields.push(("pagination", Pretty::display(pagination)));
    }
    if !filter.is_true() {
        fields.push(("filter", Pretty::display(&filter.pretty())));
    }
}

// Load <format> FROM <source> OPTIONS {key: value, ...} AS <variable>
pub struct Load {
    pub(crate) variable: VariableName,
    pub(crate) source_url: Arc<str>,
    pub(crate) format: LoadFormat,
}

impl Load {
    pub fn xmlnode(&self) -> XmlNode<'_> {
        XmlNode::simple_record(
            "Load",
            vec![
                ("variable", Pretty::display(&self.variable)),
                ("source_url", Pretty::from(self.source_url.as_ref())),
                ("format", Pretty::Record(self.format.xmlnode())),
            ],
            vec![],
        )
    }
}

#[derive(EnumAsInner, Debug, Clone)]
pub enum LoadFormat {
    Csv(CsvLoadFormat),
    // other kind of formats...
}

impl LoadFormat {
    pub fn output_type(&self) -> DataType {
        match self {
            LoadFormat::Csv(f) => f.output_type(),
        }
    }

    pub fn xmlnode(&self) -> XmlNode<'_> {
        match self {
            LoadFormat::Csv(f) => f.xmlnode(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CsvLoadFormat {
    pub(crate) header: bool,
    pub(crate) delimiter: char,
}

impl CsvLoadFormat {
    pub fn new(header: bool, delimiter: char) -> Self {
        Self { header, delimiter }
    }

    pub fn header(&self) -> bool {
        self.header
    }

    pub fn delimiter(&self) -> char {
        self.delimiter
    }

    pub fn output_type(&self) -> DataType {
        if self.header {
            // we do not know the struct fields, so put an any here.
            DataType::Any
        } else {
            // we assume the list fields are string
            DataType::new_list(DataType::String)
        }
    }

    pub fn xmlnode(&self) -> XmlNode<'_> {
        XmlNode::simple_record(
            "CsvLoadFormat",
            vec![
                ("header", Pretty::display(&self.header)),
                ("delimiter", Pretty::display(&self.delimiter)),
            ],
            vec![],
        )
    }
}
