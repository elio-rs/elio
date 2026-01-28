//! Physical expressions
pub mod agg;
pub mod error;
pub mod impl_;
pub mod scalar;

// the following function should be defined in cypher
// compile an planner expr to executable expr
// pub fn compile_expr(expr: Expr) -> Result<ExprImpl, EvalError> {
//     todo!()
// }
