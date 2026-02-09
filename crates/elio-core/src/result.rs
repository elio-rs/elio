use elio_common::scalar::Row;
use futures::Stream;

use crate::error::Error;

pub enum QueryKind {
    // read only query
    Read,
    // update or create data, also produce results
    ReadWrite,
    // write only query, update or create data, without produce any results
    Write,
    // schema change query, update schema but does not change data nor produces any results
    SchemaWrite,
    // dbms query
    Dbms,
}

pub enum QueryExecutionKind {
    Query(QueryKind),
    Profile,
    Explain,
}

/// Query Result Handle.
/// Result handle contains three parts:
/// 1. meta data: query static info(static, resolved after planning):
///     - query execution kind
///     - columns
/// 2. meta data: query execution info(dynamic, resolved after execution):
///     - query statistics
///     - execution plan
///     - gql status object
///     - notifications
/// 3. data: query result data(if any)
/// 4. listeners
///     - on query failed
///     - on query finished
///     - ...
///
/// ResultHandle communicate with execution engine with QueryExecutionHandle object
pub trait ResultHandle: Stream<Item = Result<Row, Error>> + Send {
    fn columns(&self) -> &[String];
}
