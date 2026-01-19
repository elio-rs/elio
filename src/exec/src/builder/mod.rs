use std::backtrace::Backtrace;
use std::sync::Arc;

use elio_common::mapb::IndexKeyCodec;
use elio_common::scalar::ScalarValue;
use elio_common::schema::Name2ColumnMap;
use elio_common::variable::VariableName;
use elio_cypher::ir::query_project::LoadFormat;
use elio_cypher::plan_node::{self, CreateNode, PlanExpr, PlanNode, Project};
use elio_cypher::planner::RootPlan;
use elio_expr::impl_::SharedExpression;

use crate::builder::expression::{BuildExprContext, build_expression};
use crate::executor::all_node_scan::AllNodeScanExectuor;
use crate::executor::apply::{ApplyExecutor, ArgumentContext, OutputColumnSource};
use crate::executor::argument::ArgumentExecutor;
use crate::executor::black_hole::BlackHoleExecutor;
use crate::executor::create_node::{CreateNodeExectuor, CreateNodeItem};
use crate::executor::create_rel::{CreateRelExectuor, CreateRelItem};
use crate::executor::expand::ExpandExecutor;
use crate::executor::filter::FilterExecutor;
use crate::executor::load_csv::LoadCsvExecutor;
use crate::executor::node_index_seek::NodeIndexSeekExecutor;
use crate::executor::produce_result::ProduceResultExecutor;
use crate::executor::project::ProjectExecutor;
use crate::executor::unit::UnitExecutor;
use crate::executor::var_expand::{
    ExpandAllImpl, ExpandIntoImpl, TRAIL_PATH_MODE_FACTORY, VarExpandExecutor, WALK_PATH_MODE_FACTORY,
};
use crate::executor::{Executor, SharedExecutor};
use crate::task::TaskExecContext;

pub mod expression;

#[derive(thiserror::Error, Debug)]
pub enum BuildError {
    #[error("variable {0} not found in schema")]
    VariableNotFound(VariableName, #[backtrace] Backtrace),
    #[error("token {0} not resolved")]
    UnresolvedToken(String, #[backtrace] Backtrace),
    #[error("malformed plan: {0}")]
    MalformedPlan(String, #[backtrace] Backtrace),
}

impl BuildError {
    pub fn variable_not_found(var_name: VariableName) -> Self {
        Self::VariableNotFound(var_name, Backtrace::capture())
    }

    pub fn unresolved_token(token: String) -> Self {
        Self::UnresolvedToken(token, Backtrace::capture())
    }
}

pub struct ExecutorBuildContext {
    pub ctx: Arc<TaskExecContext>,
    pub argument_ctx: Option<ArgumentContext>,
}

impl ExecutorBuildContext {
    pub fn new(ctx: Arc<TaskExecContext>) -> Self {
        Self {
            ctx,
            argument_ctx: None,
        }
    }
}

pub fn build_executor(
    ctx: &mut ExecutorBuildContext,
    _root @ RootPlan { plan, .. }: &RootPlan,
) -> Result<SharedExecutor, BuildError> {
    build_node(ctx, plan)
}

fn build_node(ctx: &mut ExecutorBuildContext, node: &PlanExpr) -> Result<SharedExecutor, BuildError> {
    // Handle Apply specially - don't pre-build right child
    if let PlanExpr::Apply(apply) = node {
        return build_apply(ctx, apply);
    }

    // Handle Argument specially - it uses the shared ArgumentContext
    if let PlanExpr::Argument(argument) = node {
        return build_argument(ctx, argument);
    }

    // For all other nodes, build children first
    let inputs = node
        .inputs()
        .iter()
        .map(|x| build_node(ctx, x))
        .collect::<Result<Vec<_>, _>>()?;

    match node {
        PlanExpr::AllNodeScan(all_node_scan) => build_all_node_scan(ctx, all_node_scan, inputs),
        PlanExpr::NodeIndexSeek(node_index_seek) => build_node_index_seek(ctx, node_index_seek, inputs),
        PlanExpr::GetProperty(_get_property) => todo!(),
        PlanExpr::Expand(expand) => build_expand(ctx, expand, inputs),
        PlanExpr::VarExpand(var_expand) => build_var_expand(ctx, var_expand, inputs),
        PlanExpr::Apply(_) => unreachable!("Apply is handled above"),
        PlanExpr::Argument(_) => unreachable!("Argument is handled above"),
        PlanExpr::Unit(_unit) => Ok(UnitExecutor::default().into_shared()),
        PlanExpr::ProduceResult(produce_result) => build_produce_result(ctx, produce_result, inputs),
        PlanExpr::CreateNode(create_node) => build_create_node(ctx, create_node, inputs),
        PlanExpr::CreateRel(create_rel) => build_create_rel(ctx, create_rel, inputs),
        PlanExpr::Load(load) => build_load(ctx, load, inputs),
        PlanExpr::Project(project) => build_project(ctx, project, inputs),
        PlanExpr::Sort(_sort) => todo!(),
        PlanExpr::Filter(filter) => build_filter(ctx, filter, inputs),
        PlanExpr::Pagination(_pagination) => todo!(),
        PlanExpr::Empty(_empty) => todo!(),
        PlanExpr::BlackHole(black_hole) => build_black_hole(ctx, black_hole, inputs),
    }
}

fn build_all_node_scan(
    _ctx: &mut ExecutorBuildContext,
    all_node_scan: &plan_node::AllNodeScan,
    inputs: Vec<SharedExecutor>,
) -> Result<SharedExecutor, BuildError> {
    assert_eq!(inputs.len(), 0);
    let schema = all_node_scan.schema();
    Ok(AllNodeScanExectuor::new(schema).into_shared())
}

fn build_node_index_seek(
    _ctx: &mut ExecutorBuildContext,
    node_index_seek: &plan_node::NodeIndexSeek,
    inputs: Vec<SharedExecutor>,
) -> Result<SharedExecutor, BuildError> {
    assert_eq!(inputs.len(), 0);

    let schema = node_index_seek.schema();

    // Extract constant values and encode them for index lookup
    // The encoding must match how values are stored in the index (using IndexKeyCodec)
    let property_values: Vec<Vec<u8>> = node_index_seek
        .inner()
        .property_values
        .iter()
        .map(|expr| {
            // For index lookup, the expression must be a constant
            match expr {
                elio_cypher::expr::Expr::Constant(constant) => {
                    // Encode using IndexKeyCodec (same as index storage)
                    let encoded = match &constant.data {
                        Some(value) => IndexKeyCodec::encode_single(&value.as_scalar_ref()),
                        None => IndexKeyCodec::encode_single(&ScalarValue::Unknown.as_scalar_ref()),
                    };
                    Ok(encoded)
                }
                _ => {
                    // Non-constant expressions not supported for index seek
                    // This shouldn't happen in practice, but handle gracefully
                    Err(BuildError::MalformedPlan(
                        format!("NodeIndexSeek requires constant property values, got: {:?}", expr),
                        Backtrace::capture(),
                    ))
                }
            }
        })
        .collect::<Result<Vec<_>, BuildError>>()?;

    Ok(NodeIndexSeekExecutor::new(
        schema,
        node_index_seek.inner().label_id,
        node_index_seek.inner().property_key_ids.clone(),
        property_values,
    )
    .into_shared())
}

fn build_expand(
    _ctx: &mut ExecutorBuildContext,
    expand: &plan_node::Expand,
    inputs: Vec<SharedExecutor>,
) -> Result<SharedExecutor, BuildError> {
    assert_eq!(inputs.len(), 1);
    let [input]: [SharedExecutor; 1] = inputs.try_into().unwrap();
    let schema = input.schema();
    let name2col = schema.name_to_col_map();

    let from = name2col
        .get(&expand.inner().from)
        .copied()
        .ok_or_else(|| BuildError::variable_not_found(expand.inner().from.clone()))?;

    let to = match expand.inner().kind {
        plan_node::ExpandKind::All => None,
        plan_node::ExpandKind::Into => Some(
            name2col
                .get(&expand.inner().to)
                .copied()
                .ok_or_else(|| BuildError::variable_not_found(expand.inner().to.clone()))?,
        ),
    };

    // assume all rtypes are token ids
    let rtype = expand
        .inner()
        .types
        .iter()
        .map(|x| match x {
            elio_common::IrToken::Resolved { token, .. } => Ok(*token),
            elio_common::IrToken::Unresolved(name) => Err(BuildError::unresolved_token(name.to_string())),
        })
        .collect::<Result<Vec<_>, _>>()?;

    match to {
        Some(to_idx) => Ok(ExpandExecutor {
            input,
            from,
            dir: expand.inner().direction,
            rtype,
            schema: expand.schema().clone(),
            expand_kind_filter: ExpandIntoImpl { to_idx },
        }
        .into_shared()),
        None => Ok(ExpandExecutor {
            input,
            from,
            dir: expand.inner().direction,
            rtype,
            schema: expand.schema().clone(),
            expand_kind_filter: ExpandAllImpl,
        }
        .into_shared()),
    }
}

fn build_var_expand(
    _ctx: &mut ExecutorBuildContext,
    expand: &plan_node::VarExpand,
    inputs: Vec<SharedExecutor>,
) -> Result<SharedExecutor, BuildError> {
    assert_eq!(inputs.len(), 1);
    let [input]: [SharedExecutor; 1] = inputs.try_into().unwrap();
    let schema = input.schema();
    let name2col = schema.name_to_col_map();

    let from = name2col
        .get(&expand.inner().from)
        .copied()
        .ok_or_else(|| BuildError::variable_not_found(expand.inner().from.clone()))?;

    let to = match expand.inner().kind {
        plan_node::ExpandKind::All => None,
        plan_node::ExpandKind::Into => Some(
            name2col
                .get(&expand.inner().to)
                .copied()
                .ok_or_else(|| BuildError::variable_not_found(expand.inner().to.clone()))?,
        ),
    };

    // assume all rtypes are token ids
    let rtype = expand
        .inner()
        .rel_pattern
        .types
        .iter()
        .map(|x| match x {
            elio_common::IrToken::Resolved { token, .. } => Ok(*token),
            elio_common::IrToken::Unresolved(name) => Err(BuildError::unresolved_token(name.to_string())),
        })
        .collect::<Result<Vec<_>, _>>()?;

    let (len_min, len_max) = expand.inner().rel_pattern.length.as_range().unwrap();

    match (expand.inner().path_mode, to) {
        (plan_node::PathMode::Trail, None) => Ok(VarExpandExecutor {
            input,
            from,
            dir: expand.inner().rel_pattern.dir,
            rel_types: rtype.into_boxed_slice().into(),
            len_min,
            len_max: len_max.unwrap_or(usize::MAX),
            node_filter: None,
            rel_filter: None,
            schema: expand.schema().clone(),
            path_container_factory: &TRAIL_PATH_MODE_FACTORY,
            expand_kind_filter: ExpandAllImpl,
        }
        .into_shared()),
        (plan_node::PathMode::Trail, Some(to_idx)) => Ok(VarExpandExecutor {
            input,
            from,
            dir: expand.inner().rel_pattern.dir,
            rel_types: rtype.into_boxed_slice().into(),
            len_min,
            len_max: len_max.unwrap_or(usize::MAX),
            node_filter: None,
            rel_filter: None,
            schema: expand.schema().clone(),
            path_container_factory: &TRAIL_PATH_MODE_FACTORY,
            expand_kind_filter: ExpandIntoImpl { to_idx },
        }
        .into_shared()),
        (plan_node::PathMode::Walk, None) => Ok(VarExpandExecutor {
            input,
            from,
            dir: expand.inner().rel_pattern.dir,
            rel_types: rtype.into_boxed_slice().into(),
            len_min,
            len_max: len_max.unwrap_or(usize::MAX),
            node_filter: None,
            rel_filter: None,
            schema: expand.schema().clone(),
            path_container_factory: &WALK_PATH_MODE_FACTORY,
            expand_kind_filter: ExpandAllImpl,
        }
        .into_shared()),

        (plan_node::PathMode::Walk, Some(to_idx)) => Ok(VarExpandExecutor {
            input,
            from,
            dir: expand.inner().rel_pattern.dir,
            rel_types: rtype.into_boxed_slice().into(),
            len_min,
            len_max: len_max.unwrap_or(usize::MAX),
            node_filter: None,
            rel_filter: None,
            schema: expand.schema().clone(),
            path_container_factory: &WALK_PATH_MODE_FACTORY,
            expand_kind_filter: ExpandIntoImpl { to_idx },
        }
        .into_shared()),
    }
}

fn build_apply(ctx: &mut ExecutorBuildContext, apply: &plan_node::Apply) -> Result<SharedExecutor, BuildError> {
    // Build only the left child first (without argument context)
    let left = build_node(ctx, &apply.inner().left)?;
    let left_schema = left.schema().clone();

    // Find argument variables from the right subtree
    let argument_vars = find_argument_variables(&apply.inner().right);

    // Create shared ArgumentContext
    let argument_ctx = ArgumentContext::default();

    // Build right child WITH argument context
    let mut right_ctx = ExecutorBuildContext {
        ctx: ctx.ctx.clone(),
        argument_ctx: Some(argument_ctx.clone()),
    };
    let right = build_node(&mut right_ctx, &apply.inner().right)?;

    // Map argument variables to left schema columns
    let left_name_to_col = left_schema.name_to_col_map();
    let argument_mapping: Vec<usize> = argument_vars
        .iter()
        .map(|var_name| {
            left_name_to_col
                .get(var_name)
                .copied()
                .ok_or_else(|| BuildError::variable_not_found(var_name.clone()))
        })
        .collect::<Result<Vec<_>, _>>()?;

    if argument_mapping.is_empty() {
        return Err(BuildError::MalformedPlan(
            "Apply node requires argument variables".to_string(),
            Backtrace::capture(),
        ));
    }

    // Compute output column mapping: for each output column, determine if it comes from left or right
    let right_schema = right.schema();
    let right_name_to_col = right_schema.name_to_col_map();
    let output_schema = apply.schema();

    let output_mapping: Vec<OutputColumnSource> = output_schema
        .columns()
        .iter()
        .map(|col| {
            // First check if the column exists in left schema
            if let Some(&left_idx) = left_name_to_col.get(&col.name) {
                Ok(OutputColumnSource::Left(left_idx))
            } else if let Some(&right_idx) = right_name_to_col.get(&col.name) {
                Ok(OutputColumnSource::Right(right_idx))
            } else {
                Err(BuildError::MalformedPlan(
                    format!("Output column {} not found in left or right schema", col.name),
                    Backtrace::capture(),
                ))
            }
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(ApplyExecutor {
        left,
        right,
        argument_ctx,
        schema: apply.schema().clone(),
        argument_mapping,
        output_mapping,
    }
    .into_shared())
}

/// Recursively find Argument nodes and return their variable names in order
/// TODO(pgao): handle nested Apply nodes
fn find_argument_variables(plan: &PlanExpr) -> Vec<VariableName> {
    match plan {
        PlanExpr::Argument(arg) => arg.schema().columns().iter().map(|f| f.name.clone()).collect(),
        _ => plan
            .inputs()
            .iter()
            .find_map(|child| {
                let vars = find_argument_variables(child);
                if vars.is_empty() { None } else { Some(vars) }
            })
            .unwrap_or_default(),
    }
}

fn build_argument(
    ctx: &mut ExecutorBuildContext,
    argument: &plan_node::Argument,
) -> Result<SharedExecutor, BuildError> {
    let argument_ctx = ctx.argument_ctx.clone().ok_or_else(|| {
        BuildError::MalformedPlan(
            "Argument node requires argument context (must be inside Apply)".to_string(),
            Backtrace::capture(),
        )
    })?;

    Ok(ArgumentExecutor {
        schema: argument.schema().clone(),
        argument_ctx,
    }
    .into_shared())
}

fn build_produce_result(
    _ctx: &mut ExecutorBuildContext,
    produce_result: &plan_node::ProduceResult,
    inputs: Vec<SharedExecutor>,
) -> Result<SharedExecutor, BuildError> {
    assert_eq!(inputs.len(), 1);
    let [input]: [SharedExecutor; 1] = inputs.try_into().unwrap();

    let schema = input.schema().clone();
    let name2col = schema.name_to_col_map();
    let return_columns = produce_result
        .inner()
        .return_columns
        .iter()
        .map(|var| {
            name2col
                .get(var)
                .copied()
                .ok_or_else(|| BuildError::variable_not_found(var.clone()))
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(ProduceResultExecutor {
        input,
        return_columns,
        schema: produce_result.schema().clone(),
    }
    .into_shared())
}

fn build_create_node(
    ctx: &mut ExecutorBuildContext,
    node: &CreateNode,
    inputs: Vec<SharedExecutor>,
) -> Result<SharedExecutor, BuildError> {
    assert_eq!(inputs.len(), 1);
    let [input]: [SharedExecutor; 1] = inputs.try_into().unwrap();

    let schema = input.schema().clone();
    let ectx = BuildExprContext::new(&schema, ctx);

    fn build_item(node: &plan_node::CreateNodeItem, ectx: &BuildExprContext) -> Result<CreateNodeItem, BuildError> {
        Ok(CreateNodeItem {
            labels: node.labels.clone(),
            properties: build_expression(ectx, &node.properties)?,
            variable: node.variable.clone(),
        })
    }

    let items = node
        .inner
        .nodes
        .iter()
        .map(|x| build_item(x, &ectx))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(CreateNodeExectuor {
        input,
        items,
        schema: node.schema().clone(),
    }
    .into_shared())
}

fn build_create_rel(
    ctx: &mut ExecutorBuildContext,
    node: &plan_node::CreateRel,
    inputs: Vec<SharedExecutor>,
) -> Result<SharedExecutor, BuildError> {
    assert_eq!(inputs.len(), 1);
    let [input]: [SharedExecutor; 1] = inputs.try_into().unwrap();

    let schema = input.schema().clone();
    let ectx = BuildExprContext::new(&schema, ctx);
    let name2col = schema.name_to_col_map();

    fn build_item(
        name2col: &Name2ColumnMap,
        rel: &plan_node::CreateRelItem,
        ectx: &BuildExprContext,
    ) -> Result<CreateRelItem, BuildError> {
        let start = name2col[&rel.start_node.name];
        let end = name2col[&rel.end_node.name];

        Ok(CreateRelItem {
            properties: build_expression(ectx, &rel.properties)?,
            rtype: rel.reltype.name().clone(),
            start,
            end,
        })
    }

    let items = node
        .inner()
        .rels
        .iter()
        .map(|x| build_item(&name2col, x, &ectx))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(CreateRelExectuor {
        input,
        items,
        schema: node.schema().clone(),
    }
    .into_shared())
}

fn build_project(
    ctx: &mut ExecutorBuildContext,
    node: &Project,
    inputs: Vec<SharedExecutor>,
) -> Result<SharedExecutor, BuildError> {
    assert_eq!(inputs.len(), 1);
    let [input]: [SharedExecutor; 1] = inputs.try_into().unwrap();

    let schema = input.schema().clone();
    let ectx = BuildExprContext::new(&schema, ctx);

    let out_name_to_col = node.schema().name_to_col_map();

    // findout project item order
    let project_items = &node.inner().projections;
    let mut out_idx_to_idx = vec![0; project_items.len()];
    let mut exprs: Vec<Option<SharedExpression>> = vec![];
    for (i, (var, expr)) in project_items.iter().enumerate() {
        let out_idx = out_name_to_col[var];
        out_idx_to_idx[out_idx] = i;
        let expr = build_expression(&ectx, expr)?;
        exprs.push(Some(expr));
    }

    // build exprs
    let mut project_exprs = vec![];
    for in_idx in out_idx_to_idx {
        project_exprs.push(exprs[in_idx].take().unwrap());
    }

    Ok(ProjectExecutor {
        input,
        exprs: project_exprs,
        // we assume the schema is with the same order of projections
        schema: node.schema().clone(),
    }
    .into_shared())
}

fn build_filter(
    ctx: &mut ExecutorBuildContext,
    node: &plan_node::Filter,
    inputs: Vec<SharedExecutor>,
) -> Result<SharedExecutor, BuildError> {
    assert_eq!(inputs.len(), 1);
    let [input]: [SharedExecutor; 1] = inputs.try_into().unwrap();

    let schema = input.schema().clone();
    let ectx = BuildExprContext::new(&schema, ctx);

    // TODO(pgao): special handle and optimize
    let expr = {
        let expr: elio_cypher::expr::Expr = node.inner().condition.clone().into();
        build_expression(&ectx, &expr)?
    };

    Ok(FilterExecutor {
        input,
        filter: expr,
        schema: node.schema().clone(),
    }
    .into_shared())
}

fn build_load(
    _ctx: &mut ExecutorBuildContext,
    load: &plan_node::Load,
    inputs: Vec<SharedExecutor>,
) -> Result<SharedExecutor, BuildError> {
    assert_eq!(inputs.len(), 0); // Load is a leaf node

    let inner = load.inner();
    let LoadFormat::Csv(csv_format) = &inner.format;

    Ok(LoadCsvExecutor {
        source_url: inner.source_url.clone().into(),
        format: csv_format.clone(),
        schema: load.schema(),
    }
    .into_shared())
}

fn build_black_hole(
    _ctx: &mut ExecutorBuildContext,
    black_hole: &plan_node::BlackHole,
    inputs: Vec<SharedExecutor>,
) -> Result<SharedExecutor, BuildError> {
    assert_eq!(inputs.len(), 1);
    let [input]: [SharedExecutor; 1] = inputs.try_into().unwrap();
    Ok(BlackHoleExecutor {
        input,
        schema: black_hole.schema().clone(),
    }
    .into_shared())
}
