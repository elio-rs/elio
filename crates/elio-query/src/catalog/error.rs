use elio_storage::error::GraphStoreError;
use thiserror;

#[derive(thiserror::Error, Debug)]
pub enum CatalogError {
    #[error("storage error {0}")]
    StorageErr(#[from] GraphStoreError),
}
