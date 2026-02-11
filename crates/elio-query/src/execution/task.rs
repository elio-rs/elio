use std::sync::Arc;

use bitvec::vec::BitVec;
use elio_common::array::chunk::DataChunk;
use elio_common::array::{NodeArray, VirtualNodeArray};
use elio_common::schema::Schema;
use elio_common::{TokenId, TokenKind};
use elio_storage::graph::GraphStore;
use elio_storage::token::TokenStore;
use elio_storage::transaction::Transaction;
use futures::StreamExt;
use itertools::Itertools;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::execution::QueryContext;
use crate::execution::builder::{ExecutorBuildContext, build_executor};
use crate::execution::error::{EvalError, ExecError};
use crate::execution::executor::SharedExecutor;
use crate::execution::expr::EvalCtx;
use crate::execution::panic::spawn_with_hook;
use crate::plan::plan_node::PlanExpr;
use crate::planner::RootPlan;

pub struct EvalCtxImpl {
    pub graph_store: Arc<GraphStore>,
    pub tx: Arc<Transaction>,
    pub token_store: Arc<TokenStore>,
}

impl EvalCtx for EvalCtxImpl {
    fn get_or_create_token(&self, token: &str, kind: TokenKind) -> Result<TokenId, EvalError> {
        self.token_store
            .get_or_create_token(token, kind)
            .map_err(|e| EvalError::GetOrCreateTokenError(e.to_string()))
    }

    fn materialize_node(&self, node_ids: &VirtualNodeArray, vis: &BitVec) -> Result<NodeArray, EvalError> {
        self.graph_store
            .materialize_node(&self.tx, node_ids, vis)
            .map_err(|e| EvalError::materialize_node_error(e.to_string()))
    }
}

/// receiver side of task
/// TODO(pgao): separate the task result fetcher and task control logic like abort etc
/// task manager is able to abort tasks
pub struct TaskHandle {
    pub query_id: Arc<str>,
    pub schema: Schema,
    pub columns: Vec<String>,
    pub recv: UnboundedReceiver<Result<DataChunk, ExecError>>,
}

impl TaskHandle {
    pub async fn cancel(&self) {
        todo!()
    }

    // fetch next data chunk result
    pub async fn next(&mut self) -> Result<Option<DataChunk>, ExecError> {
        self.recv.recv().await.transpose()
    }
}

/// create task and spawn running task execution
pub async fn create_task(qctx: Arc<QueryContext>, query_id: Arc<str>, plan: RootPlan) -> Result<TaskHandle, ExecError> {
    // compile to executor
    let mut bctx = ExecutorBuildContext::new(&qctx);
    let root_executor = build_executor(&mut bctx, &plan)?;

    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

    let columns = plan.names.keys().cloned().collect_vec();

    let handle = TaskHandle {
        query_id,
        recv: rx,
        schema: root_executor.schema().clone(),
        columns,
    };

    let runner = TaskRunner {
        ctx: qctx,
        tx,
        root_executor,
    };

    runner.start();

    Ok(handle)
}

fn plan_is_write(plan: &PlanExpr) -> bool {
    match plan {
        // NB: create constraint is handled separately in the ddl module
        // TODO(pgao): we should put the create constraint into the task execution
        PlanExpr::CreateNode(_) | PlanExpr::CreateRel(_) => true,
        _ => plan.inputs().into_iter().any(plan_is_write),
    }
}

pub struct TaskRunner {
    ctx: Arc<QueryContext>,
    tx: UnboundedSender<Result<DataChunk, ExecError>>,
    root_executor: SharedExecutor,
    // TODO(pgao): cancellation token
}

impl TaskRunner {
    pub fn start(self) {
        // spawn task and drive task to finish
        let TaskRunner { ctx, tx, root_executor } = self;
        let txn = ctx.tx.clone();
        let stream = match root_executor.open(ctx) {
            Ok(s) => s,
            Err(e) => {
                let _ = tx.send(Err(e));
                return;
            }
        };
        let mut stream = stream.boxed();

        let tx_for_panic = tx.clone();
        let panic_hook = move |msg: String| {
            let _ = tx_for_panic.send(Err(ExecError::panic(msg)));
        };

        let fut = async move {
            let mut success = true;
            // TODO(pgao): cancellation token
            while let Some(chunk) = stream.next().await {
                let is_err = chunk.is_err();
                if tx.send(chunk).is_err() {
                    success = false;
                    break;
                }
                if is_err {
                    success = false;
                    break;
                }
            }

            if success {
                if let Err(e) = txn.commit() {
                    let _ = tx.send(Err(e.into()));
                }
            } else {
                let _ = txn.abort();
            }
        };

        spawn_with_hook("task_runner".to_string(), fut, panic_hook);
    }
}
