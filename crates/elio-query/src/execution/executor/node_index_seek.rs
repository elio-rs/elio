//! NodeIndexSeek executor - uses unique index for O(1) node lookup

use std::sync::Arc;

use async_stream::try_stream;
use bitvec::vec::BitVec;
use elio_common::ResolvedIrToken;
use elio_common::array::chunk::{DataChunk, DataChunkBuilder};
use elio_common::array::{ArrayImpl, VirtualNodeArrayBuilder};
use elio_common::mapb::IndexKeyCodec;
use elio_common::schema::Schema;
use futures::StreamExt;

use super::*;
use crate::execution::error::ExecError;
use crate::execution::executor::Executor;
use crate::execution::executor::apply::OutputColumnSource;
use crate::execution::expr::SharedExpression;

/// Executor that performs index lookup to find nodes
#[derive(Debug)]
pub struct NodeIndexSeekExecutor {
    pub schema: Arc<Schema>,
    pub label: ResolvedIrToken,
    pub prop_tokens: Vec<ResolvedIrToken>,
    pub prop_values: Vec<SharedExpression>,
    pub input: Option<SharedExecutor>,
    pub output_mapping: Option<Vec<OutputColumnSource>>,
}

impl NodeIndexSeekExecutor {
    pub fn new(
        schema: Arc<Schema>,
        label: ResolvedIrToken,
        prop_tokens: Vec<ResolvedIrToken>,
        prop_values: Vec<SharedExpression>,
    ) -> Self {
        Self {
            schema,
            label,
            prop_tokens,
            prop_values,
            input: None,
            output_mapping: None,
        }
    }

    pub fn with_input(
        schema: Arc<Schema>,
        label: ResolvedIrToken,
        prop_tokens: Vec<ResolvedIrToken>,
        prop_values: Vec<SharedExpression>,
        input: SharedExecutor,
        output_mapping: Vec<OutputColumnSource>,
    ) -> Self {
        Self {
            schema,
            label,
            prop_tokens,
            prop_values,
            input: Some(input),
            output_mapping: Some(output_mapping),
        }
    }
}

impl Executor for NodeIndexSeekExecutor {
    fn open(&self, ctx: Arc<QueryContext>) -> Result<DataChunkStream, ExecError> {
        let schema = self.schema.clone();
        let label = self.label.id;
        let prop_tokens: Vec<_> = self.prop_tokens.iter().map(|x| x.id).collect();
        let prop_value_exprs = self.prop_values.clone();
        let input = self.input.clone();
        let output_mapping = self.output_mapping.clone();

        let stream = try_stream! {
            let tx = ctx.txn().clone();
            let graph_store = ctx.graph_store().clone();
            let eval_ctx = ctx.derive_eval_ctx();

            if let Some(input) = input {
                // with input: for each input row, evaluate expressions and seek
                let mapping = output_mapping.expect("output_mapping required when input is present");
                let mut out_builder = DataChunkBuilder::new(
                    schema.columns().iter().map(|col| col.typ.clone()),
                    CHUNK_SIZE
                );

                let input_stream = input.open(ctx.clone())?;
                futures::pin_mut!(input_stream);

                for await input_chunk in input_stream {
                    let input_chunk = input_chunk?.compact();

                    // evaluate prop_value expressions on input chunk
                    let prop_value_arrays: Vec<_> = prop_value_exprs
                        .iter()
                        .map(|expr| expr.eval_batch(&input_chunk, &eval_ctx))
                        .collect::<Result<Vec<_>, _>>()?;

                    // for each input row, do index seek
                    for (row_idx, input_row) in input_chunk.iter().enumerate() {
                        // extract property values for this row and encode them
                        // if any value is null, skip this row (null can't match index)
                        let mut has_null = false;
                        let prop_values_encoded: Vec<Vec<u8>> = prop_value_arrays
                            .iter()
                            .map(|arr| {
                                match arr.get(row_idx) {
                                    Some(val) => IndexKeyCodec::encode_single(&val),
                                    None => {
                                        has_null = true;
                                        vec![]
                                    }
                                }
                            })
                            .collect();

                        if has_null {
                            continue; // skip this row, null value won't match any index
                        }

                        let prop_value_refs: Vec<&[u8]> = prop_values_encoded.iter().map(|v| v.as_slice()).collect();

                        // perform index lookup
                        let node_id = graph_store.get_unique_index(tx.as_ref(), label, &prop_tokens, &prop_value_refs)?;

                        if let Some(node_id) = node_id {
                            // build output row: combine input row and node
                            let mut output_row = Vec::with_capacity(mapping.len());
                            for source in &mapping {
                                let val = match source {
                                    OutputColumnSource::Left(idx) => input_row.get(*idx).cloned().flatten(),
                                    OutputColumnSource::Right(_) => {
                                        // node is always at index 0 in the "right" side
                                        Some(elio_common::scalar::ScalarRef::VirtualNode(node_id))
                                    }
                                };
                                output_row.push(val);
                            }
                            if let Some(chunk) = out_builder.append_row(output_row) {
                                yield chunk;
                            }
                        }
                        // if no node found, skip this input row (inner join semantics)
                    }
                }

                if let Some(chunk) = out_builder.yield_chunk() {
                    yield chunk;
                }
            } else {
                // no input: evaluate expressions on empty/unit chunk and do single seek
                // create a unit chunk for expression evaluation
                let unit_chunk = DataChunk::new(vec![], BitVec::repeat(true, 1));

                let mut has_null = false;
                let prop_values_encoded: Vec<Vec<u8>> = prop_value_exprs
                    .iter()
                    .map(|expr| {
                        let arr = expr.eval_batch(&unit_chunk, &eval_ctx)?;
                        let encoded = match arr.get(0) {
                            Some(val) => IndexKeyCodec::encode_single(&val),
                            None => {
                                has_null = true;
                                vec![]
                            }
                        };
                        Ok(encoded)
                    })
                    .collect::<Result<Vec<_>, ExecError>>()?;

                // if any value is null, no result
                if has_null {
                    return;
                }

                let prop_value_refs: Vec<&[u8]> = prop_values_encoded.iter().map(|v| v.as_slice()).collect();

                // perform single index lookup
                let node_id = graph_store.get_unique_index(tx.as_ref(), label, &prop_tokens, &prop_value_refs)?;

                if let Some(node_id) = node_id {
                    // found a node - create a single-row result
                    let mut builder = VirtualNodeArrayBuilder::with_capacity(1);
                    builder.push(Some(node_id));
                    let node_array = builder.finish();

                    let chunk = DataChunk::new(
                        vec![Arc::new(ArrayImpl::VirtualNode(node_array))],
                        BitVec::repeat(true, 1),
                    );
                    yield chunk;
                }
                // if no node found, yield nothing (empty result)
            }
        }
        .boxed();

        Ok(stream)
    }

    fn schema(&self) -> &Schema {
        &self.schema
    }

    fn name(&self) -> &'static str {
        "NodeIndexSeek"
    }
}
