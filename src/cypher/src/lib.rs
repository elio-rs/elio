#![feature(error_generic_member_access)]
#![allow(clippy::double_parens)] // this is because EnumAsInner will generate extra parens

pub mod binder;
pub mod error;
pub mod expr;
pub mod ir;
pub mod optimizer;
pub mod plan_context;
pub mod plan_node;
pub mod planner;
pub mod props;
pub mod session;
pub mod variable;

pub(crate) mod pretty_utils;
