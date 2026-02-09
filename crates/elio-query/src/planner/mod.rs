use std::sync::Arc;

use elio_common::variable::VariableName;
use indexmap::IndexMap;
use itertools::Itertools;
use pretty_xmlish::{Pretty, PrettyConfig};

use crate::plan::error::PlanError;
use crate::plan::ir::query::{IrQuery, IrQueryRoot, IrSingleQueryPart};
use crate::plan::plan_context::PlanContext;
use crate::plan::plan_node::{PlanExpr, ProduceResult, ProduceResultInner};
use crate::plan::session::PlannerSession;
use crate::planner::single_query::plan_single_query;

mod component;
mod create;
mod index_selection;
mod load;
mod match_;
mod project;
mod single_query;
mod tail;

// planner temporaray state
pub struct PlannerContext {
    ctx: Arc<PlanContext>,
    sess: Arc<dyn PlannerSession>,
    _config: PlannerConfig,
}

impl PlannerContext {
    pub fn new(sess: Arc<dyn PlannerSession>) -> Self {
        Self {
            ctx: Arc::new(PlanContext::default()),
            sess,
            _config: Default::default(),
        }
    }

    pub fn session(&self) -> &Arc<dyn PlannerSession> {
        &self.sess
    }
}

#[derive(Default)]
pub struct PlannerConfig {
    // planner options here
}

pub struct RootPlan {
    pub plan: Box<PlanExpr>,
    // TODO: convert to name to variable mapping
    pub names: IndexMap<String, VariableName>,
}

impl RootPlan {
    pub fn explain(&self) -> String {
        let fields = vec![(
            "names",
            Pretty::Array(self.names.iter().map(|(k, _)| Pretty::display(k)).collect_vec()),
        )];
        let children = vec![Pretty::Record(self.plan.xmlnode())];
        let tree = Pretty::simple_record("RootPlan", fields, children);
        let mut config = PrettyConfig {
            indent: 3,
            width: 2048,
            need_boundaries: false,
            reduced_spaces: true,
        };
        let mut output = String::with_capacity(2048);
        config.unicode(&mut output, &tree);
        output
    }
}

pub fn plan_root(
    ctx: &mut PlannerContext,
    _root @ IrQueryRoot {
        inner,
        output_names: names,
    }: &IrQueryRoot,
) -> Result<RootPlan, PlanError> {
    // let mut ctx = PlannerContext {
    //     ctx: plan_ctx.clone(),
    //     // generate from session context
    //     _config: Default::default(),
    // };

    let IrQuery { queries, union_all: _ } = inner;
    assert!(!queries.is_empty());
    if queries.len() > 1 {
        return Err(PlanError::not_supported("Union all is not supported yet".to_string()));
    }

    let plan = plan_single_query(ctx, &queries[0])?;

    // plan produce result
    let plan = {
        let inner = ProduceResultInner {
            input: plan,
            return_columns: names.iter().map(|(_, v)| v.clone()).collect_vec(),
        };
        PlanExpr::ProduceResult(ProduceResult::new(inner)).boxed()
    };

    Ok(RootPlan {
        plan,
        names: names.clone(),
    })
}
