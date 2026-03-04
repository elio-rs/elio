use elio_common::data_type::LogicalType;
use elio_common::schema::{Name2ColumnMap, Schema};

use crate::execution::builder::{BuildError, ExecutorBuildContext};
use crate::execution::expr::{
    ConstantExpr, CreateListExpr, CreateStructExpr, Expression, FieldAccessExpr, HasLabelExpr, ProjectPathExpr,
    ScalarCallExpr, SharedExpression, VariableRefExpr,
};
use crate::plan::expr;
use crate::plan::expr::{Constant, CreateList, CreateStruct, Expr, ExprNode, PropertyAccess, VariableRef};

pub struct BuildExprContext<'a> {
    pub schema: &'a Schema,
    pub name2col: Name2ColumnMap,
    pub ctx: &'a ExecutorBuildContext<'a>,
}

impl<'a> BuildExprContext<'a> {
    pub fn new(schema: &'a Schema, ctx: &'a ExecutorBuildContext) -> Self {
        let name2col = schema.name_to_col_map();
        Self { schema, name2col, ctx }
    }
}

pub(crate) fn build_expression(ctx: &BuildExprContext<'_>, expr: &Expr) -> Result<SharedExpression, BuildError> {
    match expr {
        Expr::VariableRef(variable_ref) => build_variable(ctx, variable_ref),
        Expr::PropertyAccess(property_access) => build_property_access(ctx, property_access),
        Expr::Constant(constant) => build_constant(ctx, constant),
        Expr::ScalarCall(func_call) => build_func_call(ctx, func_call),
        Expr::AggCall(_agg_call) => todo!(),
        Expr::Subquery(_subquery) => todo!(),
        Expr::HasLabel(has_label) => build_has_label(ctx, has_label),
        Expr::CreateStruct(create_map) => build_create_map(ctx, create_map),
        Expr::CreateList(create_list) => build_create_list(ctx, create_list),
        Expr::ProjectPath(project_path) => build_project_path(ctx, project_path),
    }
}

fn build_variable(ctx: &BuildExprContext<'_>, variable_ref: &VariableRef) -> Result<SharedExpression, BuildError> {
    let col = ctx
        .name2col
        .get(&variable_ref.name)
        .ok_or_else(|| BuildError::variable_not_found(variable_ref.name.clone()))?;
    let typ = ctx.schema.column(*col).typ.clone();
    Ok(VariableRefExpr::new(*col, typ).into_shared())
}

fn build_property_access(
    ctx: &BuildExprContext<'_>,
    property_access @ PropertyAccess { expr, property, .. }: &PropertyAccess,
) -> Result<SharedExpression, BuildError> {
    let input = build_expression(ctx, expr)?;
    let expr = FieldAccessExpr::new(input, property.clone(), property_access.typ());
    Ok(expr.into_shared())
}

fn build_constant(_ctx: &BuildExprContext<'_>, constant: &Constant) -> Result<SharedExpression, BuildError> {
    Ok(ConstantExpr {
        value: constant.data.clone(),
        // After null coercion in binder, all constants should have a type.
        // If not, fall back to Any type for runtime handling.
        typ: constant.typ.clone().unwrap_or(LogicalType::ANY),
    }
    .into_shared())
}

fn build_func_call(ctx: &BuildExprContext<'_>, func_call: &expr::ScalarCall) -> Result<SharedExpression, BuildError> {
    let args = func_call
        .args
        .iter()
        .map(|expr| build_expression(ctx, expr))
        .collect::<Result<Vec<_>, _>>()?;

    let function_data = func_call.function_data.clone().unwrap_or(Box::new(()));
    let function_exec = (func_call.function.exec_builder)(function_data)?;

    Ok(ScalarCallExpr {
        inputs: args,
        function_exec,
        typ: func_call.typ(),
    }
    .into_shared())
}

fn build_has_label(ctx: &BuildExprContext<'_>, has_label: &expr::HasLabel) -> Result<SharedExpression, BuildError> {
    let entity = build_expression(ctx, &has_label.entity)?;
    Ok(HasLabelExpr {
        entity,
        label: has_label.label_or_rel.clone(),
    }
    .into_shared())
}

fn build_create_map(ctx: &BuildExprContext<'_>, create_map: &CreateStruct) -> Result<SharedExpression, BuildError> {
    let properties = create_map
        .properties
        .iter()
        .map(|(token, expr)| build_expression(ctx, expr).map(|expr| (token.clone(), expr)))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(CreateStructExpr {
        fields: properties,
        typ: create_map.typ().clone(),
    }
    .into_shared())
}

fn build_create_list(ctx: &BuildExprContext<'_>, create_list: &CreateList) -> Result<SharedExpression, BuildError> {
    let elements = create_list
        .elements
        .iter()
        .map(|expr| build_expression(ctx, expr))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(CreateListExpr::new(elements, create_list.typ()).into_shared())
}

fn build_project_path(
    ctx: &BuildExprContext<'_>,
    project_path: &expr::ProjectPath,
) -> Result<SharedExpression, BuildError> {
    let inputs = project_path
        .step_variables()
        .into_iter()
        .map(|varref| build_expression(ctx, &varref.into()))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ProjectPathExpr {
        inputs,
        typ: project_path.typ(),
    }
    .into_shared())
}
