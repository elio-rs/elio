use std::sync::Arc;

use elio_parser::ast;

use crate::binder::BindContext;
use crate::binder::builder::IrSingleQueryBuilder;
use crate::binder::scope::{Scope, ScopeItem};
use crate::error::{PlanError, SemanticError};
use crate::ir::query_project::{CsvLoadFormat, Load, LoadFormat, QueryProjection};

pub(crate) fn bind_load(
    bctx: &BindContext,
    builder: &mut IrSingleQueryBuilder,
    in_scope: Scope,
    load: &ast::LoadClause,
) -> Result<Scope, PlanError> {
    let ctx_name = load.to_string();

    // Semantic check 1: LOAD must be the first clause (in_scope should be empty)
    if !in_scope.items.is_empty() {
        return Err(SemanticError::load_must_be_first(&ctx_name).into());
    }

    // Semantic check 2: Variable name must not conflict with existing variables
    if in_scope.resolve_symbol(&load.variable).is_some() {
        return Err(SemanticError::variable_already_defined(&load.variable, &ctx_name).into());
    }

    // Semantic check 3: Format must be supported
    let format = parse_load_format(&load.format, &load.options, &ctx_name)?;

    // Create the Load IR
    let var_name = bctx.variable_generator.named(&load.variable);
    let ir_load = Load {
        variable: var_name.clone(),
        source_url: Arc::from(load.source.as_str()),
        format,
    };
    let output_type = ir_load.format.output_type();

    // Add Load projection to builder
    builder
        .tail_mut()
        .unwrap()
        .with_projection(QueryProjection::Load(ir_load));

    // Update scope with the new variable
    let mut out_scope = in_scope;
    let item = ScopeItem::new_variable(var_name, Some(&load.variable), output_type);
    out_scope.add_item(item);

    // create a new part
    builder.new_tail(out_scope.items.iter().map(|item| item.as_variable()).collect());

    Ok(out_scope)
}

/// Parse load format and options
fn parse_load_format(format: &str, options: &[ast::LoadOption], ctx: &str) -> Result<LoadFormat, PlanError> {
    match format.to_lowercase().as_str() {
        "csv" => {
            let csv_format = parse_csv_options(options, ctx)?;
            Ok(LoadFormat::Csv(csv_format))
        }
        _ => Err(SemanticError::unsupported_load_format(format, ctx).into()),
    }
}

/// Parse CSV-specific options
fn parse_csv_options(options: &[ast::LoadOption], ctx: &str) -> Result<CsvLoadFormat, PlanError> {
    let mut header = true; // default: has header
    let mut delimiter = ','; // default: comma

    for opt in options {
        match opt.key.to_lowercase().as_str() {
            "header" => {
                header = *opt
                    .value
                    .as_boolean()
                    .ok_or_else(|| SemanticError::invalid_option_type("header", "boolean", ctx))?;
            }
            "delimiter" => {
                let s = opt
                    .value
                    .as_string()
                    .ok_or_else(|| SemanticError::invalid_option_type("delimiter", "string", ctx))?;
                if s.len() != 1 {
                    return Err(SemanticError::invalid_option_value("delimiter", "single character", ctx).into());
                }
                delimiter = s.chars().next().unwrap();
            }
            _ => {
                return Err(SemanticError::unknown_load_option(&opt.key, ctx).into());
            }
        }
    }

    Ok(CsvLoadFormat { header, delimiter })
}
