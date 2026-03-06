use std::backtrace::Backtrace;
use std::collections::HashMap;
use std::sync::Arc;

use elio_common::schema::{Name2ColumnMap, Schema};
use elio_common::variable::VariableName;

use crate::execution::QueryContext;
use crate::execution::builder::expression::{BuildExprContext, build_expression};
use crate::execution::executor::all_node_scan::AllNodeScanExectuor;
use crate::execution::executor::apply::{ApplyExecutor, ArgumentContext, OutputColumnSource};
use crate::execution::executor::argument::ArgumentExecutor;
use crate::execution::executor::black_hole::BlackHoleExecutor;
use crate::execution::executor::create_node::{CreateNodeExectuor, CreateNodeItem};
use crate::execution::executor::create_rel::{CreateRelExectuor, CreateRelItem};
use crate::execution::executor::cross_product::CrossProductExecutor;
use crate::execution::executor::distinct::DistinctExecutor;
use crate::execution::executor::expand::ExpandExecutor;
use crate::execution::executor::filter::FilterExecutor;
use crate::execution::executor::hash_aggregate::HashAggregateExecutor;
use crate::execution::executor::load_csv::LoadCsvExecutor;
use crate::execution::executor::node_by_label_scan::NodeByLabelScanExecutor;
use crate::execution::executor::node_index_seek::NodeIndexSeekExecutor;
use crate::execution::executor::pagination::PaginationExecutor;
use crate::execution::executor::produce_result::ProduceResultExecutor;
use crate::execution::executor::project::ProjectExecutor;
use crate::execution::executor::unit::UnitExecutor;
use crate::execution::executor::unwind::UnwindExecutor;
use crate::execution::executor::var_expand::{
    ExpandAllImpl, ExpandIntoImpl, TRAIL_PATH_MODE_FACTORY, VarExpandExecutor, WALK_PATH_MODE_FACTORY,
};
use crate::execution::executor::*;
use crate::execution::expr::SharedExpression;
use crate::execution::profile::{OperatorMetrics, ProfiledExecutor};
use crate::plan::expr::{self, AggCall, Expr, VariableRef};
use crate::plan::ir::query_project::LoadFormat;
use crate::plan::plan_node::{self, CreateNode, PlanExpr, PlanNode, Project};
use crate::planner::RootPlan;

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

pub struct ExecutorBuildContext<'a> {
    pub qctx: &'a Arc<QueryContext>,
    pub argument_ctx: Option<ArgumentContext>,
    pub profile: bool,
    pub profile_metrics: Vec<Arc<OperatorMetrics>>,
}

impl<'a> ExecutorBuildContext<'a> {
    pub fn new(ctx: &'a Arc<QueryContext>) -> Self {
        Self {
            qctx: ctx,
            argument_ctx: None,
            profile: false,
            profile_metrics: vec![],
        }
    }

    pub fn new_profiled(ctx: &'a Arc<QueryContext>) -> Self {
        Self {
            qctx: ctx,
            argument_ctx: None,
            profile: true,
            profile_metrics: vec![],
        }
    }
}

pub fn build_executor(
    ctx: &mut ExecutorBuildContext,
    _root @ RootPlan { plan, .. }: &RootPlan,
) -> Result<SharedExecutor, BuildError> {
    build_node(ctx, plan)
}

pub fn build_profiled_executor(
    ctx: &mut ExecutorBuildContext,
    _root @ RootPlan { plan, .. }: &RootPlan,
) -> Result<(SharedExecutor, Arc<OperatorMetrics>), BuildError> {
    assert!(ctx.profile, "build_profiled_executor requires profile mode");
    let executor = build_node(ctx, plan)?;
    // The root metrics should be the last (and only remaining) item
    assert_eq!(ctx.profile_metrics.len(), 1);
    let root_metrics = ctx.profile_metrics.pop().unwrap();
    Ok((executor, root_metrics))
}

fn build_node(ctx: &mut ExecutorBuildContext, node: &PlanExpr) -> Result<SharedExecutor, BuildError> {
    // Handle Apply specially - don't pre-build right child
    if let PlanExpr::Apply(apply) = node {
        let executor = build_apply(ctx, apply)?;
        return Ok(maybe_wrap_profiled(ctx, executor));
    }

    // Handle Argument specially - it uses the shared ArgumentContext
    if let PlanExpr::Argument(argument) = node {
        return build_argument(ctx, argument);
    }

    // Remember how many child metrics existed before building children
    let metrics_before = ctx.profile_metrics.len();

    // For all other nodes, build children first
    let inputs = node
        .inputs()
        .iter()
        .map(|x| build_node(ctx, x))
        .collect::<Result<Vec<_>, _>>()?;

    let executor = match node {
        PlanExpr::AllNodeScan(all_node_scan) => build_all_node_scan(ctx, all_node_scan, inputs),
        PlanExpr::NodeByLabelScan(node_by_label_scan) => build_node_by_label_scan(ctx, node_by_label_scan, inputs),
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
        PlanExpr::CrossProduct(cross_product) => build_cross_product(ctx, cross_product, inputs),
        PlanExpr::Aggregate(aggregate) => build_aggregate(ctx, aggregate, inputs),
        PlanExpr::Distinct(distinct) => build_distinct(ctx, distinct, inputs),
        PlanExpr::Project(project) => build_project(ctx, project, inputs),
        PlanExpr::Sort(_sort) => todo!(),
        PlanExpr::Filter(filter) => build_filter(ctx, filter, inputs),
        PlanExpr::Pagination(pagination) => build_pagination(ctx, pagination, inputs),
        PlanExpr::Unwind(unwind) => build_unwind(ctx, unwind, inputs),
        PlanExpr::Empty(_empty) => todo!(),
        PlanExpr::BlackHole(black_hole) => build_black_hole(ctx, black_hole, inputs),
    }?;

    if ctx.profile {
        // Collect child metrics that were pushed during recursive builds
        let child_metrics: Vec<Arc<OperatorMetrics>> = ctx.profile_metrics.drain(metrics_before..).collect();
        let metrics = Arc::new(OperatorMetrics::new(executor.name(), child_metrics));
        let profiled = ProfiledExecutor {
            inner: executor,
            metrics: metrics.clone(),
        };
        ctx.profile_metrics.push(metrics);
        Ok(profiled.into_shared())
    } else {
        Ok(executor)
    }
}

/// Wrap an executor with profiling if profile mode is enabled (for Apply which is built separately).
fn maybe_wrap_profiled(ctx: &mut ExecutorBuildContext, executor: SharedExecutor) -> SharedExecutor {
    if ctx.profile {
        // For Apply, children were already wrapped, but we can't easily extract their metrics
        // from the built executor. We wrap Apply itself as a single metrics node.
        let metrics = Arc::new(OperatorMetrics::new(executor.name(), vec![]));
        let profiled = ProfiledExecutor {
            inner: executor,
            metrics: metrics.clone(),
        };
        ctx.profile_metrics.push(metrics);
        profiled.into_shared()
    } else {
        executor
    }
}

fn build_all_node_scan(
    _ctx: &mut ExecutorBuildContext,
    all_node_scan: &plan_node::AllNodeScan,
    inputs: Vec<SharedExecutor>, // expected to be Argument or None
) -> Result<SharedExecutor, BuildError> {
    let schema = all_node_scan.schema();
    let arguments = &all_node_scan.inner().arguments;
    // if inputs is not empty, which means the all node scan's input is argument, and the executor is already built.

    if let Some(arguments) = arguments {
        assert_eq!(inputs.len(), 1);
        let argument_schema = arguments.schema();
        // build output mapping: arguments first (from input/left), then node (from scan/right)
        let mut output_mapping = Vec::with_capacity(schema.columns().len());
        let arg_name_to_col = argument_schema.name_to_col_map();

        for col in schema.columns() {
            if let Some(&arg_idx) = arg_name_to_col.get(&col.name) {
                // from argument (left/input)
                output_mapping.push(OutputColumnSource::Left(arg_idx));
            } else {
                // from node scan (right), node is always at index 0
                output_mapping.push(OutputColumnSource::Right(0));
            }
        }

        Ok(AllNodeScanExectuor::with_input(schema, inputs[0].clone(), output_mapping).into_shared())
    } else {
        Ok(AllNodeScanExectuor::new(schema).into_shared())
    }
}

fn build_node_by_label_scan(
    _ctx: &mut ExecutorBuildContext,
    node_by_label_scan: &plan_node::NodeByLabelScan,
    inputs: Vec<SharedExecutor>,
) -> Result<SharedExecutor, BuildError> {
    let schema = node_by_label_scan.schema();
    let arguments = &node_by_label_scan.inner().arguments;

    if let Some(arguments) = arguments {
        assert_eq!(inputs.len(), 1);
        let argument_schema = arguments.schema();
        let mut output_mapping = Vec::with_capacity(schema.columns().len());
        let arg_name_to_col = argument_schema.name_to_col_map();

        for col in schema.columns() {
            if let Some(&arg_idx) = arg_name_to_col.get(&col.name) {
                output_mapping.push(OutputColumnSource::Left(arg_idx));
            } else {
                output_mapping.push(OutputColumnSource::Right(0));
            }
        }

        Ok(NodeByLabelScanExecutor::with_input(
            schema,
            node_by_label_scan.inner().label.clone(),
            inputs[0].clone(),
            output_mapping,
        )
        .into_shared())
    } else {
        Ok(NodeByLabelScanExecutor::new(schema, node_by_label_scan.inner().label.clone()).into_shared())
    }
}

fn build_node_index_seek(
    ctx: &mut ExecutorBuildContext,
    node_index_seek: &plan_node::NodeIndexSeek,
    inputs: Vec<SharedExecutor>,
) -> Result<SharedExecutor, BuildError> {
    let schema = node_index_seek.schema();
    let arguments = &node_index_seek.inner().arguments;

    // build expression context based on input schema (if any)
    let input_schema = if let Some(arg) = arguments {
        arg.schema()
    } else {
        Arc::new(Schema::empty())
    };
    let ectx = BuildExprContext::new(&input_schema, ctx);

    // build prop_value expressions
    let prop_values: Vec<SharedExpression> = node_index_seek
        .inner()
        .prop_values
        .iter()
        .map(|expr| build_expression(&ectx, expr))
        .collect::<Result<Vec<_>, _>>()?;

    if let Some(_arguments) = arguments {
        // with input
        if inputs.len() != 1 {
            return Err(BuildError::MalformedPlan(
                "NodeIndexSeek with arguments expects exactly 1 input".to_string(),
                Backtrace::capture(),
            ));
        }
        let input = inputs.into_iter().next().unwrap();
        let input_schema = input.schema();

        // build output mapping: node first, then arguments
        let mut output_mapping = Vec::with_capacity(schema.columns().len());
        let input_name_to_col = input_schema.name_to_col_map();

        for col in schema.columns() {
            if let Some(&input_idx) = input_name_to_col.get(&col.name) {
                // from input (left)
                output_mapping.push(OutputColumnSource::Left(input_idx));
            } else {
                // from index seek result (right), node is always at index 0
                output_mapping.push(OutputColumnSource::Right(0));
            }
        }

        Ok(NodeIndexSeekExecutor::with_input(
            schema,
            node_index_seek.inner().label.clone(),
            node_index_seek.inner().prop_tokens.clone(),
            prop_values,
            input,
            output_mapping,
        )
        .into_shared())
    } else {
        // no input
        Ok(NodeIndexSeekExecutor::new(
            schema,
            node_index_seek.inner().label.clone(),
            node_index_seek.inner().prop_tokens.clone(),
            prop_values,
        )
        .into_shared())
    }
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
        qctx: ctx.qctx,
        argument_ctx: Some(argument_ctx.clone()),
        profile: ctx.profile,
        profile_metrics: vec![],
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

fn build_aggregate(
    _ctx: &mut ExecutorBuildContext,
    node: &plan_node::Aggregate,
    inputs: Vec<SharedExecutor>,
) -> Result<SharedExecutor, BuildError> {
    assert_eq!(inputs.len(), 1);
    let [input]: [SharedExecutor; 1] = inputs.try_into().unwrap();

    let input_schema = input.schema();
    let input_name2col = input_schema.name_to_col_map();

    // group key column indexes
    let mut group_idx = Vec::with_capacity(node.inner().group_by.len());
    for (_var, expr) in node.inner().group_by.iter() {
        let idx = resolve_col_ref(&input_name2col, expr, "group key")?;
        group_idx.push(idx);
    }

    // agg args column indexes and agg impls
    let mut agg_args: Vec<Vec<usize>> = Vec::with_capacity(node.inner().aggregate.len());
    let mut aggs = Vec::with_capacity(node.inner().aggregate.len());

    for (_var, expr) in node.inner().aggregate.iter() {
        let Expr::AggCall(AggCall {
            function,
            function_data,
            args,
            distinct,
            ..
        }) = expr
        else {
            return Err(BuildError::MalformedPlan(
                "aggregate item must be an aggregate call".to_string(),
                Backtrace::capture(),
            ));
        };
        // TODO(pgao): support distinct
        if *distinct {
            return Err(BuildError::MalformedPlan(
                "DISTINCT aggregate not supported yet".to_string(),
                Backtrace::capture(),
            ));
        }

        let arg_idx: Vec<usize> = args
            .iter()
            .map(|a| resolve_col_ref(&input_name2col, a, "aggregate argument"))
            .collect::<Result<_, _>>()?;
        agg_args.push(arg_idx);

        let function_data = function_data.clone().unwrap_or(Box::new(()));
        let agg_exec = (function.exec_builder)(function_data)?;
        aggs.push(agg_exec);
    }

    // output mapping follows logical schema order (could interleave group/agg).
    let group_pos: HashMap<_, _> = node
        .inner()
        .group_by
        .iter()
        .enumerate()
        .map(|(i, (v, _))| (v.clone(), i))
        .collect();
    let agg_pos: HashMap<_, _> = node
        .inner()
        .aggregate
        .iter()
        .enumerate()
        .map(|(i, (v, _))| (v.clone(), i))
        .collect();

    let mut output_mapping = Vec::with_capacity(node.schema().columns().len());
    for col in node.schema().columns() {
        if let Some(idx) = group_pos.get(&col.name) {
            output_mapping.push(OutputColumnSource::Left(*idx));
        } else if let Some(idx) = agg_pos.get(&col.name) {
            output_mapping.push(OutputColumnSource::Right(*idx));
        } else {
            return Err(BuildError::MalformedPlan(
                format!("column {} not found in group_by or aggregate", col.name),
                Backtrace::capture(),
            ));
        }
    }

    Ok(HashAggregateExecutor {
        input,
        group_by: group_idx,
        agg_args,
        output_mapping,
        aggs,
        schema: node.schema().clone(),
    }
    .into_shared())
}

fn build_distinct(
    ctx: &mut ExecutorBuildContext,
    node: &plan_node::Distinct,
    inputs: Vec<SharedExecutor>,
) -> Result<SharedExecutor, BuildError> {
    assert_eq!(inputs.len(), 1);
    let [input]: [SharedExecutor; 1] = inputs.try_into().unwrap();

    let input_schema = input.schema().clone();
    let ectx = BuildExprContext::new(&input_schema, ctx);

    let group_exprs = node
        .inner()
        .group_exprs
        .iter()
        .map(|(_, expr)| build_expression(&ectx, expr))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(DistinctExecutor {
        input,
        group_exprs,
        schema: node.schema().clone(),
    }
    .into_shared())
}

fn resolve_col_ref(name2col: &Name2ColumnMap, expr: &Expr, ctx: &str) -> Result<usize, BuildError> {
    if let Expr::VariableRef(VariableRef { name, .. }) = expr {
        name2col
            .get(name)
            .copied()
            .ok_or_else(|| BuildError::variable_not_found(name.clone()))
    } else {
        Err(BuildError::MalformedPlan(
            format!("{} must be a column reference, got {:?}", ctx, expr),
            Backtrace::capture(),
        ))
    }
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
        let expr: expr::Expr = node.inner().condition.clone().into();
        build_expression(&ectx, &expr)?
    };

    Ok(FilterExecutor {
        input,
        filter: expr,
        schema: node.schema().clone(),
    }
    .into_shared())
}

fn build_pagination(
    _ctx: &mut ExecutorBuildContext,
    pagination: &plan_node::Pagination,
    inputs: Vec<SharedExecutor>,
) -> Result<SharedExecutor, BuildError> {
    assert_eq!(inputs.len(), 1);
    let [input]: [SharedExecutor; 1] = inputs.try_into().unwrap();

    Ok(PaginationExecutor {
        input,
        offset: pagination.inner().offset,
        limit: pagination.inner().limit,
        schema: pagination.schema().clone(),
    }
    .into_shared())
}

fn build_unwind(
    ctx: &mut ExecutorBuildContext,
    node: &plan_node::Unwind,
    inputs: Vec<SharedExecutor>,
) -> Result<SharedExecutor, BuildError> {
    assert_eq!(inputs.len(), 1);
    let [input]: [SharedExecutor; 1] = inputs.try_into().unwrap();

    let schema = input.schema().clone();
    let ectx = BuildExprContext::new(&schema, ctx);

    let expr = build_expression(&ectx, &node.inner().expr)?;

    Ok(UnwindExecutor {
        input,
        expr,
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

fn build_cross_product(
    _ctx: &mut ExecutorBuildContext,
    cross_product: &plan_node::CrossProduct,
    inputs: Vec<SharedExecutor>,
) -> Result<SharedExecutor, BuildError> {
    assert_eq!(inputs.len(), 2);
    let [left, right]: [SharedExecutor; 2] = inputs.try_into().unwrap();

    let left_schema = left.schema();
    let right_schema = right.schema();
    let output_schema = cross_product.schema();

    let left_name_to_col = left_schema.name_to_col_map();
    let right_name_to_col = right_schema.name_to_col_map();

    // build output mapping: for overlapping variables, prefer left side
    let output_mapping: Vec<OutputColumnSource> = output_schema
        .columns()
        .iter()
        .map(|col| {
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

    Ok(CrossProductExecutor {
        left,
        right,
        schema: cross_product.schema().clone(),
        output_mapping,
    }
    .into_shared())
}
