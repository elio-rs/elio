use crate::plan_node::*;

/// Transform plan to another one by visiting each node.
pub trait PlanRewriter {
    fn rewrite(&mut self, plan: PlanExpr) -> PlanExpr {
        self.dispatch(plan)
    }

    fn dispatch(&mut self, plan: PlanExpr) -> PlanExpr {
        match plan {
            PlanExpr::AllNodeScan(p) => self.rewrite_all_node_scan(p),
            PlanExpr::NodeIndexSeek(p) => self.rewrite_node_index_seek(p),
            PlanExpr::GetProperty(p) => self.rewrite_get_property(p),
            PlanExpr::Expand(p) => self.rewrite_expand(p),
            PlanExpr::VarExpand(p) => self.rewrite_var_expand(p),
            PlanExpr::Apply(p) => self.rewrite_apply(p),
            PlanExpr::Argument(p) => self.rewrite_argument(p),
            PlanExpr::Unit(p) => self.rewrite_unit(p),
            PlanExpr::ProduceResult(p) => self.rewrite_produce_result(p),
            PlanExpr::CreateNode(p) => self.rewrite_create_node(p),
            PlanExpr::CreateRel(p) => self.rewrite_create_rel(p),
            PlanExpr::Load(p) => self.rewrite_load(p),
            PlanExpr::CrossProduct(p) => self.rewrite_cross_product(p),
            PlanExpr::Aggregate(p) => self.rewrite_aggregate(p),
            PlanExpr::Project(p) => self.rewrite_project(p),
            PlanExpr::Sort(p) => self.rewrite_sort(p),
            PlanExpr::Filter(p) => self.rewrite_filter(p),
            PlanExpr::Pagination(p) => self.rewrite_pagination(p),
            PlanExpr::Empty(p) => self.rewrite_empty(p),
            PlanExpr::BlackHole(p) => self.rewrite_black_hole(p),
        }
    }

    fn rewrite_children(&mut self, plan: PlanExpr) -> PlanExpr {
        plan.map_children(|child| self.rewrite(child))
    }

    fn rewrite_all_node_scan(&mut self, plan: AllNodeScan) -> PlanExpr {
        self.rewrite_children(PlanExpr::AllNodeScan(plan))
    }

    fn rewrite_node_index_seek(&mut self, plan: NodeIndexSeek) -> PlanExpr {
        self.rewrite_children(PlanExpr::NodeIndexSeek(plan))
    }

    fn rewrite_get_property(&mut self, plan: GetProperty) -> PlanExpr {
        self.rewrite_children(PlanExpr::GetProperty(plan))
    }

    fn rewrite_expand(&mut self, plan: Expand) -> PlanExpr {
        self.rewrite_children(PlanExpr::Expand(plan))
    }

    fn rewrite_var_expand(&mut self, plan: VarExpand) -> PlanExpr {
        self.rewrite_children(PlanExpr::VarExpand(plan))
    }

    fn rewrite_apply(&mut self, plan: Apply) -> PlanExpr {
        self.rewrite_children(PlanExpr::Apply(plan))
    }

    fn rewrite_argument(&mut self, plan: Argument) -> PlanExpr {
        PlanExpr::Argument(plan)
    }

    fn rewrite_unit(&mut self, plan: Unit) -> PlanExpr {
        PlanExpr::Unit(plan)
    }

    fn rewrite_produce_result(&mut self, plan: ProduceResult) -> PlanExpr {
        self.rewrite_children(PlanExpr::ProduceResult(plan))
    }

    fn rewrite_create_node(&mut self, plan: CreateNode) -> PlanExpr {
        self.rewrite_children(PlanExpr::CreateNode(plan))
    }

    fn rewrite_create_rel(&mut self, plan: CreateRel) -> PlanExpr {
        self.rewrite_children(PlanExpr::CreateRel(plan))
    }

    fn rewrite_load(&mut self, plan: Load) -> PlanExpr {
        PlanExpr::Load(plan)
    }

    fn rewrite_cross_product(&mut self, plan: CrossProduct) -> PlanExpr {
        self.rewrite_children(PlanExpr::CrossProduct(plan))
    }

    fn rewrite_aggregate(&mut self, plan: Aggregate) -> PlanExpr {
        self.rewrite_children(PlanExpr::Aggregate(plan))
    }

    fn rewrite_project(&mut self, plan: Project) -> PlanExpr {
        self.rewrite_children(PlanExpr::Project(plan))
    }

    fn rewrite_sort(&mut self, plan: Sort) -> PlanExpr {
        self.rewrite_children(PlanExpr::Sort(plan))
    }

    fn rewrite_filter(&mut self, plan: Filter) -> PlanExpr {
        self.rewrite_children(PlanExpr::Filter(plan))
    }

    fn rewrite_pagination(&mut self, plan: Pagination) -> PlanExpr {
        self.rewrite_children(PlanExpr::Pagination(plan))
    }

    fn rewrite_empty(&mut self, plan: Empty) -> PlanExpr {
        PlanExpr::Empty(plan)
    }

    fn rewrite_black_hole(&mut self, plan: BlackHole) -> PlanExpr {
        self.rewrite_children(PlanExpr::BlackHole(plan))
    }
}
