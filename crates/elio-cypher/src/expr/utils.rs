use elio_common::variable::VariableName;
use indexmap::IndexSet;

pub use super::*;

impl Expr {
    pub fn collect_variables(&self) -> IndexSet<Variable> {
        let mut vars = IndexSet::new();
        match self {
            Expr::VariableRef(variable_ref) => {
                vars.insert(variable_ref.as_variable());
            }
            Expr::PropertyAccess(property_access) => vars.extend(property_access.expr.collect_variables()),
            Expr::Constant(_) => {}
            Expr::FuncCall(func_call) => {
                vars.extend(func_call.args.iter().flat_map(|arg| arg.collect_variables()));
            }
            Expr::AggCall(agg_call) => {
                vars.extend(agg_call.args.iter().flat_map(|arg| arg.collect_variables()));
            }
            Expr::Subquery(_) => todo!("subquery not supported"),
            Expr::HasLabel(HasLabel { entity, .. }) => {
                vars.extend(entity.collect_variables());
            }
            Expr::CreateStruct(CreateStruct { properties, .. }) => {
                vars.extend(properties.iter().flat_map(|(_, expr)| expr.collect_variables()));
            }
            Expr::CreateList(CreateList { elements, .. }) => {
                vars.extend(elements.iter().flat_map(|expr| expr.collect_variables()));
            }
            Expr::ProjectPath(project_path) => vars.extend(project_path.used_variable()),
        }
        vars
    }

    pub fn depend_only_on(&self, vars: &IndexSet<VariableName>) -> bool {
        self.collect_variables()
            .into_iter()
            .map(|x| x.name.clone())
            .collect::<IndexSet<VariableName>>()
            .is_subset(vars)
    }
}
