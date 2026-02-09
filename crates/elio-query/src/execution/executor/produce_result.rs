use std::sync::Arc;

use async_stream::try_stream;
use bitvec::vec::BitVec;
use elio_common::array::{Array, ListArray, PathArray};
use futures::StreamExt;

use super::*;

// materialize virtual node to node
// change output column order
#[derive(Debug)]
pub struct ProduceResultExecutor {
    pub input: SharedExecutor,
    // return_columns[i] = j means
    // the i-th column of the output is the j-th column of the input
    pub return_columns: Vec<usize>,
    pub schema: Arc<Schema>,
}

impl Executor for ProduceResultExecutor {
    fn open(&self, ctx: Arc<TaskExecContext>) -> Result<DataChunkStream, ExecError> {
        let return_columns = self.return_columns.clone();

        let input_stream = self.input.open(ctx.clone())?;

        let stream = try_stream! {
            for await input in input_stream {
                let input = input?;
                let input = input.compact();
                let vis = input.visibility();

                let mut out_cols = vec![];
                for col_idx in return_columns.iter(){
                    let col = input.column(*col_idx);
                    // if col is type of virtual node, materialize it
                    if let Some(virtual_col) = col.as_virtual_node(){
                        let node_col= ctx.tx().materialize_node(virtual_col, vis)?;
                        out_cols.push(Arc::new(node_col.into()));
                    } else if let Some(vpath) = col.as_virtual_path() {
                        let (path_vnodes, path_rels, valid) = vpath.clone().into_parts();
                        // matierlize virtual path nodes
                        let path_nodes = {
                            let (offsets, child, valid) = path_vnodes.as_ref().clone().into_parts();
                            let child_nodes = child.as_virtual_node().expect("virtual path nodes should be virtual node");
                            let vis = BitVec::repeat(true, child_nodes.len());
                            let mz_nodes = ctx.tx().materialize_node(child_nodes, &vis)?;
                            Arc::new(ListArray::from_parts(offsets, Arc::new(mz_nodes.into()), valid))
                        };

                        let mz_path = PathArray::from_parts(path_nodes, path_rels, valid);
                        out_cols.push(Arc::new(mz_path.into()));
                    } else {
                        out_cols.push(col.clone());
                    }
               }

               let output = DataChunk::new(out_cols, vis.clone());
               yield output;
            }
        }
        .boxed();
        Ok(stream)
    }

    fn schema(&self) -> &Schema {
        &self.schema
    }

    fn name(&self) -> &'static str {
        "ProduceResult"
    }
}
