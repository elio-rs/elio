use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use anyhow::Error;
use async_trait::async_trait;
use elio_catalog::FunctionCatalog;
use elio_catalog::error::CatalogError;
use elio_common::{LabelId, PropertyKeyId, TokenId, TokenKind};
use elio_cypher::plan_context::PlanContext;
use elio_cypher::session::{IndexHint, PlannerSession};
use elio_expr::func::FUNCTION_REGISTRY;
use elio_parser::ast;
use itertools::Itertools;
use sqlplannertest::ParsedTestCase;

#[derive(Hash, Debug, PartialEq, Eq)]
struct TokenKey {
    kind: TokenKind,
    key: String,
}

/// Key for mock index lookup
#[derive(Hash, Debug, PartialEq, Eq, Clone)]
struct MockIndexKey {
    label_id: LabelId,
    property_key_ids: Vec<PropertyKeyId>,
}

#[derive(Debug)]
pub struct MockCatalog {
    functions: HashMap<String, FunctionCatalog>,
    tokens: Mutex<HashMap<TokenKey, TokenId>>,
    /// Mock indexes: (label_id, property_key_ids) -> (constraint_name)
    indexes: Mutex<HashMap<MockIndexKey, String>>,
}

impl Default for MockCatalog {
    fn default() -> Self {
        let functions = FUNCTION_REGISTRY
            .name2def
            .iter()
            .map(|(k, v)| {
                (
                    k.clone(),
                    FunctionCatalog {
                        name: k.clone(),
                        func: v.clone(),
                    },
                )
            })
            .collect();
        Self {
            functions,
            tokens: Default::default(),
            indexes: Default::default(),
        }
    }
}

impl MockCatalog {
    /// Add a mock index for testing
    pub fn add_index(&self, label_id: LabelId, property_key_ids: Vec<PropertyKeyId>, constraint_name: &str) {
        let key = MockIndexKey {
            label_id,
            property_key_ids,
        };
        self.indexes.lock().unwrap().insert(key, constraint_name.to_string());
    }

    /// Find index matching the given label and properties
    pub fn find_index(
        &self,
        label_id: LabelId,
        property_key_ids: &[PropertyKeyId],
    ) -> Option<(String, Vec<PropertyKeyId>)> {
        let indexes = self.indexes.lock().unwrap();
        // Look for an index where all index properties are covered by the query
        for (key, constraint_name) in indexes.iter() {
            if key.label_id == label_id
                && key.property_key_ids.len() <= property_key_ids.len()
                && key.property_key_ids.iter().all(|p| property_key_ids.contains(p))
            {
                return Some((constraint_name.clone(), key.property_key_ids.clone()));
            }
        }
        None
    }
}

#[derive(Debug)]
pub struct MockPlannerSession {
    catalog: Arc<MockCatalog>,
}

impl MockPlannerSession {
    pub fn parse(self: &Arc<Self>, cypher: &str) -> anyhow::Result<ast::Statement> {
        let ast = elio_cypher::session::parse_statement(cypher)?;
        Ok(ast)
    }

    pub fn bind_query(self: &Arc<Self>, ast: &ast::RegularQuery) -> anyhow::Result<String> {
        let ir = elio_cypher::session::bind_query(self.clone(), ast)?;
        Ok(ir.explain())
    }

    pub fn plan_query(self: &Arc<Self>, ast: &ast::RegularQuery) -> anyhow::Result<String> {
        let plan = elio_cypher::session::plan_query(self.clone(), ast)?;
        Ok(plan.explain())
    }

    /// Execute a DDL statement (CREATE CONSTRAINT, DROP CONSTRAINT)
    /// This records the constraint in the mock catalog for index selection
    pub fn execute_ddl(self: &Arc<Self>, stmt: &ast::Statement) -> anyhow::Result<()> {
        match stmt {
            ast::Statement::CreateConstraint(constraint) => {
                // Extract label and properties
                let label_name = match &constraint.entity {
                    ast::ConstraintEntity::Node { label, .. } => label.clone(),
                    ast::ConstraintEntity::Relationship { rel_type, .. } => rel_type.clone(),
                };

                let properties: Vec<String> = match &constraint.constraint_type {
                    ast::ConstraintType::Unique { properties } => {
                        properties.iter().map(|p| p.property.clone()).collect()
                    }
                    ast::ConstraintType::NodeKey { properties } => {
                        properties.iter().map(|p| p.property.clone()).collect()
                    }
                    ast::ConstraintType::NotNull { property } => vec![property.property.clone()],
                };

                // Get or create token IDs
                let label_id = self.get_or_create_token(&label_name, TokenKind::Label)?;
                let prop_key_ids: Vec<PropertyKeyId> = properties
                    .iter()
                    .map(|p| self.get_or_create_token(p, TokenKind::PropertyKey))
                    .collect::<Result<_, _>>()?;

                // Record the index in mock catalog
                self.catalog.add_index(label_id, prop_key_ids, &constraint.name);
                Ok(())
            }
            ast::Statement::DropConstraint(_) => {
                // For mock, we just ignore drop constraint
                Ok(())
            }
            _ => Err(Error::msg("Only DDL statements (CREATE/DROP CONSTRAINT) are supported")),
        }
    }
}

impl PlannerSession for MockPlannerSession {
    fn derive_plan_context(self: Arc<Self>) -> Arc<PlanContext> {
        Arc::new(PlanContext::new(self))
    }

    fn get_or_create_token(&self, token: &str, kind: TokenKind) -> Result<TokenId, CatalogError> {
        let key = TokenKey {
            kind,
            key: token.to_string(),
        };
        let mut tokens = self.catalog.tokens.lock().unwrap();
        let token_id = tokens.len() as TokenId;
        Ok(*tokens.entry(key).or_insert_with(|| token_id))
    }

    fn get_function_by_name(&self, name: &str) -> Option<&FunctionCatalog> {
        self.catalog.functions.get(name)
    }

    fn get_token_id(&self, token: &str, kind: TokenKind) -> Option<TokenId> {
        let key = TokenKey {
            kind,
            key: token.to_string(),
        };
        let tokens = self.catalog.tokens.lock().unwrap();
        tokens.get(&key).cloned()
    }

    fn find_unique_index(&self, label_id: LabelId, property_key_ids: &[PropertyKeyId]) -> Option<IndexHint> {
        // Check mock indexes
        self.catalog
            .find_index(label_id, property_key_ids)
            .map(|(constraint_name, prop_ids)| IndexHint {
                constraint_name: constraint_name.into(),
                label_id,
                property_key_ids: prop_ids,
            })
    }

    fn send_notification(&self, _notification: String) {
        todo!()
    }
}

#[derive(Default)]
pub struct TestEnv {
    catalog: Arc<MockCatalog>,
}

impl TestEnv {
    pub fn new_session(&self) -> MockPlannerSession {
        MockPlannerSession {
            catalog: self.catalog.clone(),
        }
    }

    /// Execute DDL statements (for setting up indexes before planning)
    pub fn task_ddl(&self, cypher: &str) -> anyhow::Result<()> {
        let session = Arc::new(self.new_session());
        let stmts = cypher.trim().split(';').filter(|s| !s.trim().is_empty()).collect_vec();
        for stmt in stmts {
            let ast = session.parse(stmt.trim())?;
            session.execute_ddl(&ast)?;
        }
        Ok(())
    }

    // generate plan
    pub fn task_plan(&self, result: &mut String, cypher: &str, task: &str, _opts: &TaskOption) -> anyhow::Result<()> {
        let session = Arc::new(self.new_session());
        let stmts = cypher.trim().split(';').collect_vec();
        for stmt in stmts {
            let ast = session.parse(stmt)?;
            match ast {
                ast::Statement::Query(regular_query) => {
                    let plan = session.plan_query(&regular_query)?;
                    result.push_str(&plan);
                    result.push('\n');
                }
                _ => return Err(Error::msg(format!("invalid cypher{} and task{}", stmt, task))),
            }
        }
        Ok(())
    }

    pub fn task_bind(&self, result: &mut String, cypher: &str, task: &str, _opts: &TaskOption) -> anyhow::Result<()> {
        let session = Arc::new(self.new_session());
        let stmts = cypher.trim().split(';').collect_vec();
        for stmt in stmts {
            let ast = session.parse(stmt)?;
            match ast {
                ast::Statement::Query(regular_query) => {
                    let ir = session.bind_query(&regular_query)?;
                    result.push_str(&ir);
                    result.push('\n');
                }
                _ => return Err(Error::msg(format!("invalid cypher{} and task{}", stmt, task))),
            }
        }
        Ok(())
    }
}

// TODO(pgao): task options
#[derive(Debug, Default)]
pub struct TaskOption {
    _rules: Vec<String>,
}

/// Available tasks
///   - plan: generate plan from the SQL
///   - bind: generate bind result from the SQL
///   - ddl: execute DDL statements to setup indexes (no output)
#[async_trait]
impl sqlplannertest::PlannerTestRunner for TestEnv {
    /// Run a test case and return the result
    async fn run(&mut self, test_case: &ParsedTestCase) -> anyhow::Result<String> {
        let mut result = String::new();
        for task in test_case.tasks.iter() {
            if task == "ddl" {
                // Execute DDL to setup indexes, no output
                self.task_ddl(&test_case.sql)?;
            } else if task == "plan" {
                self.task_plan(&mut result, &test_case.sql, task, &TaskOption::default())?;
            } else if task == "bind" {
                self.task_bind(&mut result, &test_case.sql, task, &TaskOption::default())?;
            }
        }
        Ok(result)
    }
}
