use std::sync::Arc;

use elio_common::data_type::DataType;
use elio_common::schema::{Schema, Variable};
use elio_common::variable::VariableName;
use enum_as_inner::EnumAsInner;
use itertools::Itertools;
use pretty_xmlish::{Pretty, XmlNode};

use crate::expr::{Expr, ExprNode};
use crate::plan_context::PlanContext;
use crate::plan_node::plan_base::{PlanBase, PlanNodeId};

pub mod aggregate;
pub mod all_node_scan;
pub mod apply;
pub mod argument;
pub mod black_hole;
pub mod create_node;
pub mod create_rel;
pub mod cross_product;
pub mod distinct;
pub mod empty;
pub mod expand;
pub mod filter;
pub mod get_prop;
pub mod load;
pub mod node_index_seek;
pub mod pagination;
pub mod plan_base;
pub mod produce_result;
pub mod project;
pub mod sort;
pub mod unit;
pub mod var_expand;
pub use aggregate::*;
pub use all_node_scan::*;
pub use apply::*;
pub use argument::*;
pub use black_hole::*;
pub use create_node::*;
pub use create_rel::*;
pub use cross_product::*;
pub use distinct::*;
pub use empty::*;
pub use expand::*;
pub use filter::*;
pub use get_prop::*;
pub use load::*;
pub use node_index_seek::*;
pub use pagination::*;
pub use produce_result::*;
pub use project::*;
pub use sort::*;
pub use unit::*;
pub use var_expand::*;

#[derive(Default, Debug, Clone, Copy, derive_more::Display)]
pub enum PathMode {
    Walk, // repeated node, repeated rel
    #[default]
    Trail, // repeated node, non-repeated rel
}

#[derive(Clone, Debug, EnumAsInner)]
pub enum PlanExpr {
    // graph
    AllNodeScan(AllNodeScan),
    NodeIndexSeek(NodeIndexSeek),
    GetProperty(GetProperty),
    Expand(Expand),
    VarExpand(VarExpand),
    Apply(Apply),
    Argument(Argument),
    Unit(Unit),
    ProduceResult(ProduceResult),
    // graph-modify
    CreateNode(CreateNode),
    CreateRel(CreateRel),
    // relational
    Load(Load),
    CrossProduct(CrossProduct),
    Aggregate(Aggregate),
    Distinct(Distinct),
    Project(Project),
    Sort(Sort),
    Filter(Filter),
    Pagination(Pagination),
    Empty(Empty),
    BlackHole(BlackHole),
}

impl PlanExpr {
    pub fn empty(schema: Arc<Schema>, ctx: Arc<PlanContext>) -> Self {
        Self::Empty(Empty::new(schema, ctx))
    }

    pub fn boxed(self) -> Box<Self> {
        Box::new(self)
    }
}

pub trait PlanNode {
    type Inner: InnerNode;

    fn inner(&self) -> &Self::Inner;

    fn into_inner(self) -> Self::Inner;

    fn inputs(&self) -> Vec<&PlanExpr> {
        self.inner().inputs()
    }

    fn xmlnode(&self) -> XmlNode<'_>;
}

pub trait InnerNode {
    fn build_base(&self) -> PlanBase;
    fn inputs(&self) -> Vec<&PlanExpr>;
}

macro_rules! impl_plan_node_common {
    ($plan_node:ident, $inner:ident) => {
        impl $plan_node {
            pub fn id(&self) -> PlanNodeId {
                self.base.id()
            }

            pub fn schema(&self) -> Arc<Schema> {
                self.base.schema().clone()
            }

            pub fn ctx(&self) -> Arc<PlanContext> {
                self.base.ctx()
            }
        }

        // impl To for plan_node
        impl From<$plan_node> for PlanExpr {
            fn from(value: $plan_node) -> Self {
                Self::$plan_node(value)
            }
        }

        impl From<$plan_node> for Box<PlanExpr> {
            fn from(value: $plan_node) -> Self {
                Box::new(PlanExpr::$plan_node(value))
            }
        }
    };
}

impl_plan_node_common!(AllNodeScan, AllNodeScanInner);
impl_plan_node_common!(NodeIndexSeek, NodeIndexSeekInner);
impl_plan_node_common!(GetProperty, GetPropertyInner);
impl_plan_node_common!(Expand, ExpandInner);
impl_plan_node_common!(VarExpand, VarExpandInner);
impl_plan_node_common!(Apply, ApplyInner);
impl_plan_node_common!(Argument, ArgumentInner);
impl_plan_node_common!(Unit, UnitInner);
impl_plan_node_common!(CreateNode, CreateNodeInner);
impl_plan_node_common!(CreateRel, CreateRelInner);
impl_plan_node_common!(Load, LoadInner);
impl_plan_node_common!(Project, ProjectInner);
impl_plan_node_common!(Aggregate, AggregateInner);
impl_plan_node_common!(Distinct, DistinctInner);
impl_plan_node_common!(Sort, SortInner);
impl_plan_node_common!(Filter, FilterInner);
impl_plan_node_common!(Pagination, PaginationInner);
impl_plan_node_common!(Empty, EmptyInner);
impl_plan_node_common!(BlackHole, BlackHoleInner);
impl_plan_node_common!(ProduceResult, ProduceResultInner);
impl_plan_node_common!(CrossProduct, CrossProductInner);

macro_rules! impl_plan_expr_dispatch {
    ($($plan_node:ident),*) => {
        impl PlanExpr {
            pub fn id(&self) -> PlanNodeId {
                match self {
                    $(PlanExpr::$plan_node(p) => p.id(),)*
                }
            }

            pub fn schema(&self) -> Arc<Schema> {
                match self {
                    $(PlanExpr::$plan_node(p) => p.schema(),)*
                }
            }

            pub fn ctx(&self) -> Arc<PlanContext>{
                match self {
                    $(PlanExpr::$plan_node(p) => p.ctx(),)*
                }
            }

            pub fn inputs(&self) -> Vec<&PlanExpr> {
                match self {
                    $(PlanExpr::$plan_node(p) => p.inputs(),)*
                }
            }

            pub fn xmlnode(&self) -> XmlNode<'_>{
                match self {
                    $(PlanExpr::$plan_node(p) => p.xmlnode(),)*
                }
            }
        }
    };
}

impl_plan_expr_dispatch!(
    AllNodeScan,
    NodeIndexSeek,
    GetProperty,
    Expand,
    VarExpand,
    Apply,
    Argument,
    Unit,
    CreateNode,
    CreateRel,
    Load,
    CrossProduct,
    Aggregate,
    Distinct,
    Project,
    Sort,
    Filter,
    Pagination,
    Empty,
    BlackHole,
    ProduceResult
);

#[macro_export]
macro_rules! impl_plan_inner {
    () => {
        fn inner(&self) -> &Self::Inner {
            &self.inner
        }

        fn into_inner(self) -> Self::Inner {
            self.inner
        }
    };
}

impl PlanExpr {
    /// Apply a mapper to all direct child plan nodes and rebuild this node.
    pub fn map_children<F>(self, mut f: F) -> PlanExpr
    where
        F: FnMut(PlanExpr) -> PlanExpr,
    {
        self.map_children_result::<_, ()>(|p| Ok(f(p))).expect("infallible")
    }

    /// Like `map_children` but allows the mapper to return a `Result`.
    pub fn map_children_result<F, E>(self, mut f: F) -> Result<PlanExpr, E>
    where
        F: FnMut(PlanExpr) -> Result<PlanExpr, E>,
    {
        match self {
            PlanExpr::AllNodeScan(plan) => {
                let AllNodeScanInner {
                    variable,
                    arguments,
                    ctx,
                } = plan.into_inner();
                let arguments = match arguments {
                    Some(arg) => Some(Box::new(f(*arg)?)),
                    None => None,
                };
                Ok(AllNodeScan::new(AllNodeScanInner::new(variable, arguments, ctx)).into())
            }
            PlanExpr::NodeIndexSeek(plan) => {
                let NodeIndexSeekInner {
                    variable,
                    label,
                    constraint_name,
                    prop_tokens,
                    prop_values,
                    ctx,
                    arguments,
                } = plan.into_inner();
                let arguments = match arguments {
                    Some(arg) => Some(Box::new(f(*arg)?)),
                    None => None,
                };
                Ok(NodeIndexSeek::new(NodeIndexSeekInner::new(
                    variable,
                    label,
                    constraint_name,
                    prop_tokens,
                    prop_values,
                    ctx,
                    arguments,
                ))
                .into())
            }
            PlanExpr::GetProperty(plan) => {
                let GetPropertyInner { input, entities } = plan.into_inner();
                let input = Box::new(f(*input)?);
                Ok(GetProperty::new(GetPropertyInner { input, entities }).into())
            }
            PlanExpr::Expand(plan) => {
                let ExpandInner {
                    input,
                    from,
                    to,
                    rel,
                    direction,
                    types,
                    kind,
                } = plan.into_inner();
                let input = Box::new(f(*input)?);
                Ok(Expand::new(ExpandInner {
                    input,
                    from,
                    to,
                    rel,
                    direction,
                    types,
                    kind,
                })
                .into())
            }
            PlanExpr::VarExpand(plan) => {
                let VarExpandInner {
                    input,
                    from,
                    to,
                    rel_pattern,
                    node_filter,
                    rel_filter,
                    kind,
                    path_mode,
                } = plan.into_inner();
                let input = Box::new(f(*input)?);
                Ok(VarExpand::new(VarExpandInner {
                    input,
                    from,
                    to,
                    rel_pattern,
                    node_filter,
                    rel_filter,
                    kind,
                    path_mode,
                })
                .into())
            }
            PlanExpr::Apply(plan) => {
                let ApplyInner { left, right } = plan.into_inner();
                let left = Box::new(f(*left)?);
                let right = Box::new(f(*right)?);
                Ok(Apply::new(ApplyInner { left, right }).into())
            }
            PlanExpr::Argument(plan) => Ok(PlanExpr::Argument(plan)),
            PlanExpr::Unit(plan) => Ok(PlanExpr::Unit(plan)),
            PlanExpr::ProduceResult(plan) => {
                let ProduceResultInner { input, return_columns } = plan.into_inner();
                let input = Box::new(f(*input)?);
                Ok(ProduceResult::new(ProduceResultInner { input, return_columns }).into())
            }
            PlanExpr::CreateNode(plan) => {
                let CreateNodeInner { input, nodes } = plan.into_inner();
                let input = Box::new(f(*input)?);
                Ok(CreateNode::new(CreateNodeInner { input, nodes }).into())
            }
            PlanExpr::CreateRel(plan) => {
                let CreateRelInner { input, rels } = plan.into_inner();
                let input = Box::new(f(*input)?);
                Ok(CreateRel::new(CreateRelInner { input, rels }).into())
            }
            PlanExpr::Load(plan) => Ok(PlanExpr::Load(plan)),
            PlanExpr::CrossProduct(plan) => {
                let CrossProductInner { left, right } = plan.into_inner();
                let left = Box::new(f(*left)?);
                let right = Box::new(f(*right)?);
                Ok(PlanExpr::CrossProduct(CrossProduct::new(CrossProductInner {
                    left,
                    right,
                })))
            }
            PlanExpr::Aggregate(plan) => {
                let AggregateInner {
                    input,
                    group_by,
                    aggregate,
                } = plan.into_inner();
                let input = Box::new(f(*input)?);
                Ok(Aggregate::new(AggregateInner {
                    input,
                    group_by,
                    aggregate,
                })
                .into())
            }
            PlanExpr::Distinct(plan) => {
                let DistinctInner { input, group_exprs } = plan.into_inner();
                let input = Box::new(f(*input)?);
                Ok(Distinct::new(DistinctInner { input, group_exprs }).into())
            }
            PlanExpr::Project(plan) => {
                let ProjectInner { input, projections } = plan.into_inner();
                let input = Box::new(f(*input)?);
                Ok(Project::new(ProjectInner { input, projections }).into())
            }
            PlanExpr::Sort(plan) => {
                let SortInner { input, items } = plan.into_inner();
                let input = Box::new(f(*input)?);
                Ok(Sort::new(SortInner { input, items }).into())
            }
            PlanExpr::Filter(plan) => {
                let FilterInner { input, condition } = plan.into_inner();
                let input = Box::new(f(*input)?);
                Ok(Filter::new(FilterInner { input, condition }).into())
            }
            PlanExpr::Pagination(plan) => {
                let PaginationInner { input, offset, limit } = plan.into_inner();
                let input = Box::new(f(*input)?);
                Ok(Pagination::new(PaginationInner { input, offset, limit }).into())
            }
            PlanExpr::Empty(plan) => Ok(PlanExpr::Empty(plan)),
            PlanExpr::BlackHole(plan) => {
                let BlackHoleInner { input } = plan.into_inner();
                let input = Box::new(f(*input)?);
                Ok(BlackHole::new(BlackHoleInner { input }).into())
            }
        }
    }
}
