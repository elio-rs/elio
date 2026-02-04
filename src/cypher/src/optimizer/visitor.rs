use crate::plan_node::*;

/// Visit plan tree and perform some actions.
/// Default implementation walks children and returns `Default::default()`.
pub trait PlanVisitor<C = ()> {
    type Output: Default;

    fn visit(&mut self, plan: &PlanExpr, ctx: &C) -> Self::Output {
        self.dispatch(plan, ctx)
    }

    fn dispatch(&mut self, plan: &PlanExpr, ctx: &C) -> Self::Output {
        match plan {
            PlanExpr::AllNodeScan(p) => self.visit_all_node_scan(p, ctx),
            PlanExpr::NodeIndexSeek(p) => self.visit_node_index_seek(p, ctx),
            PlanExpr::GetProperty(p) => self.visit_get_property(p, ctx),
            PlanExpr::Expand(p) => self.visit_expand(p, ctx),
            PlanExpr::VarExpand(p) => self.visit_var_expand(p, ctx),
            PlanExpr::Apply(p) => self.visit_apply(p, ctx),
            PlanExpr::Argument(p) => self.visit_argument(p, ctx),
            PlanExpr::Unit(p) => self.visit_unit(p, ctx),
            PlanExpr::ProduceResult(p) => self.visit_produce_result(p, ctx),
            PlanExpr::CreateNode(p) => self.visit_create_node(p, ctx),
            PlanExpr::CreateRel(p) => self.visit_create_rel(p, ctx),
            PlanExpr::Load(p) => self.visit_load(p, ctx),
            PlanExpr::CrossProduct(p) => self.visit_cross_product(p, ctx),
            PlanExpr::Aggregate(p) => self.visit_aggregate(p, ctx),
            PlanExpr::Project(p) => self.visit_project(p, ctx),
            PlanExpr::Sort(p) => self.visit_sort(p, ctx),
            PlanExpr::Filter(p) => self.visit_filter(p, ctx),
            PlanExpr::Pagination(p) => self.visit_pagination(p, ctx),
            PlanExpr::Empty(p) => self.visit_empty(p, ctx),
            PlanExpr::BlackHole(p) => self.visit_black_hole(p, ctx),
        }
    }

    fn visit_children(&mut self, plan: &PlanExpr, ctx: &C) -> Self::Output {
        for child in plan.inputs() {
            self.visit(child, ctx);
        }
        Self::Output::default()
    }

    fn visit_all_node_scan(&mut self, plan: &AllNodeScan, ctx: &C) -> Self::Output {
        self.visit_children(&PlanExpr::AllNodeScan(plan.clone()), ctx)
    }

    fn visit_node_index_seek(&mut self, plan: &NodeIndexSeek, ctx: &C) -> Self::Output {
        self.visit_children(&PlanExpr::NodeIndexSeek(plan.clone()), ctx)
    }

    fn visit_get_property(&mut self, plan: &GetProperty, ctx: &C) -> Self::Output {
        self.visit_children(&PlanExpr::GetProperty(plan.clone()), ctx)
    }

    fn visit_expand(&mut self, plan: &Expand, ctx: &C) -> Self::Output {
        self.visit_children(&PlanExpr::Expand(plan.clone()), ctx)
    }

    fn visit_var_expand(&mut self, plan: &VarExpand, ctx: &C) -> Self::Output {
        self.visit_children(&PlanExpr::VarExpand(plan.clone()), ctx)
    }

    fn visit_apply(&mut self, plan: &Apply, ctx: &C) -> Self::Output {
        self.visit_children(&PlanExpr::Apply(plan.clone()), ctx)
    }

    fn visit_argument(&mut self, _plan: &Argument, _ctx: &C) -> Self::Output {
        Self::Output::default()
    }

    fn visit_unit(&mut self, _plan: &Unit, _ctx: &C) -> Self::Output {
        Self::Output::default()
    }

    fn visit_produce_result(&mut self, plan: &ProduceResult, ctx: &C) -> Self::Output {
        self.visit_children(&PlanExpr::ProduceResult(plan.clone()), ctx)
    }

    fn visit_create_node(&mut self, plan: &CreateNode, ctx: &C) -> Self::Output {
        self.visit_children(&PlanExpr::CreateNode(plan.clone()), ctx)
    }

    fn visit_create_rel(&mut self, plan: &CreateRel, ctx: &C) -> Self::Output {
        self.visit_children(&PlanExpr::CreateRel(plan.clone()), ctx)
    }

    fn visit_load(&mut self, _plan: &Load, _ctx: &C) -> Self::Output {
        Self::Output::default()
    }

    fn visit_cross_product(&mut self, plan: &CrossProduct, ctx: &C) -> Self::Output {
        self.visit_children(&PlanExpr::CrossProduct(plan.clone()), ctx)
    }

    fn visit_aggregate(&mut self, plan: &Aggregate, ctx: &C) -> Self::Output {
        self.visit_children(&PlanExpr::Aggregate(plan.clone()), ctx)
    }

    fn visit_project(&mut self, plan: &Project, ctx: &C) -> Self::Output {
        self.visit_children(&PlanExpr::Project(plan.clone()), ctx)
    }

    fn visit_sort(&mut self, plan: &Sort, ctx: &C) -> Self::Output {
        self.visit_children(&PlanExpr::Sort(plan.clone()), ctx)
    }

    fn visit_filter(&mut self, plan: &Filter, ctx: &C) -> Self::Output {
        self.visit_children(&PlanExpr::Filter(plan.clone()), ctx)
    }

    fn visit_pagination(&mut self, plan: &Pagination, ctx: &C) -> Self::Output {
        self.visit_children(&PlanExpr::Pagination(plan.clone()), ctx)
    }

    fn visit_empty(&mut self, _plan: &Empty, _ctx: &C) -> Self::Output {
        Self::Output::default()
    }

    fn visit_black_hole(&mut self, plan: &BlackHole, ctx: &C) -> Self::Output {
        self.visit_children(&PlanExpr::BlackHole(plan.clone()), ctx)
    }
}
