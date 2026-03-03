//! Strategy to plan an connected component.

use std::collections::VecDeque;

use elio_common::data_type::LogicalType;
use elio_common::schema::{Schema, Variable};
use elio_common::variable::VariableName;
use indexmap::IndexSet;
use itertools::Itertools;

use super::index_selection::find_index_candidates;
use super::*;
use crate::plan::expr::FilterExprs;
use crate::plan::ir::node_connection::RelPattern;
use crate::plan::ir::query_graph::QueryGraph;
use crate::plan::plan_node::{
    AllNodeScan, AllNodeScanInner, Argument, ArgumentInner, Expand, ExpandInner, ExpandKind, Filter, FilterInner,
    NodeIndexSeek, NodeIndexSeekInner, PathMode, VarExpand, VarExpandInner,
};

// This is an simple implementation of planning an query graph.
// we do not handle optionl match and update pattern in qg here.
// So, if qg's node and imported is both empty, then qg is empty.
pub(crate) fn plan_qg_simple(ctx: &mut PlannerContext, qg: &QueryGraph) -> Result<Box<PlanExpr>, PlanError> {
    if qg.nodes.is_empty() && qg.input_bindings().is_empty() {
        return Ok(PlanExpr::empty(Schema::empty().into(), ctx.ctx.clone()).boxed());
    }

    // Try to find an index that can be used for the first node
    let (solver, remaining_filter) = TraversalSolver::new_with_index_selection(ctx, qg);
    let mut solver = solver;
    solver.solve()?;
    let mut root = solver.root;

    // solve remaining filter (after index conditions removed)
    if !remaining_filter.is_true() {
        root = Filter::new(FilterInner {
            input: root,
            condition: remaining_filter,
        })
        .into();
    }

    Ok(root)
}

/// Solve the query graph by Traversal strategy:
/// 1. select start node to traversal, generate an plan leaf
///   - Argument
///   - NodeScan
/// 2. select node connection by the given node
///   - Expand
/// 3. if the node have multiple node connections, we have two strategy 3.1 DFS: this is what we currently doing 3.2 BFS
///
/// This only solves graph traversal, filter and get properties not solved by this class.
///
/// Topology is our first class citizon, purly solve the graph traversal.
struct TraversalSolver<'a> {
    pub _ctx: &'a PlannerContext<'a>,
    pub qg: &'a QueryGraph,
    solved: IndexSet<VariableName>,
    stack: VecDeque<&'a RelPattern>,
    root: Box<PlanExpr>,
}

impl<'a> TraversalSolver<'a> {
    /// Create solver with index selection optimization
    /// Returns (solver, remaining_filter) where remaining_filter has index conditions removed
    fn new_with_index_selection(ctx: &'a PlannerContext<'a>, qg: &'a QueryGraph) -> (Self, FilterExprs) {
        assert!(!qg.nodes.is_empty() || !qg.input_bindings().is_empty());
        let imported = qg.input_bindings().clone();
        let mut solved = IndexSet::new();
        let mut stack = VecDeque::new();
        let mut remaining_filter = qg.filter.clone();

        let mut qg_nodes = qg.nodes.iter();

        let mut root = if !imported.is_empty() {
            // imported as argument as plan leaf
            let inner = ArgumentInner {
                variables: imported.iter().cloned().collect_vec(),
                ctx: ctx.ctx.clone(),
            };
            let root: PlanExpr = Argument::new(inner).into();
            // if arguments have node connections, push the connection on stack
            for arg in imported.iter().filter(|i| i.is_node()).map(|x| &x.name) {
                // push in reverse order, since it's stack
                for conn in qg.connections(arg).rev() {
                    stack.push_back(conn);
                }
            }

            // at this point, all arguments are solved
            imported.iter().for_each(|i| {
                solved.insert(i.name.clone());
            });
            Some(root)
        } else {
            None
        };

        if stack.is_empty() && !qg.nodes.is_empty() {
            // Try to find an index for the first node
            let first = qg_nodes.next().unwrap();
            let first = Variable::new(first, &LogicalType::VIRTUAL_NODE);

            // Check if we can use an index for this node
            let (plan, filter) = Self::try_create_index_seek(ctx, &first, &qg.filter, &imported).unwrap_or_else(|| {
                // Fallback to AllNodeScan
                let arguments = (!imported.is_empty()).then(|| {
                    Argument::new(ArgumentInner {
                        variables: imported.iter().cloned().collect_vec(),
                        ctx: ctx.ctx.clone(),
                    })
                    .into()
                });
                let inner = AllNodeScanInner::new(first.name.clone(), arguments, ctx.ctx.clone());
                (AllNodeScan::new(inner).into(), qg.filter.clone())
            });

            root = Some(plan);
            remaining_filter = filter;
            solved.insert(first.name.clone());

            // push connections on stack
            for conn in qg.connections(&first.name).rev() {
                stack.push_back(conn);
            }
        }

        (
            Self {
                _ctx: ctx,
                qg,
                solved,
                stack,
                // SAFETY: imported.is_empty() and !stack.is_empty() won't happen at the same time.
                root: root.unwrap().into(),
            },
            remaining_filter,
        )
    }

    /// Try to create a NodeIndexSeek for the given node variable
    /// Returns Some((plan, remaining_filter)) if an index can be used, None otherwise
    fn try_create_index_seek(
        ctx: &PlannerContext,
        node_var: &Variable,
        filter: &FilterExprs,
        arguments: &IndexSet<Variable>,
    ) -> Option<(PlanExpr, FilterExprs)> {
        // Find index candidates for this node
        let candidate = find_index_candidates(ctx, arguments, filter, node_var)?;

        let arguments = if !arguments.is_empty() {
            Some(
                Argument::new(ArgumentInner {
                    variables: arguments.iter().cloned().collect_vec(),
                    ctx: ctx.ctx.clone(),
                })
                .into(),
            )
        } else {
            None
        };
        // Create NodeIndexSeek plan
        let plan = NodeIndexSeek::new(NodeIndexSeekInner::new(
            candidate.variable.clone(),
            candidate.label.clone(),
            candidate.index_hint.index_name.clone(),
            candidate.prop_tokens.clone(),
            candidate.prop_values.clone(),
            ctx.ctx.clone(),
            arguments,
        ));

        // Remove conditions covered by the index from the filter
        let remaining_filter = filter.diff(&candidate.solved_predicate);

        Some((plan.into(), remaining_filter))
    }

    // DFS traversal
    // the final plan will be placed at root
    fn solve(&mut self) -> Result<(), PlanError> {
        while let Some(rel) = self.stack.pop_back() {
            if !self.solved.contains(&rel.variable) {
                self.solve_node_connection_by_expand(rel)?;
            }
        }
        Ok(())
    }

    fn solve_node_connection_by_expand(
        &mut self,
        rel @ RelPattern {
            variable,
            endpoints: (left, right),
            dir,
            types,
            length,
        }: &'a RelPattern,
    ) -> Result<(), PlanError> {
        // if !length.is_simple() {
        //     return Err(PlanError::not_supported(
        //         "variable length relationship is not supported yet".to_string(),
        //     ));
        // }
        let (kind, from, to, direction, expanded_node) = {
            match (self.solved.contains(left), self.solved.contains(right)) {
                (true, true) => (ExpandKind::Into, left, right, *dir, None),
                (true, false) => (ExpandKind::All, left, right, *dir, Some(right.clone())),
                (false, true) => (ExpandKind::All, right, left, dir.rev(), Some(left.clone())),
                (false, false) => unreachable!(),
            }
        };

        let empty = PlanExpr::empty(Schema::empty().into(), self.root.ctx()).boxed();
        if length.is_simple() {
            // expand
            let inner = ExpandInner {
                input: std::mem::replace(&mut self.root, empty),
                from: from.clone(),
                to: to.clone(),
                rel: variable.clone(),
                direction,
                types: types.clone(),
                kind,
            };
            self.root = Expand::new(inner).into();
        } else {
            // variable length expand
            let inner = VarExpandInner {
                input: std::mem::replace(&mut self.root, empty),
                from: from.clone(),
                to: to.clone(),
                rel_pattern: rel.clone(),
                // TODO(pgao): support filter
                node_filter: FilterExprs::default(),
                rel_filter: FilterExprs::default(),
                kind,
                // TODO(pgao): path mode support
                path_mode: PathMode::default(),
            };
            self.root = VarExpand::new(inner).into();
        }

        self.solved.insert(variable.clone());

        // push new expanded vertex's connections into stack
        if let Some(expanded) = expanded_node {
            self.solved.insert(expanded.clone());
            self.qg
                .connections(&expanded)
                .rev()
                .for_each(|rel| self.stack.push_back(rel));
        }

        Ok(())
    }
}
