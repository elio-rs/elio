#![allow(clippy::double_parens)] // this is because EnumAsInner will generate extra parens

use derive_more::Display;
use enum_as_inner::EnumAsInner;

#[derive(Debug, Clone, PartialEq, Eq, Hash, EnumAsInner)]
pub enum Expr {
    Literal {
        lit: Literal,
    },
    Variable {
        name: String,
    },
    Parameter {
        name: String,
    },
    MapExpression {
        keys: Vec<String>,
        values: Vec<Expr>,
    },
    PropertyAccess {
        map: Box<Expr>,
        key: String,
    },
    ListExpression {
        items: Vec<Expr>,
    },
    ListSlice {
        list: Box<Expr>,
        start: Option<Box<Expr>>,
        end: Option<Box<Expr>>,
    },
    ListIndex {
        list: Box<Expr>,
        index: Box<Expr>,
    },
    Unary {
        op: UnaryOperator,
        oprand: Box<Expr>,
    },
    Binary {
        left: Box<Expr>,
        op: BinaryOperator,
        right: Box<Expr>,
    },
    FunctionCall {
        name: String,
        distinct: bool,
        args: Vec<Expr>,
    },
}

impl Expr {
    pub fn new_boolean(value: bool) -> Self {
        Expr::Literal {
            lit: Literal::Boolean(value),
        }
    }

    pub fn new_integer(value: String) -> Self {
        Expr::Literal {
            lit: Literal::Integer(value),
        }
    }

    pub fn new_float(value: String) -> Self {
        Expr::Literal {
            lit: Literal::Float(value),
        }
    }

    pub fn new_string(value: String) -> Self {
        Expr::Literal {
            lit: Literal::String(value),
        }
    }

    pub fn new_null() -> Self {
        Expr::Literal { lit: Literal::Null }
    }

    pub fn new_variable(name: String) -> Self {
        Expr::Variable { name }
    }

    pub fn new_parameter(name: String) -> Self {
        Expr::Parameter { name }
    }

    pub fn new_map_expression(keys: Vec<String>, values: Vec<Expr>) -> Self {
        Expr::MapExpression { keys, values }
    }

    pub fn new_property_access(map: Expr, key: String) -> Self {
        Expr::PropertyAccess {
            map: Box::new(map),
            key,
        }
    }

    pub fn new_list_expression(items: Vec<Expr>) -> Self {
        Expr::ListExpression { items }
    }

    pub fn new_list_slice(list: Expr, start: Option<Expr>, end: Option<Expr>) -> Self {
        Expr::ListSlice {
            list: Box::new(list),
            start: start.map(Box::new),
            end: end.map(Box::new),
        }
    }

    pub fn new_list_index(list: Expr, index: Expr) -> Self {
        Expr::ListIndex {
            list: Box::new(list),
            index: Box::new(index),
        }
    }

    pub fn new_unary(op: UnaryOperator, oprand: Expr) -> Self {
        Expr::Unary {
            op,
            oprand: Box::new(oprand),
        }
    }

    pub fn new_binary(left: Expr, op: BinaryOperator, right: Expr) -> Self {
        Expr::Binary {
            left: Box::new(left),
            op,
            right: Box::new(right),
        }
    }

    pub fn new_function_call(name: String, distinct: bool, args: Vec<Expr>) -> Self {
        Expr::FunctionCall { name, distinct, args }
    }
}

impl std::fmt::Display for Expr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Expr::Literal { lit } => write!(f, "{lit}"),
            Expr::Variable { name } => write!(f, "{name}"),
            Expr::MapExpression { keys, values } => {
                write!(
                    f,
                    "{{{}}}",
                    keys.iter()
                        .zip(values)
                        .map(|(k, v)| format!("{k}: {v}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
            Expr::PropertyAccess { map, key } => write!(f, "{map}.{key}"),
            Expr::ListExpression { items } => {
                write!(
                    f,
                    "[{}]",
                    items.iter().map(|item| item.to_string()).collect::<Vec<_>>().join(", ")
                )
            }
            Expr::ListSlice { list, start, end } => {
                let start = start.as_ref().map(|x| x.to_string()).unwrap_or_default();
                let end = end.as_ref().map(|x| x.to_string()).unwrap_or_default();
                write!(f, "{list}[{start}..{end}]")
            }
            Expr::ListIndex { list, index } => {
                write!(f, "{list}[{index}]")
            }
            Expr::Parameter { name } => write!(f, "${name}"),
            Expr::Unary { op, oprand } => match op.associativity() {
                Associativity::Prefix => write!(f, "{op}({oprand})"),
                Associativity::Postfix => write!(f, "({oprand}){op}"),
            },
            Expr::Binary { left, op, right } => {
                write!(f, "({left}) {op} ({right})")
            }
            Expr::FunctionCall { name, distinct, args } => {
                write!(
                    f,
                    "{}({}{})",
                    name,
                    if *distinct { "DISTINCT " } else { "" },
                    args.iter().map(|arg| arg.to_string()).collect::<Vec<_>>().join(", ")
                )
            }
        }
    }
}

#[derive(Debug, Clone, Display, PartialEq, Eq, Hash, EnumAsInner)]
pub enum Literal {
    #[display("{}", if *_0 { "TRUE" } else { "FALSE" })]
    Boolean(bool),
    #[display("{}", _0)]
    Integer(String), // un-parsed integer
    #[display("{}", _0)]
    Float(String), // un-parsed float
    #[display("\'{}\'", _0)]
    String(String),
    #[display("NULL")]
    Null,
    #[display("INF")]
    Inf,
}

enum Associativity {
    Prefix,
    Postfix,
}

macro_rules! unary_operator {
    ($($variant:ident => $sym:expr, $assoc:expr, $func_name:expr),* $(,)?) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub enum UnaryOperator {
            $($variant),*
        }

        impl UnaryOperator {
            fn associativity(&self) -> Associativity {
                match self {
                    $(Self::$variant => $assoc),*
                }
            }

            pub fn as_func_name(&self) -> &'static str {
                match self {
                    $(Self::$variant => $func_name),*
                }
            }
        }


        impl std::fmt::Display for UnaryOperator {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    $(Self::$variant => write!(f, $sym),)*
                }
            }
        }
    };
}

macro_rules! binary_operator {
    ($($variant:ident => $sym:expr, $func_name:expr),* $(,)?) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub enum BinaryOperator {
            $($variant),*
        }

        impl BinaryOperator {
            pub fn as_func_name(&self) -> &'static str {
                match self {
                    $(Self::$variant => $func_name),*
                }
            }
        }

        impl std::fmt::Display for BinaryOperator {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    $(Self::$variant => write!(f, $sym),)*
                }
            }
        }
    };
}

unary_operator! {
    UnaryAdd => "+", Associativity::Prefix, "unary_add",
    UnarySubtract => "-", Associativity::Prefix, "unary_substract",
    Not => "NOT", Associativity::Prefix, "not",
    IsNull => "IS NULL",   Associativity::Postfix, "is_null",
    IsNotNull => "IS NOT NULL",   Associativity::Postfix, "is_not_null",
}

binary_operator! {
    // artimetic
    Add => "+", "add",
    Subtract => "-", "subtract",
    Multiply => "*", "multiply",
    Divide => "/", "divide",
    Modulo => "%", "modulo",
    Pow => "^", "pow",
    // list
    Concat => "||", "concat",
    // logical
    Or => "OR", "or",
    Xor => "XOR", "xor",
    And => "AND", "and",
    // comparison
    Eq => "=", "eq",
    NotEq => "<>", "not_eq",
    Gt => ">", "gt",
    GtEq => ">=", "gt_eq",
    Lt => "<", "lt",
    LtEq => "<=", "lt_eq",
    // comparasion2
    StartsWith => "STARTS WITH", "starts_with",
    EndsWith => "ENDS WITH", "ends_with",
    Contains => "CONTAINS", "contains",
    In => "IN", "in",
}

#[derive(Debug, Clone)]
pub enum LabelExpr {
    Label(String),
    // (n:A|B)
    Or(Box<LabelExpr>, Box<LabelExpr>),
    // (n:A&B)
    And(Box<LabelExpr>, Box<LabelExpr>),
}

impl LabelExpr {
    pub fn new_label(label: String) -> Self {
        Self::Label(label)
    }

    pub fn new_or(left: Self, right: Self) -> Self {
        Self::Or(Box::new(left), Box::new(right))
    }

    pub fn new_and(left: Self, right: Self) -> Self {
        Self::And(Box::new(left), Box::new(right))
    }

    // for relationship type, label expr should either be single reltype or reltype conjuncted with OR or NONE
    pub fn contains_only_or(&self) -> bool {
        match self {
            Self::Label(_) => true,
            Self::Or(left, right) => left.contains_only_or() && right.contains_only_or(),
            Self::And(_, _) => false,
        }
    }

    pub fn contains_only_and(&self) -> bool {
        match self {
            Self::Label(_) => true,
            Self::Or(_, _) => false,
            Self::And(left, right) => left.contains_only_and() && right.contains_only_and(),
        }
    }

    pub fn leafs(&self) -> Vec<String> {
        match self {
            Self::Label(label) => vec![label.clone()],
            Self::Or(left, right) => {
                let mut leafs = left.leafs();
                leafs.extend(right.leafs());
                leafs
            }
            Self::And(left, right) => {
                let mut leafs = left.leafs();
                leafs.extend(right.leafs());
                leafs
            }
        }
    }

    // in the context of relationship pattern, extract all relationship types from label expr
    // return None if label expr contains AND
    pub fn extract_relationship_types(&self) -> Option<Vec<String>> {
        match self {
            Self::Label(label) => Some(vec![label.clone()]),
            Self::Or(left, right) => {
                if let Some(mut ltypes) = left.extract_relationship_types()
                    && let Some(rtypes) = right.extract_relationship_types()
                {
                    ltypes.extend(rtypes);
                    return Some(ltypes);
                }
                None
            }
            Self::And(..) => {
                // relationship types should not contain AND
                None
            }
        }
    }
}

impl std::fmt::Display for LabelExpr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Label(label) => write!(f, "{label}"),
            Self::Or(left, right) => write!(f, "({left}|{right})"),
            Self::And(left, right) => write!(f, "({left}&{right})"),
        }
    }
}
