use std::backtrace::Backtrace;

use thiserror::Error;

use crate::execution::builder::BuildError;
use crate::execution::error::ExecError;
use crate::plan::error::PlanError;

#[derive(Error, Debug)]
pub enum Error {
    #[error("open db failed")]
    OpenDbFailed {
        #[from]
        source: elio_storage::error::GraphStoreError,
    },

    #[error("{0}")]
    PlanError(#[from] PlanError, #[backtrace] Backtrace),
    #[error("{0}")]
    ExecError(#[from] ExecError, #[backtrace] Backtrace),
    #[error("{0}")]
    BuildError(#[from] BuildError, #[backtrace] Backtrace),

    // DDL errors
    #[error("constraint '{0}' already exists")]
    ConstraintAlreadyExists(String),

    #[error("constraint '{0}' not found")]
    ConstraintNotFound(String),

    // Internal error (e.g., panic captured)
    #[error("paniced: {0}")]
    Panic(String),
}
