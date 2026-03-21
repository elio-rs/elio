use std::fmt::Display;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum GraphStoreError {
    #[error("heed error: {0}")]
    Heed(#[from] heed::Error),
    #[error("internal error: {0}")]
    Internal(String),
    #[error("type mismatch: {0}")]
    TypeMismatch(String),
    #[error("token not found: {0}")]
    Token(String),
}

impl GraphStoreError {
    pub fn internal<T: Display>(msg: T) -> Self {
        Self::Internal(msg.to_string())
    }

    pub fn type_mismatch<T: Display>(msg: T) -> Self {
        Self::TypeMismatch(msg.to_string())
    }

    /// Returns true if this error is an LMDB MDB_MAP_FULL error.
    pub fn is_map_full(&self) -> bool {
        matches!(self, Self::Heed(heed::Error::Mdb(heed::MdbError::MapFull)))
    }
}
