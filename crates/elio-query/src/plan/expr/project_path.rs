use elio_common::SemanticDirection;
use elio_common::data_type::LogicalType;
use elio_common::schema::Variable;
use elio_common::variable::VariableName;
use indexmap::IndexSet;

use crate::plan::expr::{Expr, ExprNode, VariableRef};

// output virtual path only
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProjectPath {
    // pub name: VariableName,
    pub steps: Vec<PathStep>,
}

impl ProjectPath {
    pub fn new(steps: Vec<PathStep>) -> Self {
        Self { steps }
    }

    pub fn step_variables(&self) -> Vec<VariableRef> {
        self.steps.iter().flat_map(|step| step.as_variable_ref()).collect()
    }
}

// TODO(pgao): seems we do not need path steps here. just use variable name directly should be ok
#[derive(Debug, Clone, PartialEq, Eq, Hash, derive_more::Display)]
pub enum PathStep {
    #[display("({})", _0)]
    NodeStep(VariableName),
    #[display("{ldir}[{rel}]{rdir}({other})", ldir = direction.l_arrow(), rdir = direction.r_arrow())]
    SingleRelStep {
        rel: VariableName,
        direction: SemanticDirection,
        other: VariableName,
    },
    #[display("{ldir}[{rel}*]{rdir}({other})", ldir = direction.l_arrow(), rdir = direction.r_arrow())]
    MutliRelStep {
        rel: VariableName,
        direction: SemanticDirection,
        other: VariableName,
    },
}

impl PathStep {
    pub fn used_variable(&self) -> IndexSet<Variable> {
        let mut set = IndexSet::new();
        match self {
            Self::NodeStep(var) => {
                set.insert(Variable::new(var, &LogicalType::VIRTUAL_NODE));
            }
            Self::SingleRelStep { rel, other, .. } => {
                set.insert(Variable::new(rel, &LogicalType::VIRTUAL_REL));
                set.insert(Variable::new(other, &LogicalType::VIRTUAL_NODE));
            }
            Self::MutliRelStep { rel, other, .. } => {
                set.insert(Variable::new(rel, &LogicalType::new_list(LogicalType::REL)));
                set.insert(Variable::new(other, &LogicalType::VIRTUAL_NODE));
            }
        }
        set
    }

    pub fn as_variable_ref(&self) -> Vec<VariableRef> {
        match self {
            PathStep::NodeStep(var) => vec![VariableRef::new_unchecked(var.clone(), LogicalType::VIRTUAL_NODE)],
            PathStep::SingleRelStep { rel, other, .. } => {
                vec![
                    VariableRef::new_unchecked(rel.clone(), LogicalType::VIRTUAL_REL),
                    VariableRef::new_unchecked(other.clone(), LogicalType::VIRTUAL_NODE),
                ]
            }
            PathStep::MutliRelStep { rel, other, .. } => {
                vec![
                    VariableRef::new_unchecked(rel.clone(), LogicalType::new_list(LogicalType::REL)),
                    VariableRef::new_unchecked(other.clone(), LogicalType::VIRTUAL_NODE),
                ]
            }
        }
    }
}

impl ExprNode for ProjectPath {
    fn typ(&self) -> LogicalType {
        LogicalType::VIRTUAL_PATH
    }
}

impl ProjectPath {
    pub fn used_variable(&self) -> IndexSet<Variable> {
        let mut set = IndexSet::new();
        for step in &self.steps {
            set.extend(step.used_variable());
        }
        set
    }

    pub fn pretty(&self) -> String {
        self.steps
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
            .join("")
            .to_string()
    }
}

impl From<ProjectPath> for Expr {
    fn from(val: ProjectPath) -> Self {
        Expr::ProjectPath(val)
    }
}
