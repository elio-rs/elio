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

pub mod all_node_scan;
pub mod apply;
pub mod argument;
pub mod black_hole;
pub mod create_node;
pub mod create_rel;
pub mod cross_product;
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
pub use all_node_scan::*;
pub use apply::*;
pub use argument::*;
pub use black_hole::*;
pub use create_node::*;
pub use create_rel::*;
pub use cross_product::*;
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
    Project,
    Sort,
    Filter,
    Pagination,
    Empty,
    BlackHole,
    ProduceResult
);
