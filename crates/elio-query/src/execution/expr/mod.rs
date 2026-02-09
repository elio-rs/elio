use std::sync::Arc;

use bitvec::vec::BitVec;
use elio_common::array::chunk::DataChunk;
use elio_common::array::{ArrayRef, NodeArray, VirtualNodeArray};
use elio_common::data_type::DataType;
use elio_common::{TokenId, TokenKind};

use crate::execution::error::EvalError;

pub mod agg_call;
pub mod constant;
pub mod create_list;
pub mod create_struct;
pub mod field_access;
pub mod label;
pub mod project_path;
pub mod scalar_call;
pub mod variable_ref;

pub use agg_call::*;
pub use constant::*;
pub use create_list::*;
pub use create_struct::*;
pub use field_access::*;
pub use label::*;
pub use project_path::*;
pub use scalar_call::*;
pub use variable_ref::*;

pub trait EvalCtx {
    // schema
    fn get_or_create_token(&self, token: &str, kind: TokenKind) -> Result<TokenId, EvalError>;
    // graph storage
    // access the storage engine and materialize node
    fn materialize_node(&self, chunk: &VirtualNodeArray, vis: &BitVec) -> Result<NodeArray, EvalError>;
}

// an evaluatable expression
pub trait Expression: Send + Sync + 'static + std::fmt::Debug {
    fn typ(&self) -> &DataType;
    fn eval_batch(&self, chunk: &DataChunk, ctx: &dyn EvalCtx) -> Result<ArrayRef, EvalError>;
    fn into_shared(self) -> SharedExpression
    where
        Self: Sized,
    {
        Arc::new(self)
    }
}

pub type SharedExpression = Arc<dyn Expression>;
