use crate::error::PlanError;
use crate::ir::query_project::Load as IrLoad;
use crate::plan_node::{Load, LoadInner, PlanExpr};
use crate::planner::PlannerContext;

pub fn plan_load(ctx: &mut PlannerContext, ir_load: &IrLoad) -> Result<Box<PlanExpr>, PlanError> {
    let inner = LoadInner {
        ctx: ctx.ctx.clone(),
        source_url: ir_load.source_url.to_string(),
        variable: ir_load.variable.clone(),
        format: ir_load.format.clone(),
    };

    Ok(Load::new(inner).into())
}
