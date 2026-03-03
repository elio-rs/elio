use elio_common::data_type::LogicalType;
use enum_as_inner::EnumAsInner;

use crate::execution::expr::agg_call::AggInvocation;
use crate::execution::expr::scalar_call::ScalarInvocation;

/// Function definition
#[derive(Clone, Debug)]
pub struct FuncDef {
    pub name: String, // function name
    pub impls: Vec<FuncImpl>,
    pub is_agg: bool,
    // TODO: propagate nulls
}

/// Function implementation
#[derive(Clone, Debug)]
pub struct FuncImpl {
    // signature id, used to identify the function implementation
    // is the hash of args type.
    pub func_id: String,
    pub args: Vec<FuncImplArg>,
    pub ret: FuncImplReturn,
    // function pointer which is invoked when the function is called
    pub func_invoke: FuncInvoke,
}

#[derive(Clone, Debug, EnumAsInner)]
pub enum FuncInvoke {
    Scalar(ScalarInvocation),
    Agg(AggInvocation),
}

impl FuncImpl {
    pub fn new_scalar(name: &str, args: Vec<FuncImplArg>, ret: FuncImplReturn, func: ScalarInvocation) -> Self {
        let signature_id = Self::compute_signature(name, &args);
        Self {
            func_id: signature_id,
            args,
            ret,
            func_invoke: FuncInvoke::Scalar(func),
        }
    }

    pub fn new_agg(name: &str, args: Vec<FuncImplArg>, ret: FuncImplReturn, func: AggInvocation) -> Self {
        let signature_id = Self::compute_signature(name, &args);
        Self {
            func_id: signature_id,
            args,
            ret,
            func_invoke: FuncInvoke::Agg(func),
        }
    }

    fn compute_signature(name: &str, args: &[FuncImplArg]) -> String {
        format!(
            "{}({})",
            name,
            args.iter().map(|arg| arg.signature()).collect::<Vec<_>>().join(", ")
        )
    }
}

impl FuncImpl {
    // if match, return the return type
    pub fn matches(&self, args: &[LogicalType]) -> Option<LogicalType> {
        if self.args.len() != args.len() {
            return None;
        }
        for (i, func_arg) in self.args.iter().enumerate() {
            if !func_arg.matches_type(&args[i]) {
                return None;
            }
        }
        Some(self.ret.resolve_ret(args))
    }

    /// Match function signature with null coercion support.
    ///
    /// For arguments marked as untyped null, this method will try to infer
    /// their types from the function signature.
    ///
    /// # Arguments
    /// * `args` - The argument types
    /// * `is_untyped_null` - Boolean array indicating which arguments are untyped nulls
    ///
    /// # Returns
    /// * `Some((return_type, coerced_arg_types))` if match succeeds
    /// * `None` if match fails
    pub fn matches_with_null_coercion(
        &self,
        args: &[LogicalType],
        is_untyped_null: &[bool],
    ) -> Option<(LogicalType, Vec<LogicalType>)> {
        if self.args.len() != args.len() || self.args.len() != is_untyped_null.len() {
            return None;
        }

        let mut coerced_types = Vec::with_capacity(args.len());

        for (i, func_arg) in self.args.iter().enumerate() {
            let arg_type = if is_untyped_null[i] {
                // For untyped null, infer type from function signature
                match func_arg {
                    FuncImplArg::Exact(dt) => dt.clone(),
                    FuncImplArg::Union(types) => {
                        // Use first type in union as default
                        types.first().cloned().unwrap_or(LogicalType::ANY)
                    }
                    FuncImplArg::Templated(_) => {
                        // For templated args, keep as Any for now
                        LogicalType::ANY
                    }
                    FuncImplArg::AnyList => {
                        // For AnyList with null, default to List<Any>
                        LogicalType::new_list(LogicalType::ANY)
                    }
                }
            } else {
                // Use original type
                args[i].clone()
            };

            // Check if the coerced type matches the function signature
            if !func_arg.matches_type(&arg_type) {
                return None;
            }

            coerced_types.push(arg_type);
        }

        Some((self.ret.resolve_ret(&coerced_types), coerced_types))
    }
}

#[derive(Clone, Debug, Hash)]
pub enum FuncImplArg {
    // UNION type will be mapped into Any data type when execution
    // here we use union type here only for semantic checking
    // the actual function implementation siguare will be Any
    Union(Vec<LogicalType>),
    /// Exact argument type, e.g. `Int` in `map(Int, [1, 2, 3])`
    Exact(LogicalType),
    /// Templated argument type, e.g. `add<T>(T, T)`
    Templated(String),
    /// Matches any List type, e.g. `list_index(List<T>, Int) -> T`
    AnyList,
}

impl FuncImplArg {
    pub fn signature(&self) -> String {
        match self {
            FuncImplArg::Union(types) => {
                format!(
                    "anyof {}",
                    types.iter().map(|dt| dt.signature()).collect::<Vec<_>>().join(" | ")
                )
            }
            FuncImplArg::Exact(dt) => dt.signature(),
            FuncImplArg::Templated(t) => format!("{}<T>", t),
            FuncImplArg::AnyList => "list(any)".to_string(),
        }
    }

    /// Check if the given data type matches this argument spec.
    /// Returns true if it matches.
    pub fn matches_type(&self, dt: &LogicalType) -> bool {
        match self {
            FuncImplArg::Union(types) => types.contains(dt),
            FuncImplArg::Exact(expected) => expected == dt || *expected == LogicalType::ANY,
            FuncImplArg::Templated(_) => {
                // TODO(power): add type inference for templated arguments
                false
            }
            FuncImplArg::AnyList => dt.is_list(),
        }
    }
}

#[derive(Clone, Debug)]
pub enum FuncImplReturn {
    /// Exact return type, e.g. `add(Int, Int) -> Int`
    Exact(LogicalType),
    /// Templated return type, e.g. `add<T>(T, T) -> T`
    Templated(String),
    /// Return type is the same as the nth argument, e.g. `list_slice(List<T>, Int, Int) -> List<T>`
    SameAsArg(usize),
    /// Return type is the element type of the nth argument (which must be a List),
    /// e.g. `list_index(List<T>, Int) -> T`
    ListElement(usize),
}

impl FuncImplReturn {
    // resolve return type
    pub fn resolve_ret(&self, args: &[LogicalType]) -> LogicalType {
        match self {
            FuncImplReturn::Exact(data_type) => data_type.clone(),
            FuncImplReturn::Templated(_t) => {
                // TODO(power): add type inference for templated return type
                unimplemented!()
            }
            FuncImplReturn::SameAsArg(idx) => args.get(*idx).cloned().unwrap_or(LogicalType::ANY),
            FuncImplReturn::ListElement(idx) => {
                if let Some(inner) = args.get(*idx).and_then(|a| a.as_list()) {
                    inner.clone()
                } else {
                    LogicalType::ANY
                }
            }
        }
    }
}

#[macro_export]
macro_rules! func_impl_arg {
    ({ exact $dt:ident }) => {
        $crate::function::scalar::sig::FuncImplArg::Exact(elio_common::data_type::LogicalType::$dt)
    };

    ({ anyof $($dt:ident)|+ }) => {
        $crate::function::scalar::sig::FuncImplArg::Union(vec![$(elio_common::data_type::LogicalType::$dt),+])
    };

    ({ anylist }) => {
        $crate::function::scalar::sig::FuncImplArg::AnyList
    };
}

// generate function implementation
#[macro_export]
macro_rules! define_function {
    (name: $name:expr,
     impls: [
        $({
            args: [$($arg_type:tt),*],
            ret: $ret_type:ident,
            func: $func_impl:ident
        }),+
     ],
     is_agg: $is_agg:expr
    ) => {{
        $crate::function::scalar::sig::FuncDef{
            name: $name.to_string(),
            impls: vec![
                $({
                    use $crate::function::scalar::sig::FuncImplReturn;

                    $crate::function::scalar::sig::FuncImpl::new_scalar(
                        $name,
                        vec![$($crate::func_impl_arg!($arg_type)),*],
                        FuncImplReturn::Exact(elio_common::data_type::LogicalType::$ret_type),
                        $func_impl,
                    )
                },)+
            ],
            is_agg: $is_agg,
        }
    }};
}
