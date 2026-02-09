use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use elio_common::scalar::Row;
use elio_core::db_env::{DbConfig, DbEnv};
use elio_core::error::Error as GraphDBError;
use elio_core::session::Session;
use futures::stream::StreamExt;
use sqllogictest::{AsyncDB, ColumnType, DBOutput};
use tempfile::TempDir;

pub struct EmbeddedGraphDB {
    _db: Arc<DbEnv>,
    sess: Arc<Session>,
    // hold db files, references the temp file, in case of temp file is deleted during test
    _temp_dir: TempDir,
}

impl EmbeddedGraphDB {
    pub fn open(temp_dir: TempDir) -> Result<Self, GraphDBError> {
        let config = DbConfig::with_db_path(temp_dir.path());

        let db = DbEnv::open(&config)?;
        let sess = db.new_session();
        Ok(Self {
            _db: db,
            sess,
            _temp_dir: temp_dir,
        })
    }
}

fn convert_row(row: Row) -> Vec<String> {
    row.into_iter()
        .map(|v| v.map_or_else(|| "null".to_string(), |v| v.to_string()))
        .collect()
}

pub fn graphdb_column_validator<T: ColumnType>(a: &Vec<T>, b: &Vec<T>) -> bool {
    a == b
}

/// The valid types are:
/// - 'A' - Any value type, in text format
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum GraphDBColumnType {
    // basic
    Any,
}

impl ColumnType for GraphDBColumnType {
    fn from_char(value: char) -> Option<Self> {
        match value {
            'A' => Some(GraphDBColumnType::Any),
            _ => None,
        }
    }

    fn to_char(&self) -> char {
        match self {
            GraphDBColumnType::Any => 'A',
        }
    }
}

#[async_trait]
impl AsyncDB for EmbeddedGraphDB {
    /// The type of result columns
    type ColumnType = GraphDBColumnType;
    /// The error type of SQL execution.
    type Error = GraphDBError;

    /// Async run a SQL query and return the output.
    async fn run(&mut self, sql: &str) -> Result<DBOutput<Self::ColumnType>, Self::Error> {
        let mut stream = self.sess.execute(sql.to_string(), Default::default()).await?;
        let mut rows = Vec::new();
        while let Some(row) = stream.next().await {
            let row = row?;
            rows.push(convert_row(row));
        }
        let types = (0..stream.columns().len())
            .map(|_| GraphDBColumnType::Any)
            .collect::<Vec<_>>();
        Ok(DBOutput::Rows { types, rows })
    }

    /// Shutdown the connection gracefully.
    async fn shutdown(&mut self) {
        // cleanup handled by drop
    }

    /// Engine name of current database.
    fn engine_name(&self) -> &str {
        "graphdb"
    }

    /// [`Runner`] calls this function to perform sleep.
    ///
    /// The default implementation is `std::thread::sleep`, which is universal to any async runtime
    /// but would block the current thread. If you are running in tokio runtime, you should override
    /// this by `tokio::time::sleep`.
    async fn sleep(dur: Duration) {
        tokio::time::sleep(dur).await;
    }

    /// [`Runner`] calls this function to run a system command.
    ///
    /// The default implementation is `std::process::Command::output`, which is universal to any
    /// async runtime but would block the current thread. If you are running in tokio runtime, you
    /// should override this by `tokio::process::Command::output`.
    async fn run_command(command: Command) -> std::io::Result<std::process::Output> {
        let mut cmd = tokio::process::Command::from(command);
        cmd.output().await
    }
}
