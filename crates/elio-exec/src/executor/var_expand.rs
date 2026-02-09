use std::collections::VecDeque;
use std::sync::LazyLock;

use async_stream::try_stream;
use educe::Educe;
use elio_common::array::{ArrayImpl, DataChunkBuilder, RelArrayBuilder};
use elio_common::scalar::{ListValueRef, RelValue, ScalarRef, ScalarVTable, StructValue};
use elio_common::store_types::RelDirection;
use elio_common::{NodeId, SemanticDirection, TokenId, TokenKind};
use elio_expr::impl_::SharedExpression;
use elio_storage::codec::RelFormat;
use futures::StreamExt;
use indexmap::IndexSet;

use super::*;

// output direction schema: [input, rel, to]
#[derive(Educe)]
#[educe(Debug)]
pub struct VarExpandExecutor<PATHMODE: PathContainer, EXPANDKIND: ExpandKindStrategy> {
    pub input: SharedExecutor,
    pub from: usize,
    pub dir: SemanticDirection, // expansion direction
    pub rel_types: Arc<[TokenId]>,
    pub len_min: usize,
    pub len_max: usize,
    pub node_filter: Option<SharedExpression>,
    pub rel_filter: Option<SharedExpression>,
    #[educe(Debug(ignore))]
    pub path_container_factory: &'static PathContainerFactory<PATHMODE>,
    #[educe(Debug(ignore))]
    pub expand_kind_filter: EXPANDKIND,
    pub schema: Arc<Schema>,
}

impl<PATHMODE: PathContainer, EXPANDKIND: ExpandKindStrategy> Executor for VarExpandExecutor<PATHMODE, EXPANDKIND> {
    fn open(&self, ctx: Arc<TaskExecContext>) -> Result<DataChunkStream, ExecError> {
        let input_stream = self.input.open(ctx.clone())?;
        let schema = self.schema.clone();
        let from = self.from;
        let dir = self.dir;
        let rel_types = self.rel_types.clone();
        let len_min = self.len_min;
        let len_max = self.len_max;
        let _node_filter = self.node_filter.clone();
        let _rel_filter = self.rel_filter.clone();
        let path_container_factory = self.path_container_factory;
        let expand_kind_filter = self.expand_kind_filter.clone();

        let stream = try_stream! {
            let mut out_builder = DataChunkBuilder::new(schema.columns().iter().map(|col| col.typ.physical_type()), CHUNK_SIZE);
            for await chunk in input_stream{
                let outer = chunk?;
                let outer = outer.compact();
                for row in outer.iter() {
                    // if from is null, then remove this row
                    let from_id = match row[from].and_then(|id| id.get_node_id()){
                        Some(id) => id,
                        None => continue, // if from is null, then skip this row
                    };
                    let path_iter = VarExpandIter::<PATHMODE> {
                        stack: VecDeque::from([(from_id, (path_container_factory)())]),
                        ctx: ctx.clone(),
                        dir,
                        rel_types: rel_types.clone(),
                        min_len: len_min,
                        max_len: len_max,
                        // path_mode: (self.path_mode_factory)(),
                    };

                    for item in path_iter {
                        let (to_node, path) = item?;
                        let mut row = row.clone();
                        if path.len() >= len_min && expand_kind_filter.is_valid(&row, to_node) {
                            // push path rels list
                            let mut rel_array = RelArrayBuilder::with_capacity(path.len());
                            path.iter().for_each(|rel|
                                rel_array.push(Some(rel.as_scalar_ref()))
                            );
                            let rel_array: ArrayImpl= rel_array.finish().into();
                            row.push(Some(ScalarRef::List(ListValueRef::from_array(&rel_array, 0, rel_array.len()))));
                            EXPANDKIND::append_other_node(&mut row, to_node);
                            if let Some(chunk) = out_builder.append_row(row) {
                                yield chunk;
                            }
                        }
                    }
                }

                if let Some(chunk) = out_builder.yield_chunk() {
                    yield chunk;
                }
            }

        }
        .boxed();

        Ok(stream)
    }

    fn schema(&self) -> &Schema {
        &self.schema
    }

    fn name(&self) -> &'static str {
        "VarExpand"
    }
}

pub struct VarExpandIter<PATHMODE: PathContainer> {
    pub stack: VecDeque<(NodeId, PATHMODE)>,
    pub ctx: Arc<TaskExecContext>,
    pub dir: SemanticDirection,
    pub rel_types: Arc<[TokenId]>,
    pub min_len: usize,
    pub max_len: usize,
}

impl<PATHMODE: PathContainer> Iterator for VarExpandIter<PATHMODE> {
    type Item = Result<(NodeId, Vec<RelValue>), ExecError>;

    fn next(&mut self) -> Option<Self::Item> {
        // pop from stack
        let (node, path) = self.stack.pop_back()?;
        if path.len() < self.max_len {
            // expand node and path
            let rel_iter = match self.ctx.tx().rel_iter_for_node(node, self.dir, &self.rel_types) {
                Ok(rel_iter) => rel_iter,
                Err(e) => return Some(Err(e.into())),
            };

            // add expanded path to stack
            // for each rel, add (target, path + rel) to stack
            for rel_kv in rel_iter {
                let mut expanded_path = path.clone();
                let rel_kv = match rel_kv {
                    Ok(kv) => kv,
                    Err(e) => return Some(Err(e.into())),
                };
                let (from_id, rel_dir, token_id, to_id, rel_id, value) = rel_kv;
                // TODO(pgao): avoid get token value for each rel
                // maybe we can cache all the token value on the execution context
                let rel_type = match self.ctx.catalog().get_token_val(token_id, TokenKind::RelationshipType) {
                    Ok(rel_type) => rel_type,
                    Err(e) => return Some(Err(e.into())),
                };

                // TODO(pgao): we can know the start id and end id at planning time
                let (start_id, end_id) = match rel_dir {
                    RelDirection::Out => (from_id, to_id),
                    RelDirection::In => (to_id, from_id),
                };

                // TODO(pgao): lazy deserialize
                let prop_map = RelFormat::decode_value(&value);
                // TODO(pgao): avoid clone
                let struct_value = {
                    let mut fileds = vec![];
                    for entry in prop_map.iter() {
                        let key = match self
                            .ctx
                            .store()
                            .token_store()
                            .get_token_val(entry.key(), TokenKind::PropertyKey)
                        {
                            Ok(key) => key,
                            Err(e) => return Some(Err(e.into())),
                        };
                        // TODO(pgao): avoid clone
                        fileds.push((key, entry.value().to_owned_scalar()));
                    }
                    StructValue::new(fileds)
                };

                // TODO(pgao): expand into and filter etc
                let rel_value = RelValue {
                    id: rel_id,
                    reltype: rel_type,
                    start_id,
                    end_id,
                    props: struct_value,
                };

                if expanded_path.can_add_rel(&rel_value) {
                    expanded_path.add_rel(rel_value);
                    self.stack.push_back((to_id, expanded_path));
                }
            }
        }
        // emit (node, path)
        Some(Ok((node, path.into_list())))
    }
}

pub type PathContainerFactory<P> = Box<dyn Fn() -> P + Sync + Send>;

pub static TRAIL_PATH_MODE_FACTORY: LazyLock<PathContainerFactory<TrailPathContainer>> =
    LazyLock::new(|| Box::new(TrailPathContainer::default));
pub static WALK_PATH_MODE_FACTORY: LazyLock<PathContainerFactory<WalkPathContainer>> =
    LazyLock::new(|| Box::new(WalkPathContainer::default));

#[allow(clippy::len_without_is_empty)]
pub trait PathContainer: 'static + Sync + Send + Clone {
    // return true on this step is ok to expand
    // NOTE: each relationship have different relationship id
    fn can_add_rel(&self, step: &RelValue) -> bool;
    fn add_rel(&mut self, step: RelValue);
    fn into_list(self) -> Vec<RelValue>;
    fn len(&self) -> usize;
}

pub trait ExpandKindStrategy: 'static + Sync + Send + Clone {
    // True on the step is valid to expand
    fn is_valid(&self, row: &[Option<ScalarRef>], actual_to_id: NodeId) -> bool;
    // append to node id to the row
    fn append_other_node(row: &mut Vec<Option<ScalarRef>>, node_id: NodeId);
}

#[derive(Clone, Copy)]
pub struct ExpandIntoImpl {
    pub(crate) to_idx: usize,
}

impl ExpandKindStrategy for ExpandIntoImpl {
    fn is_valid(&self, row: &[Option<ScalarRef>], actual_to_id: NodeId) -> bool {
        let to_node_id = match row[self.to_idx].and_then(|id| id.get_node_id()) {
            Some(id) => id,
            None => return false, // if to is null, then skip this row
        };
        to_node_id == actual_to_id
    }

    fn append_other_node(_row: &mut Vec<Option<ScalarRef>>, _node_id: NodeId) {}
}

#[derive(Clone, Copy)]
pub struct ExpandAllImpl;

impl ExpandKindStrategy for ExpandAllImpl {
    fn is_valid(&self, _row: &[Option<ScalarRef>], _actual_to_id: NodeId) -> bool {
        true
    }

    fn append_other_node(row: &mut Vec<Option<ScalarRef>>, node_id: NodeId) {
        row.push(Some(ScalarRef::VirtualNode(node_id)));
    }
}

#[derive(Default, Clone)]
pub struct WalkPathContainer {
    pub path: Vec<RelValue>,
}
impl PathContainer for WalkPathContainer {
    fn can_add_rel(&self, _step: &RelValue) -> bool {
        true
    }

    fn add_rel(&mut self, step: RelValue) {
        self.path.push(step);
    }

    fn into_list(self) -> Vec<RelValue> {
        self.path
    }

    fn len(&self) -> usize {
        self.path.len()
    }
}

#[derive(Default, Clone)]
pub struct TrailPathContainer {
    pub(crate) path: IndexSet<RelValue>,
}

impl PathContainer for TrailPathContainer {
    fn can_add_rel(&self, step: &RelValue) -> bool {
        !self.path.contains(step)
    }

    fn add_rel(&mut self, step: RelValue) {
        self.path.insert(step);
    }

    fn into_list(self) -> Vec<RelValue> {
        self.path.into_iter().collect()
    }

    fn len(&self) -> usize {
        self.path.len()
    }
}
