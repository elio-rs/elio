#![feature(error_generic_member_access)]
#![allow(clippy::double_parens)] // this is because EnumAsInner will generate extra parens

pub mod binder;
pub mod catalog;
pub mod database;
pub mod execution;
pub mod function;
pub mod optimizer;
pub mod plan;
pub mod planner;
