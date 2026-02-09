use elio_parser::ast;

use crate::binder::BindContext;
use crate::binder::builder::IrSingleQueryBuilder;
use crate::binder::expr::bind_expr;
use crate::binder::pattern::{PatternContext, bind_pattern};
use crate::binder::query::ClauseKind;
use crate::binder::scope::Scope;
use crate::plan::error::PlanError;
use crate::plan::expr::FilterExprs;
use crate::plan::ir::query_graph::QueryGraph;

pub(crate) fn bind_match(
    bctx: &BindContext,
    builder: &mut IrSingleQueryBuilder,
    in_scope: Scope,
    match_ @ ast::MatchClause {
        optional,
        mode,
        pattern,
        where_,
    }: &ast::MatchClause,
) -> Result<Scope, PlanError> {
    if matches!(mode, ast::MatchMode::WALK) {
        return Err(PlanError::not_supported("TRAIL walk mode not supported"));
    }

    let input_bindings = in_scope.input_bindings();

    // add the pattern graph to builder
    let pctx = PatternContext {
        bctx,
        clause: ClauseKind::Match,
        name: &match_.to_string(),
        allow_update: false,
        reject_qpp: false,
        reject_named_path: false,
        reject_selective: false,
    };
    let (paths, mut scope) = bind_pattern(&pctx, in_scope.clone(), &pattern.patterns)?;

    let mut qg = {
        let mut qg = QueryGraph::new(input_bindings.clone());
        paths.iter().for_each(|path| qg.add_path_pattern(path));
        qg
    };

    let filter = {
        let mut filter = FilterExprs::empty();
        if let Some(expr) = where_ {
            let ectx = bctx.derive_expr_context(&scope, "MATCH WHERE");
            let expr = bind_expr(&ectx, &[], expr)?;
            filter.push(expr);
        }
        filter
    };
    qg.add_filter(filter);

    if *optional {
        // delete:optional match qg input variables := in_scope intersect qg's match used variables
        // SAFETY: tail must exists
        // let imported: IndexSet<Variable> = in_scope.items.iter().map(|item| item.as_variable()).collect();
        // let qg_used_vars = qg.used_variables();
        // let optional_imported: IndexSet<Variable> = imported.intersection(&qg_used_vars).cloned().collect();
        // qg.add_imported_set(&optional_imported);
        qg.with_input_bindings(input_bindings.clone());
        builder.tail_mut().unwrap().add_optional_match_pattern(qg);
    } else {
        builder.tail_mut().unwrap().add_match_pattern(qg);
    }

    // TODO(pgao): semantic check match mode
    scope.remove_anonymous();
    Ok(scope)
}
