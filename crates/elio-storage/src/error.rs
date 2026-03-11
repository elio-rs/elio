use std::fmt::Display;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum GraphStoreError {
    #[error("storage error: {0}")]
    Storage(String),
    #[error("internal error: {0}")]
    Internal(String),
    #[error("type mismatch: {0}")]
    TypeMismatch(String),
    #[error("token not found: {0}")]
    Token(String),
}

impl GraphStoreError {
    pub fn storage<T: Display>(msg: T) -> Self {
        Self::Storage(msg.to_string())
    }

    pub fn internal<T: Display>(msg: T) -> Self {
        Self::Internal(msg.to_string())
    }

    pub fn type_mismatch<T: Display>(msg: T) -> Self {
        Self::TypeMismatch(msg.to_string())
    }
}
