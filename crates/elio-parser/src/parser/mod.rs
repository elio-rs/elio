peg::parser! {
  pub grammar cypher_parser() for str {
    use crate::ast::*;
    // use either::Either;


    /// ---------------------
    /// Whitespace
    /// ---------------------
    rule _() = [' ' | '\t' | '\r' | '\n']+


    /// ---------------------
    /// Statement
    /// ---------------------
    pub rule statement() -> Statement
        = _? s:explain_stmt() _? (";" _?)? { s }
        / _? s:create_constraint_stmt() _? (";" _?)? { s }
        / _? s:drop_constraint_stmt() _? (";" _?)? { s }
        / _? s:regular_query() _? (";" _?)? { s }

    /// EXPLAIN query
    rule explain_stmt() -> Statement
        = EXPLAIN() _ query:regular_query_inner() {
            Statement::Explain(Box::new(Explain { query }))
        }

    /// CREATE CONSTRAINT constraint_name [IF NOT EXISTS]
    /// FOR (var:Label) REQUIRE prop IS UNIQUE
    rule create_constraint_stmt() -> Statement
        = CREATE() _ CONSTRAINT() _ name:ident()
          not_exists:(_ IF() _ NOT() _ EXISTS())?
          _ FOR() _ entity:constraint_entity()
          _ REQUIRE() _ constraint:constraint_type() {
            Statement::CreateConstraint(Box::new(CreateConstraint {
                name: name.to_string(),
                if_not_exists: not_exists.is_some(),
                entity,
                constraint_type: constraint,
            }))
        }

    /// (var:Label) or ()-[var:REL_TYPE]-()
    rule constraint_entity() -> ConstraintEntity
        = "(" _? var:ident() _? ":" _? label:ident() _? ")" {
            ConstraintEntity::Node {
                variable: var.to_string(),
                label: label.to_string(),
            }
        }
        / "(" _? ")" _? "-" _? "[" _? var:ident() _? ":" _? rel_type:ident() _? "]" _? "-" _? "(" _? ")" {
            ConstraintEntity::Relationship {
                variable: var.to_string(),
                rel_type: rel_type.to_string(),
            }
        }

    /// IS UNIQUE | IS NODE KEY | IS NOT NULL
    rule constraint_type() -> ConstraintType
        = props:property_ref_list() _ IS() _ NODE() _ KEY() {
            ConstraintType::NodeKey { properties: props }
        }
        / props:property_ref_list() _ IS() _ UNIQUE() {
            ConstraintType::Unique { properties: props }
        }
        / prop:property_ref() _ IS() _ NOT() _ NULL() {
            ConstraintType::NotNull { property: prop }
        }

    /// (p.a, p.b) or p.a
    rule property_ref_list() -> Vec<PropertyRef>
        = "(" _? props:(property_ref() ++ comma_separator()) _? ")" { props }
        / prop:property_ref() { vec![prop] }

    /// var.property
    rule property_ref() -> PropertyRef
        = var:ident() "." prop:ident() {
            PropertyRef {
                variable: var.to_string(),
                property: prop.to_string(),
            }
        }

    /// DROP CONSTRAINT constraint_name [IF EXISTS]
    rule drop_constraint_stmt() -> Statement
        = DROP() _ CONSTRAINT() _ name:ident() if_exists:(_ IF() _ EXISTS())? {
            Statement::DropConstraint(Box::new(DropConstraint {
                name: name.to_string(),
                if_exists: if_exists.is_some(),
            }))
        }

    /// create database statement
    // pub rule create_database() -> Statement
    //     = CREATE() _ DATABASE() _ not_exists:if_not_exists() _ db_name:ident() _ options:with_attribute_list()? {
    //         Statement::CreateDatabase(Box::new(CreateDatabase {
    //             db_name: db_name.to_string(),
    //             not_exists: false,
    //             options: options.unwrap_or_default(),
    //         }))
    //     }

    /// create vertex type statement
    /// CREATE VERTEX TYPE IF NOT EXISTS name (column1 type1 nullable, column2 type2 nullable, PRIMARY KEY (column1))
    /// WITH (option1: value1, option2: value2)
    // pub rule create_vertex_type() -> Statement
    //     = CREATE() _ VERTEX() _ TYPE() _ not_exists:if_not_exists() _ name:ident() _ "(" _ column_or_constraint:(column_def_or_constraint() ** comma_separator()) _ ")"  _ options:with_attribute_list()? {
    //         let (columns, constraints) = {
    //             let mut columns = vec![];
    //             let mut constraints = vec![];
    //             for item in column_or_constraint {
    //                 match item {
    //                     Either::Left(col) => columns.push(col),
    //                     Either::Right(constraint) => constraints.push(constraint),
    //                 }
    //             }
    //             (columns, constraints)
    //         };

    //         Statement::CreateVertexType(Box::new(CreateVertexType {
    //             name: name.to_string(),
    //             not_exists: false,
    //             columns,
    //             constrait: constraints,
    //             options: options.unwrap_or_default(),
    //         }))
    //     }

    /// create edge type statement
    /// CREATE EDGE TYPE IF NOT EXISTS name (FROM from_vertex_type, TO to_vertex_type, column1 type1 nullable, column2 type2 nullable, PRIMARY KEY (column1))
    /// WITH (option1: value1, option2: value2)
    // pub rule create_edge_type() -> Statement
    //     = CREATE() _ EDGE() _ TYPE() _ not_exists:if_not_exists() _ name:ident() _ "(" _ FROM() _ from:ident() _ "," _ TO() _ to:ident() _ "," _ column_or_constraint:(column_def_or_constraint() ** comma_separator()) _ ")" _  options:with_attribute_list()? {
    //         let (columns, constraints) = {
    //             let mut columns = vec![];
    //             let mut constraints = vec![];
    //             for item in column_or_constraint {
    //                 match item {
    //                     Either::Left(col) => columns.push(col),
    //                     Either::Right(constraint) => constraints.push(constraint),
    //                 }
    //             }
    //             (columns, constraints)
    //         };

    //         Statement::CreateEdgeType(Box::new(CreateEdgeType {
    //             name: name.to_string(),
    //             not_exists: false,
    //             from: from.to_string(),
    //             to: to.to_string(),
    //             columns,
    //             constrait: constraints,
    //             options: options.unwrap_or_default(),
    //         }))
    //     }

    rule if_not_exists() -> bool
        = IF() _ NOT() _ EXISTS() { true }
          / { false }

    rule column_nullable() -> bool
        = _ NOT() _ NULL() { false }
        / _ NULL() { true }


    /// ---------------------
    /// RegularQuery
    /// ---------------------
    rule regular_query() -> Statement
        = query:regular_query_inner() {
            Statement::Query(Box::new(query))
        }

    rule regular_query_inner() -> RegularQuery
        = first:single_query() _ union_:(union_query() ++ _) {
            let mut queries = vec![first];
            let mut union_all = false;
            for (is_all, query) in union_ {
                union_all |= is_all;
                queries.push(query);
            }
            RegularQuery{
                queries,
                union_all,
            }
        }
        / first:single_query() {
            RegularQuery{
                queries: vec![first],
                union_all: false,
            }
    }

    rule single_query() -> SingleQuery
        = clauses:(clause() ++ _) {
            SingleQuery {
                clauses,
            }
        }

    rule union_query() -> (bool, SingleQuery)
        = UNION() _ ALL() _ query:single_query() {
                (true, query)
        }
        / UNION() _ query:single_query() {
            (false, query)
        }

    /// ---------------------
    /// Clauses
    /// ---------------------

    pub rule clause() -> Clause
        = load:load_clause() {
            Clause::Load(load)
        }
        / create:create_clause() {
            Clause::Create(create)
        }
        / match_:match_clause() {
            Clause::Match(match_)
        }
        / with:with_clause() {
            Clause::With(with)
        }
        / return_:return_clause() {
            Clause::Return(return_)
        }
        / unwind:unwind_clause() {
            Clause::Unwind(unwind)
        }


    /// Create Clause
    rule create_clause() -> CreateClause
        = CREATE() _ patterns:pattern() {
            CreateClause {
                pattern: UpdatePattern{ patterns },
            }
        }

    rule match_clause() -> MatchClause
        = optional:optional_match() mode:match_mode()? _ patterns:pattern() where_:where_clause()? {
            MatchClause {
                optional,
                mode: mode.unwrap_or_default(),
                pattern: MatchPattern { patterns },
                where_,
            }
        }

    rule optional_match() -> bool
        = OPTIONAL() _ MATCH() { true }
          / MATCH() { false }

    rule with_clause() -> WithClause
        = WITH() _ return_body:return_body() where_:where_clause()? {
            WithClause {
                distinct: return_body.0,
                return_items: return_body.1,
                order_by: return_body.2,
                skip: return_body.3,
                limit: return_body.4,
                where_,
            }
        }

    rule return_clause() -> ReturnClause
        = RETURN() _ return_body:return_body() {
            ReturnClause {
                distinct: return_body.0,
                return_items: return_body.1,
                order_by: return_body.2,
                skip: return_body.3,
                limit: return_body.4,
            }
        }


    rule unwind_clause() -> UnwindClause
        = UNWIND() _ expr:expr() alias:alias() {
            UnwindClause {
                expr: Box::new(expr),
                variable: alias.to_string(),
            }
        }

    /// LOAD <format> FROM <source> OPTIONS {key: value, ...} AS <alias>
    rule load_clause() -> LoadClause
        = LOAD() _ fmt:ident() _ FROM() _ src:string_literal()
          opts:(_ OPTIONS() _ "{" _? opts:load_options() _? "}" { opts })?
          _ AS() _ var:ident() {
            LoadClause {
                format: fmt.to_string(),
                source: src.to_string(),
                options: opts.unwrap_or_default(),
                variable: var.to_string(),
            }
        }

    rule load_options() -> Vec<LoadOption>
        = opts:(load_option() ** comma_separator()) { opts }

    rule load_option() -> LoadOption
        = key:ident() _? ":" _? val:literal_value() {
            LoadOption { key: key.to_string(), value: val }
        }

    rule literal_value() -> Literal
        = b:(TRUE() / FALSE()) { Literal::Boolean(b == "TRUE") }
        / f:float_literal() { Literal::Float(f.to_string()) }
        / i:integer_literal() { Literal::Integer(i.to_string()) }
        / s:string_literal() { Literal::String(s.to_string()) }

    /// ---------------------
    /// Pattern
    /// ---------------------
    pub rule pattern() -> Vec<PatternPart>
        = parts:(pattern_part() ** comma_separator()) { parts }

    pub rule pattern_part() -> PatternPart
        = variable:variable_declare()? selector:selector()? _? factors:anonymous_pattern() {
            PatternPart{
                variable,
                selector: selector.unwrap_or_default(),
                factors
            }
        }

    pub(crate) rule match_mode() -> MatchMode
        = _ WALK() { MatchMode::WALK }
        / _ TRAIL() { MatchMode::TRAIL }

    pub(crate) rule selector() -> Selector
        = _ ALL() _ PATH_OR_PATHS() {
            Selector::AllPaths
        }
        / _ ANY() _ count:integer_literal() _ PATH_OR_PATHS() {
            let count: u32 = count.parse().unwrap();
            Selector::AnyPath(count)
        }
        / _ ALL() _ SHORTEST() _ PATH_OR_PATHS() {
            Selector::AllShortest
        }
        / _ ANY() _ SHORTEST() _ PATH_OR_PATHS() {
            Selector::AnyShortestPath
        }
        / _ SHORTEST() _ count:integer_literal() _ PATH_OR_PATHS() _ GROUP_OR_GROUPS() {
            let count: u32 = count.parse().unwrap();
            Selector::CountedShortestGroup(count)
        }
        / _ SHORTEST() _ count:integer_literal() _ PATH_OR_PATHS() {
            let count: u32 = count.parse().unwrap();
            Selector::CountedShortestPath(count)
        }

    pub rule anonymous_pattern() -> Vec<PathFactor>
        = head:simple_path_pattern() _ tail:(path_facror() ++ (_?)) {
            let mut parts = vec![];
            parts.push(PathFactor::Simple(head));
            parts.extend(tail);
            parts
        }
        / head:simple_path_pattern() {
            vec![PathFactor::Simple(head)]
        }

    rule path_facror() -> PathFactor
        = simple:simple_path_pattern() {
            PathFactor::Simple(simple)
        }
        / quantified:quantified_path_pattern() {
            PathFactor::Quantified(quantified)
        }

    rule simple_path_pattern() -> SimplePathPattern
        = node:node_pattern() chain:(pattern_element_chain()*) {
            let mut nodes = vec![];
            let mut relationships = vec![];
            nodes.push(node);
            for (rel, node) in chain {
                relationships.push(rel);
                nodes.push(node);
            }
            SimplePathPattern {
                nodes,
                relationships,
            }
        }

    pub rule quantified_path_pattern() -> QuantifiedPathPattern
        = "(" _? pattern:pattern_part() filter:where_clause()? _? ")" _? quantifier:quantifier() {
            QuantifiedPathPattern{
                non_selective_part: Box::new(pattern),
                quantifier,
                filter,
            }
         }

    rule pattern_element_chain() -> (RelationshipPattern, NodePattern)
        = rel:relationship_pattern() node:node_pattern() {
            (rel, node)
        }

    rule node_pattern() -> NodePattern
        // TODO(pgao): support Where expr in node pattern
        = "(" _? variable:ident()? _? label_expr:label_expr()? _? properties:map_expr()? _? ")" {
            NodePattern {
                variable: variable.map(|v| v.to_string()),
                label_expr,
                properties,
                predicate: None,
            }
        }

    rule relationship_pattern() -> RelationshipPattern
        = LEFT_ARROW() ARROW_LINE() {
            let mut relationship = RelationshipPattern::new();
            relationship.direction = SemanticDirection::Incoming;
            relationship
        }
        / ARROW_LINE() RIGHT_ARROW() {
            let mut relationship = RelationshipPattern::new();
            relationship.direction = SemanticDirection::Outgoing;
            relationship
        }
        / ARROW_LINE() ARROW_LINE() {
            let mut relationship = RelationshipPattern::new();
            relationship.direction = SemanticDirection::Both;
            relationship
        }
        / LEFT_ARROW() relationship:relationship_detail() ARROW_LINE() {
            let mut relationship = relationship;
            relationship.direction = SemanticDirection::Incoming;
            relationship
        }
        / ARROW_LINE() relationship:relationship_detail() RIGHT_ARROW() {
            let mut relationship = relationship;
            relationship.direction = SemanticDirection::Outgoing;
            relationship
        }
        / ARROW_LINE() relationship:relationship_detail() ARROW_LINE() {
            let mut relationship = relationship;
            relationship.direction = SemanticDirection::Both;
            relationship
        }

    rule relationship_detail() -> RelationshipPattern
        = "[" _? variable:ident()? _? label_expr:label_expr()? _? length:range_literal()? _? properties:map_expr()? _? "]" {
            RelationshipPattern{
                variable: variable.map(|v| v.to_string()),
                label_expr,
                properties,
                length,
                ..Default::default()
            }
        }


    rule range_literal() -> Option<std::ops::Range<usize>>
        = "*" _? lower:integer_literal()? ".." upper:integer_literal()? {
            // NOTE: if lower is None, then lower is 1
            let lower: usize = lower.map(|x| x.parse().unwrap()).unwrap_or(1);
            let upper: usize = upper.map(|x| x.parse().unwrap()).unwrap_or(usize::MAX);
            Some(lower..upper)
        }
        / "*" _? exact:integer_literal() {
            let count: usize = exact.parse().unwrap();
            Some(count..count)
        }
        / "*" _? {
            None
        }

    rule quantifier() -> PatternQuantifier
        = "+" { PatternQuantifier::Plus}
        / "*" { PatternQuantifier::Star}
        / "{" _? count:integer_literal() _? "}" {
            let count: u32 = count.parse().unwrap();
            PatternQuantifier::Fixed(count)
        }
        / "{" _? lower:integer_literal()? _? "," _? upper:integer_literal()? _? "}" {
            let lower: Option<u32> = lower.map(|v| v.parse().unwrap());
            let upper: Option<u32> = upper.map(|v| v.parse().unwrap());
            PatternQuantifier::Interval{ lower, upper}
        }

    rule variable_declare() -> String
         = variable:ident() _? "=" {
            variable.to_string()
         }

    rule where_clause() -> Box<Expr>
         = _ WHERE() _ expr:expr() {
            Box::new(expr)
         }

    /// ---------------------
    /// Return Body
    /// ---------------------

    // (distinct, return_items, orderby, skip, limit)
    rule return_body() -> (bool, ReturnItems, Option<OrderBy>, Option<Box<Expr>>, Option<Box<Expr>>)
         = distinct:distinct()? return_items:return_items() orderby:order_by()? skip:skip()? limit:limit()? {
            (
                distinct.unwrap_or(false),
                return_items,
                orderby,
                skip,
                limit,
            )
        }

    pub rule return_items() -> ReturnItems
        = "*" _? "," _? items:(return_item() ++ comma_separator()) {
            ReturnItems {
                projection_kind: ProjectionKind::Additive,
                items,
            }
        }
        / "*" {
            ReturnItems {
                projection_kind: ProjectionKind::Additive,
                items: vec![],
            }
        }
        / items:(return_item() ++ comma_separator()) {
            ReturnItems {
                projection_kind: ProjectionKind::Free,
                items,
            }
        }

    rule distinct() -> bool
        = DISTINCT() _ { true }
        / ALL() _ { false }

    pub rule return_item() -> ReturnItem
        = expr:expr() alias:alias()? {
            ReturnItem {
                expr: Box::new(expr),
                alias,
            }
        }

    rule alias() -> String
        = _ AS() _ alias:variable() {
            alias.to_string()
        }


    /// ---------------------
    /// Order By
    /// ---------------------

    rule order_by() -> OrderBy
        = _ ORDER() _ BY() _ items:(sort_item() ++ comma_separator()) {
            OrderBy {
                items,
            }
        }

    rule sort_item() -> SortItem
        = expr:expr() direction:(sort_direction())? {
            SortItem {
                expr: Box::new(expr),
                direction: direction.unwrap_or_default(),
            }
        }

    rule sort_direction() -> SortDirection
        = _ ASCENDING() { SortDirection::Asc }
        / _ ASC() { SortDirection::Asc }
        / _ DESCENDING() { SortDirection::Desc }
        / _ DESC() { SortDirection::Desc }


    /// ---------------------
    /// Skip / Limit
    /// ---------------------

    rule skip() -> Box<Expr>
        = _ SKIP() _ expr:expr() {
            Box::new(expr)
        }

    rule limit() -> Box<Expr>
        = _ LIMIT() _ expr:expr() {
            Box::new(expr)
        }

    /// ---------------------
    /// Expression
    /// ---------------------
    pub rule expr() -> Expr
        = precedence! {
            left:(@) _ OR() _ right:@ {Expr::new_binary(left, BinaryOperator::Or, right)}
            --
            left:(@) _ XOR() _ right:@ {Expr::new_binary(left, BinaryOperator::Xor, right)}
            --
            left:(@) _ AND() _ right:@ {Expr::new_binary(left, BinaryOperator::And, right)}
            --
                    _? NOT() _ right:@ {Expr::new_unary(UnaryOperator::Not, right)}
            --
            left:(@) _? op:$("=" / "!=" / "<>" / "<=" / "<" / ">=" / ">") _? right:@ {
                let operator = match op {
                    "=" => BinaryOperator::Eq,
                    "!="  | "<>" => BinaryOperator::NotEq,
                    "<" => BinaryOperator::Lt,
                    "<=" => BinaryOperator::LtEq,
                    ">" => BinaryOperator::Gt,
                    ">=" => BinaryOperator::GtEq,
                    _ => unreachable!(),
                };
                Expr::new_binary(left, operator, right)
            }
            --
            left:(@) _ op:null_predicate() {
                Expr::new_unary(op, left)
            }
            --
            left:(@) _? op:$("+" / "-" / "||" ) _? right:@ {
                let operator = match op {
                    "+" => BinaryOperator::Add,
                    "-" => BinaryOperator::Subtract,
                    "||" => BinaryOperator::Concat,
                    _ => unreachable!(),
                };
                Expr::new_binary(left, operator, right)
            }
            --
            left:(@) _? op:$("*" / "/" / "%") _? right:@ {
                let operator = match op {
                    "*" => BinaryOperator::Multiply,
                    "/" => BinaryOperator::Divide,
                    "%" => BinaryOperator::Modulo,
                    _ => unreachable!(),
                };
                Expr::new_binary(left, operator, right)
            }
            --
            left:(@) _? op:$("^") _? right:@ {
                Expr::new_binary(left, BinaryOperator::Pow, right)
            }
            --
            op:$("+" / "-") _? right:@ {
                let operator = match op {
                    "+" => UnaryOperator::UnaryAdd,
                    "-" => UnaryOperator::UnarySubtract,
                    _ => unreachable!(),
                };
                Expr::new_unary(operator, right)
            }
            --
            // postfix
            // property_access
            left:(@) "." key:ident() { Expr::new_property_access(left, key.to_string()) }
            // list_slice
            left:(@) "[" _? start:expr()? _? ".." _? end:expr()? _? "]" {
                Expr::new_list_slice(left, start, end)
            }
            // list_index
            left:(@) "[" _? index:expr() _? "]" {
                Expr::new_list_index(left, index)
            }
            --
            // atom
            a:atom() { a }
        }

    rule atom() -> Expr
        = l:literal() { l }
        / "$" v:ident() { Expr::new_parameter(v.to_string()) }
        / "(" _? e:expr() _? ")" { e }
        / m:map_expr() { m }
        / l:list_expr() { l }
        / f:function_call() { f }
        / v:variable() { v }

    rule literal() -> Expr
        = b:(TRUE() / FALSE()) { Expr::new_boolean(b == "TRUE") }
        / f:float_literal() { Expr::new_float(f.to_string()) }
        / i:integer_literal() { Expr::new_integer(i.to_string()) }
        / s:string_literal() { Expr::new_string(s.to_string()) }
        / n:null_literal() { n }


    rule function_call() -> Expr
        // for function like `count(*)`
        = name:ident() _? "(" _? "*" _? ")" {
            Expr::new_function_call(name.to_string(), false, vec![])
        }
        / name:ident() _? "(" _? distinct:distinct()? _? args:(expr() ** comma_separator()) _? ")" {
            Expr::new_function_call(name.to_string(), distinct.unwrap_or(false), args)
        }

    rule variable() -> Expr
        = v:ident() { Expr::new_variable(v.to_string()) }

    rule null_predicate() -> UnaryOperator
        = is_null() / is_not_null()
    rule is_null() -> UnaryOperator
        = IS() _ NULL() { UnaryOperator::IsNull }
    rule is_not_null() -> UnaryOperator
        = IS() _ NOT() _ NULL() { UnaryOperator::IsNotNull }

    rule integer_literal() -> &'input str
        = $(number())
    rule float_literal() -> &'input str
        = $(number() "." number())
    rule string_literal() -> &'input str
        = "'" content:$((!['\''] [_])* ) "'" { content }
        / "\"" content:$((!['\"'] [_])* ) "\"" { content }
    rule null_literal() -> Expr
        = NULL() { Expr::new_null() }

    rule map_expr() -> Expr
        = "{" _? items:(map_expr_item() ** comma_separator()) _? "}" {
            let (keys, values) = items.into_iter().unzip();
            Expr::new_map_expression(keys, values)
        }
    rule map_expr_item() -> (String, Expr)
        = key:ident() _? ":" _? value:expr() { (key.to_string(), value) }

    rule list_expr() -> Expr
        = "[" _? items:(expr() ** comma_separator()) _? "]" {
            Expr::new_list_expression(items)
        }

    /// ---------------------
    /// Label Expression
    /// ---------------------
    pub rule label_expr() -> LabelExpr
        = ":" _? expr:label_expr_inner() {
            expr
        }

    rule label_expr_inner() -> LabelExpr
        = precedence! {
            left:(@) _? op:$("|") _? right:@ { LabelExpr::new_or(left, right)}
            --
            left:(@) _? op:$("&") _? right:@ { LabelExpr::new_and(left, right)}
            label:ident() { LabelExpr::new_label(label.to_string()) }
        }


    /// ---------------------
    /// Data Types
    /// ---------------------
    rule data_type() -> DataType
        = INTEGER() { DataType::Integer }
        / FLOAT() { DataType::Float }
        / STRING() { DataType::String }
        / BOOLEAN() { DataType::Boolean }



    /// ---------------------
    /// Common
    /// ---------------------
    rule number() -> &'input str
        = first:$(['0'..='9']+) { first }

    rule ident() -> &'input str
        = escaped_ident()
        / $(['a'..='z' | 'A'..='Z' | '_'] ['a'..='z' | 'A'..='Z' | '0'..='9' | '_']*)

    rule escaped_ident() -> &'input str
        = "`" content:$((!['`'] [_])*) "`" { content }

    rule comma_separator() = _? "," _?

    /// ---------------------
    /// Key Words
    /// ---------------------
    rule AS() -> &'static str
        = ['a' | 'A'] ['s' | 'S'] { "AS" }
    rule IS() -> &'static str
        = ['i' | 'I'] ['s' | 'S'] { "IS" }
    rule NULL() -> &'static str
        = ['n' | 'N'] ['u' | 'U'] ['l' | 'L'] ['l' | 'L'] { "NULL" }
    rule TRUE() -> &'static str
        = ['t' | 'T'] ['r' | 'R'] ['u' | 'U'] ['e' | 'E'] { "TRUE" }
    rule FALSE() -> &'static str
        = ['f' | 'F'] ['a' | 'A'] ['l' | 'L'] ['s' | 'S'] ['e' | 'E'] { "FALSE" }
    rule CREATE() -> &'static str
        = ['c' | 'C'] ['r' | 'R'] ['e' | 'E'] ['a' | 'A'] ['t' | 'T'] ['e' | 'E'] { "CREATE" }
    rule DATABASE() -> &'static str
        = ['d' | 'D'] ['a' | 'A'] ['t' | 'T'] ['a' | 'A'] ['b' | 'B'] ['a' | 'A'] ['s' | 'S'] ['e' | 'E'] { "DATABASE" }
    rule VERTEX() -> &'static str
        = ['v' | 'V'] ['e' | 'E'] ['r' | 'R'] ['t' | 'T'] ['e' | 'E'] ['x' | 'X'] { "VERTEX" }
    rule TYPE() -> &'static str
        = ['t' | 'T'] ['y' | 'Y'] ['p' | 'P'] ['e' | 'E'] { "TYPE" }
    rule EDGE() -> &'static str
        = ['e' | 'E'] ['d' | 'D'] ['g' | 'G'] ['e' | 'E'] { "EDGE" }
    rule WITH() -> &'static str
        = ['w' | 'W'] ['i' | 'I'] ['t' | 'T'] ['h' | 'H'] { "WITH" }
    rule IF() -> &'static str
        = ['i' | 'I'] ['f' | 'F'] { "IF" }
    rule NOT() -> &'static str
        = ['n' | 'N'] ['o' | 'O'] ['t' | 'T'] { "NOT" }
    rule EXISTS() -> &'static str
        = ['e' | 'E'] ['x' | 'X'] ['i' | 'I'] ['s' | 'S'] ['t' | 'T'] ['s' | 'S'] { "EXISTS" }
    rule UNION() -> &'static str
        = ['u' | 'U'] ['n' | 'N'] ['i' | 'I'] ['o' | 'O'] ['n' | 'N'] { "UNION" }
    rule ALL() -> &'static str
        = ['a' | 'A'] ['l' | 'L'] ['l' | 'L'] { "ALL" }
    rule ANY() -> &'static str
        = ['a' | 'A'] ['n' | 'N'] ['y' | 'Y'] { "ANY" }
    rule SHORTEST() -> &'static str
        = ['s' | 'S'] ['h' | 'H'] ['o' | 'O'] ['r' | 'R'] ['t' | 'T'] ['e' | 'E'] ['s' | 'S'] ['t' | 'T'] { "SHORTEST" }
    rule PATH() -> &'static str
        = ['p' | 'P'] ['a' | 'A'] ['t' | 'T'] ['h' | 'H'] { "PATH" }
    rule PATHS() -> &'static str
        = ['p' | 'P'] ['a' | 'A'] ['t' | 'T'] ['h' | 'H'] ['s' | 'S'] { "PATHS" }
    rule PATH_OR_PATHS() -> &'static str
        = PATHS() / PATH()
    rule GROUP() -> &'static str
        = ['g' | 'G'] ['r' | 'R'] ['o' | 'O'] ['u' | 'U'] ['p' | 'P'] { "GROUP" }
    rule GROUPS() -> &'static str
        = ['g' | 'G'] ['r' | 'R'] ['o' | 'O'] ['u' | 'U'] ['p' | 'P'] ['s' | 'S'] { "GROUPS" }
    rule GROUP_OR_GROUPS() -> &'static str
        = GROUPS() / GROUP()
    rule WHERE() -> &'static str
        = ['w' | 'W'] ['h' | 'H'] ['e' | 'E'] ['r' | 'R'] ['e' | 'E'] { "WHERE" }
    rule OPTIONAL() -> &'static str
        = ['o' | 'O'] ['p' | 'P'] ['t' | 'T'] ['i' | 'I'] ['o' | 'O'] ['n' | 'N'] { "OPTIONAL" }
    rule MATCH() -> &'static str
        = ['m' | 'M'] ['a' | 'A'] ['t' | 'T'] ['c' | 'C'] ['h' | 'H'] { "MATCH" }
    rule WALK() -> &'static str
        = ['w' | 'W'] ['a' | 'A'] ['l' | 'L'] ['k' | 'K'] { "WALK" }
    rule TRAIL() -> &'static str
        = ['t' | 'T'] ['r' | 'R'] ['a' | 'A'] ['i' | 'I'] ['l' | 'L'] { "TRAIL" }
    rule DISTINCT() -> &'static str
        = ['d' | 'D'] ['i' | 'I'] ['s' | 'S'] ['t' | 'T'] ['i' | 'I'] ['n' | 'N'] ['c' | 'C'] ['t' | 'T'] { "DISTINCT" }
    rule ORDER() -> &'static str
        = ['o' | 'O'] ['r' | 'R'] ['d' | 'D'] ['e' | 'E'] ['r' | 'R'] { "ORDER" }
    rule BY() -> &'static str
        = ['b' | 'B'] ['y' | 'Y'] { "BY" }
    rule ASC() -> &'static str
        = ['a' | 'A'] ['s' | 'S'] ['c' | 'C'] { "ASC" }
    rule DESC() -> &'static str
        = ['d' | 'D'] ['e' | 'E'] ['s' | 'S'] ['c' | 'C'] { "DESC" }
    rule ASCENDING() -> &'static str
        = ['a' | 'A'] ['s' | 'S'] ['c' | 'C'] ['e' | 'E'] ['n' | 'N'] ['d' | 'D'] ['i' | 'I'] ['n' | 'N'] { "ASCENDING" }
    rule DESCENDING() -> &'static str
        = ['d' | 'D'] ['e' | 'E'] ['s' | 'S'] ['c' | 'C'] ['e' | 'E'] ['n' | 'N'] ['d' | 'D'] ['i' | 'I'] ['n' | 'N'] { "DESCENDING" }
    rule SKIP() -> &'static str
        = ['s' | 'S'] ['k' | 'K'] ['i' | 'I'] ['p' | 'P'] { "SKIP" }
    rule LIMIT() -> &'static str
        = ['l' | 'L'] ['i' | 'I'] ['m' | 'M'] ['i' | 'I'] ['t' | 'T'] { "LIMIT" }
    rule RETURN() -> &'static str
        = ['r' | 'R'] ['e' | 'E'] ['t' | 'T'] ['u' | 'U'] ['r' | 'R'] ['n' | 'N'] { "RETURN" }
    rule UNWIND() -> &'static str
        = ['u' | 'U'] ['n' | 'N'] ['w' | 'W'] ['i' | 'I'] ['n' | 'N'] ['d' | 'D'] { "UNWIND" }
    rule LOAD() -> &'static str
        = ['l' | 'L'] ['o' | 'O'] ['a' | 'A'] ['d' | 'D'] { "LOAD" }
    rule OPTIONS() -> &'static str
        = ['o' | 'O'] ['p' | 'P'] ['t' | 'T'] ['i' | 'I'] ['o' | 'O'] ['n' | 'N'] ['s' | 'S'] { "OPTIONS" }

    rule INTEGER() -> &'static str
        = ['i' | 'I'] ['n' | 'N'] ['t' | 'T'] ['e' | 'E'] ['g' | 'G'] ['e' | 'E'] ['r' | 'R'] { "INTEGER" }
    rule FLOAT() -> &'static str
        = ['f' | 'F'] ['l' | 'L'] ['o' | 'O'] ['a' | 'A'] ['t' | 'T'] { "FLOAT" }
    rule STRING() -> &'static str
        = ['s' | 'S'] ['t' | 'T'] ['r' | 'R'] ['i' | 'I'] ['n' | 'N'] ['g' | 'G'] { "STRING" }
    rule BOOLEAN() -> &'static str
        = ['b' | 'B'] ['o' | 'O'] ['o' | 'O'] ['l' | 'L'] ['e' | 'E'] ['a' | 'A'] ['n' | 'N'] { "BOOLEAN" }

    rule PRIMARY() -> &'static str
        = ['p' | 'P'] ['r' | 'R'] ['i' | 'I'] ['m' | 'M'] ['a' | 'A'] ['r' | 'R'] ['y' | 'Y'] { "PRIMARY" }
    rule KEY() -> &'static str
        = ['k' | 'K'] ['e' | 'E'] ['y' | 'Y'] { "KEY" }

    rule FROM() -> &'static str
        = ['f' | 'F'] ['r' | 'R'] ['o' | 'O'] ['m' | 'M'] { "FROM" }
    rule TO() -> &'static str
        = ['t' | 'T'] ['o' | 'O'] { "TO" }

    // Constraint keywords
    rule CONSTRAINT() -> &'static str
        = ['c' | 'C'] ['o' | 'O'] ['n' | 'N'] ['s' | 'S'] ['t' | 'T'] ['r' | 'R']
          ['a' | 'A'] ['i' | 'I'] ['n' | 'N'] ['t' | 'T'] { "CONSTRAINT" }
    rule FOR() -> &'static str
        = ['f' | 'F'] ['o' | 'O'] ['r' | 'R'] { "FOR" }
    rule REQUIRE() -> &'static str
        = ['r' | 'R'] ['e' | 'E'] ['q' | 'Q'] ['u' | 'U'] ['i' | 'I'] ['r' | 'R'] ['e' | 'E'] { "REQUIRE" }
    rule UNIQUE() -> &'static str
        = ['u' | 'U'] ['n' | 'N'] ['i' | 'I'] ['q' | 'Q'] ['u' | 'U'] ['e' | 'E'] { "UNIQUE" }
    rule NODE() -> &'static str
        = ['n' | 'N'] ['o' | 'O'] ['d' | 'D'] ['e' | 'E'] { "NODE" }
    rule DROP() -> &'static str
        = ['d' | 'D'] ['r' | 'R'] ['o' | 'O'] ['p' | 'P'] { "DROP" }
    rule EXPLAIN() -> &'static str
        = ['e' | 'E'] ['x' | 'X'] ['p' | 'P'] ['l' | 'L'] ['a' | 'A'] ['i' | 'I'] ['n' | 'N'] { "EXPLAIN" }

    // operator
    rule OR() -> &'static str
        = ['o' | 'O'] ['r' | 'R'] { "OR" }
    rule XOR() -> &'static str
        = ['x' | 'X'] ['o' | 'O'] ['r' | 'R'] { "XOR" }
    rule AND() -> &'static str
        = ['a' | 'A'] ['n' | 'N'] ['d' | 'D'] { "AND" }

    // pattern
    rule ARROW_LINE() -> &'static str
        = "-" { "-" }
    rule LEFT_ARROW() -> &'static str
        = "<-" { "<-" }
    rule RIGHT_ARROW() -> &'static str
        = "->" { "->" }
  }
}
