use crate::plan::expr::*;

/// Read-only traversal over expressions.
/// Default impl walks all children and returns `Default::default()`.
pub trait ExprVisitor<C = ()> {
    type Output: Default;

    fn visit(&mut self, expr: &Expr, ctx: &C) -> Self::Output {
        self.dispatch(expr, ctx)
    }

    fn dispatch(&mut self, expr: &Expr, ctx: &C) -> Self::Output {
        match expr {
            Expr::VariableRef(e) => self.visit_variable_ref(e, ctx),
            Expr::PropertyAccess(e) => self.visit_property_access(e, ctx),
            Expr::Constant(e) => self.visit_constant(e, ctx),
            Expr::FuncCall(e) => self.visit_func_call(e, ctx),
            Expr::AggCall(e) => self.visit_agg_call(e, ctx),
            Expr::Subquery(e) => self.visit_subquery(e, ctx),
            Expr::HasLabel(e) => self.visit_has_label(e, ctx),
            Expr::CreateStruct(e) => self.visit_create_struct(e, ctx),
            Expr::CreateList(e) => self.visit_create_list(e, ctx),
            Expr::ProjectPath(e) => self.visit_project_path(e, ctx),
        }
    }

    fn visit_children(&mut self, expr: &Expr, ctx: &C) -> Self::Output {
        for child in expr.children() {
            self.visit(child, ctx);
        }
        Self::Output::default()
    }

    fn visit_variable_ref(&mut self, _expr: &VariableRef, _ctx: &C) -> Self::Output {
        Self::Output::default()
    }

    fn visit_property_access(&mut self, expr: &PropertyAccess, ctx: &C) -> Self::Output {
        self.visit(expr.expr.as_ref(), ctx)
    }

    fn visit_constant(&mut self, _expr: &Constant, _ctx: &C) -> Self::Output {
        Self::Output::default()
    }

    fn visit_func_call(&mut self, expr: &FuncCall, ctx: &C) -> Self::Output {
        for arg in expr.args.iter() {
            self.visit(arg, ctx);
        }
        Self::Output::default()
    }

    fn visit_agg_call(&mut self, expr: &AggCall, ctx: &C) -> Self::Output {
        for arg in expr.args.iter() {
            self.visit(arg, ctx);
        }
        Self::Output::default()
    }

    fn visit_subquery(&mut self, _expr: &Subquery, _ctx: &C) -> Self::Output {
        // TODO: traverse subquery plan/expr when implemented
        Self::Output::default()
    }

    fn visit_has_label(&mut self, expr: &HasLabel, ctx: &C) -> Self::Output {
        self.visit(expr.entity.as_ref(), ctx)
    }

    fn visit_create_struct(&mut self, expr: &CreateStruct, ctx: &C) -> Self::Output {
        for (_, value) in expr.properties.iter() {
            self.visit(value, ctx);
        }
        Self::Output::default()
    }

    fn visit_create_list(&mut self, expr: &CreateList, ctx: &C) -> Self::Output {
        for elem in expr.elements.iter() {
            self.visit(elem, ctx);
        }
        Self::Output::default()
    }

    fn visit_project_path(&mut self, _expr: &ProjectPath, _ctx: &C) -> Self::Output {
        Self::Output::default()
    }
}

/// Transforming traversal over expressions.
pub trait ExprRewriter {
    fn rewrite(&mut self, expr: Expr) -> Expr {
        self.dispatch(expr)
    }

    fn dispatch(&mut self, expr: Expr) -> Expr {
        match expr {
            Expr::VariableRef(e) => self.rewrite_variable_ref(e),
            Expr::PropertyAccess(e) => self.rewrite_property_access(e),
            Expr::Constant(e) => self.rewrite_constant(e),
            Expr::FuncCall(e) => self.rewrite_func_call(e),
            Expr::AggCall(e) => self.rewrite_agg_call(e),
            Expr::Subquery(e) => self.rewrite_subquery(e),
            Expr::HasLabel(e) => self.rewrite_has_label(e),
            Expr::CreateStruct(e) => self.rewrite_create_struct(e),
            Expr::CreateList(e) => self.rewrite_create_list(e),
            Expr::ProjectPath(e) => self.rewrite_project_path(e),
        }
    }

    fn rewrite_children(&mut self, expr: Expr) -> Expr {
        expr.map_children(|child| self.rewrite(child))
    }

    fn rewrite_variable_ref(&mut self, expr: VariableRef) -> Expr {
        Expr::VariableRef(expr)
    }

    fn rewrite_property_access(&mut self, expr: PropertyAccess) -> Expr {
        self.rewrite_children(Expr::PropertyAccess(expr))
    }

    fn rewrite_constant(&mut self, expr: Constant) -> Expr {
        Expr::Constant(expr)
    }

    fn rewrite_func_call(&mut self, expr: FuncCall) -> Expr {
        self.rewrite_children(Expr::FuncCall(expr))
    }

    fn rewrite_agg_call(&mut self, expr: AggCall) -> Expr {
        self.rewrite_children(Expr::AggCall(expr))
    }

    fn rewrite_subquery(&mut self, expr: Subquery) -> Expr {
        Expr::Subquery(expr)
    }

    fn rewrite_has_label(&mut self, expr: HasLabel) -> Expr {
        self.rewrite_children(Expr::HasLabel(expr))
    }

    fn rewrite_create_struct(&mut self, expr: CreateStruct) -> Expr {
        self.rewrite_children(Expr::CreateStruct(expr))
    }

    fn rewrite_create_list(&mut self, expr: CreateList) -> Expr {
        self.rewrite_children(Expr::CreateList(expr))
    }

    fn rewrite_project_path(&mut self, expr: ProjectPath) -> Expr {
        Expr::ProjectPath(expr)
    }
}
