//! DDL (Data Definition Language) operations
//!
//! This module handles schema-related operations like:
//! - CREATE CONSTRAINT
//! - DROP CONSTRAINT
//! - (future) CREATE INDEX, DROP INDEX, etc.

mod constraint;

pub use constraint::{create_constraint, drop_constraint};
