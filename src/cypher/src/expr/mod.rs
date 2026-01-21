use elio_common::IrToken;
use elio_common::data_type::DataType;
use elio_common::schema::Variable;
use enum_as_inner::EnumAsInner;

pub mod agg_call;
pub mod create_list;
pub mod create_map;
pub mod filters;
/// Logical expr
pub mod func_call;
pub mod label;
pub mod project_path;
pub mod property_access;
pub mod subquery;
pub mod utils;
pub mod value;
pub mod variable_ref;
pub use agg_call::*;
pub use create_list::*;
pub use create_map::*;
pub use filters::*;
pub use func_call::*;
pub use label::*;
pub use project_path::*;
pub use property_access::*;
pub use subquery::*;
pub use value::*;
pub use variable_ref::*;

// TODO(pgao): do we need Hash here?
#[derive(Debug, Hash, Clone, Eq, PartialEq, EnumAsInner)]
pub enum Expr {
    VariableRef(VariableRef),
    PropertyAccess(PropertyAccess),
    Constant(Constant),
    FuncCall(FuncCall),
    AggCall(AggCall),
    Subquery(Subquery),
    HasLabel(HasLabel),
    CreateStruct(CreateStruct),
    CreateList(CreateList),
    // graph
    ProjectPath(ProjectPath),
}

pub type BoxedExpr = Box<Expr>;

pub trait ExprNode: std::fmt::Debug + Clone {
    fn typ(&self) -> DataType;
}

macro_rules! impl_expr_node_for_enum {
    ($enum_name:ident, $($variant:ident),+) => {
        impl ExprNode for $enum_name {
            fn typ(&self) -> DataType {
                match self {
                    $(
                        Self::$variant(expr) => expr.typ(),
                    )+
                }
            }
        }

        /// convert from variant to BoxedExpr
        $(
            impl From<$variant> for BoxedExpr {
                fn from(expr: $variant) -> Self {
                    Box::new(Expr::$variant(expr))
                }
            }
        )+
    };
}

impl_expr_node_for_enum!(
    Expr,
    VariableRef,
    PropertyAccess,
    Constant,
    FuncCall,
    AggCall,
    Subquery,
    HasLabel,
    CreateStruct,
    CreateList,
    ProjectPath
);

impl Expr {
    #[inline]
    pub fn boxed(self) -> Box<Expr> {
        Box::new(self)
    }

    #[inline]
    pub fn from_variable(var: &Variable) -> Self {
        Expr::VariableRef(VariableRef::new_unchecked(var.name.clone(), var.typ.clone()))
    }

    #[inline]
    pub fn new_has_label(entity: Variable, label_or_rel: IrToken) -> Self {
        Expr::HasLabel(HasLabel {
            entity: Self::from_variable(&entity).boxed(),
            label_or_rel,
        })
    }
}

impl Expr {
    pub fn boolean(b: bool) -> Self {
        Expr::Constant(Constant::boolean(b))
    }

    // SAFETY:
    //   caller should guarantee inputs are bool
    pub fn and(self, rhs: Self) -> Self {
        Expr::FuncCall(FuncCall::and_unchecked(vec![self, rhs]))
    }

    pub fn or(self, rhs: Self) -> Self {
        Expr::FuncCall(FuncCall::or_unchecked(vec![self, rhs]))
    }

    pub fn equal(self, rhs: Self) -> Self {
        Expr::FuncCall(FuncCall::equal_unchecked(vec![self, rhs]))
    }

    pub fn property(self, prop: &IrToken, typ: &DataType) -> Self {
        Expr::PropertyAccess(PropertyAccess::new_unchecked(self.boxed(), prop, typ))
    }

    pub fn as_null_constant_mut(&mut self) -> Option<&mut Constant> {
        if let Expr::Constant(constant) = self {
            Some(constant)
        } else {
            None
        }
    }
}

impl Expr {
    pub fn pretty(&self) -> String {
        match self {
            Expr::VariableRef(variable_ref) => variable_ref.name.to_string(),
            Expr::PropertyAccess(property_access) => {
                format!("{}.{}", property_access.expr.pretty(), property_access.property)
            }
            Expr::Constant(constant) => constant.pretty(),
            Expr::FuncCall(func_call) => {
                format!(
                    "{}({})",
                    func_call.func,
                    func_call.args.iter().map(|a| a.pretty()).collect::<Vec<_>>().join(", ")
                )
            }
            Expr::AggCall(_agg_call) => todo!(),
            Expr::Subquery(_subquery) => todo!(),
            Expr::HasLabel(has_label) => {
                format!("{}:{}", has_label.entity.pretty(), has_label.label_or_rel)
            }
            Expr::CreateStruct(create_map) => {
                format!(
                    "create_map{{{}}}",
                    create_map
                        .properties
                        .iter()
                        .map(|(k, v)| format!("{}: {}", k, v.pretty()))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
            Expr::CreateList(create_list) => create_list.pretty(),
            Expr::ProjectPath(project_path) => project_path.pretty(),
        }
    }
}
