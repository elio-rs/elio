use std::fmt;
use std::sync::Arc;

pub type F64 = ordered_float::OrderedFloat<f64>;

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, derive_more::Display)]
pub enum LogicalTypeKind {
    // special
    #[display("Null")]
    Null,
    // accept any type at binding type and will be mapped to variant physical type
    #[display("Any")]
    Any,
    // basic
    #[display("Bool")]
    Bool,
    #[display("Integer")]
    Integer,
    #[display("Float")]
    Float,
    // temporal
    #[display("Date")]
    Date,
    #[display("LocalTime")]
    LocalTime,
    #[display("LocalDateTime")]
    LocalDateTime,
    #[display("ZonedTime")]
    ZonedTime,
    #[display("ZonedDateTime")]
    ZonedDateTime,
    #[display("Duration")]
    Duration,
    //
    #[display("String")]
    String,
    #[display("TokenId")]
    TokenId,
    // graph
    #[display("VirtualNode")]
    VirtualNode,
    #[display("VirtualRel")]
    VirtualRel,
    #[display("VirtualPath")]
    VirtualPath,
    #[display("Node")]
    Node,
    #[display("Rel")]
    Rel,
    #[display("Path")]
    Path,
    // structure
    #[display("List")]
    List,
    #[display("Struct")]
    Struct,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum ExtraTypeInfo {
    List(Box<LogicalType>),
    Struct(Vec<(Arc<str>, LogicalType)>),
}

/// Here we put node/rel/path are physical types becase they are first class data types in graph query.
#[derive(Debug, Clone, Hash, PartialEq, Eq, derive_more::Display)]
pub enum PhysicalType {
    #[display("Variant")]
    Variant,
    #[display("Bool")]
    Bool,
    // graph
    #[display("VirtualNode")]
    VirtualNode,
    #[display("VirtualRel")]
    VirtualRel,
    #[display("VirtualPath")]
    VirtualPath,
    #[display("Node")]
    Node,
    #[display("Rel")]
    Rel,
    #[display("Path")]
    Path,
    #[display("List")]
    List,
    #[display("Struct")]
    Struct,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct LogicalType {
    pub kind: LogicalTypeKind,
    pub physical_type: PhysicalType,
    pub extra: Option<ExtraTypeInfo>,
}

/// Alias kept during migration so downstream code can refer to `DataType`.
/// TODO(pgao): should be removed in the future.
pub type DataType = LogicalType;

impl LogicalType {
    pub const fn physical_type(kind: LogicalTypeKind) -> PhysicalType {
        match kind {
            // null map to bool, it's just an place holder, null will be stored in bitmap and never stored in data
            // array.
            LogicalTypeKind::Null | LogicalTypeKind::Bool => PhysicalType::Bool,
            LogicalTypeKind::Integer
            | LogicalTypeKind::Float
            | LogicalTypeKind::Date
            | LogicalTypeKind::LocalTime
            | LogicalTypeKind::LocalDateTime
            | LogicalTypeKind::ZonedTime
            | LogicalTypeKind::ZonedDateTime
            | LogicalTypeKind::Duration
            | LogicalTypeKind::String
            | LogicalTypeKind::TokenId
            | LogicalTypeKind::Any => PhysicalType::Variant,
            LogicalTypeKind::VirtualNode => PhysicalType::VirtualNode,
            LogicalTypeKind::VirtualRel => PhysicalType::VirtualRel,
            LogicalTypeKind::Node => PhysicalType::Node,
            LogicalTypeKind::Rel => PhysicalType::Rel,
            LogicalTypeKind::Path => PhysicalType::Path,
            LogicalTypeKind::VirtualPath => PhysicalType::VirtualPath,
            LogicalTypeKind::List => PhysicalType::List,
            LogicalTypeKind::Struct => PhysicalType::Struct,
        }
    }
}

macro_rules! define_logical_type_consts {
    ($($const_name:ident => $kind:ident),+ $(,)?) => {
        $(
            pub const $const_name: Self = Self {
                kind: LogicalTypeKind::$kind,
                physical_type: Self::physical_type(LogicalTypeKind::$kind),
                extra: None,
            };
        )+
    };
}

impl LogicalType {
    define_logical_type_consts! {
        ANY => Any,
        BOOL => Bool,
        DATE => Date,
        DURATION => Duration,
        FLOAT => Float,
        INTEGER => Integer,
        LOCAL_DATE_TIME => LocalDateTime,
        LOCAL_TIME => LocalTime,
        NODE => Node,
        NULL => Null,
        PATH => Path,
        REL => Rel,
        STRING => String,
        TOKEN_ID => TokenId,
        VIRTUAL_NODE => VirtualNode,
        VIRTUAL_PATH => VirtualPath,
        VIRTUAL_REL => VirtualRel,
        ZONED_DATE_TIME => ZonedDateTime,
        ZONED_TIME => ZonedTime,
    }
}

impl LogicalType {
    #[inline]
    pub fn new_list(inner: LogicalType) -> Self {
        Self {
            kind: LogicalTypeKind::List,
            physical_type: Self::physical_type(LogicalTypeKind::List),
            extra: Some(ExtraTypeInfo::List(Box::new(inner))),
        }
    }

    #[inline]
    pub fn new_struct(fields: impl Iterator<Item = (Arc<str>, LogicalType)>) -> Self {
        Self {
            kind: LogicalTypeKind::Struct,
            physical_type: Self::physical_type(LogicalTypeKind::Struct),
            extra: Some(ExtraTypeInfo::Struct(fields.collect())),
        }
    }
}

impl LogicalType {
    #[inline]
    pub fn is_node(&self) -> bool {
        matches!(self.kind, LogicalTypeKind::Node | LogicalTypeKind::VirtualNode)
    }

    #[inline]
    pub fn is_rel(&self) -> bool {
        matches!(self.kind, LogicalTypeKind::Rel | LogicalTypeKind::VirtualRel)
    }

    #[inline]
    pub fn is_entity(&self) -> bool {
        self.is_node() || self.is_rel()
    }

    #[inline]
    pub fn is_list(&self) -> bool {
        self.kind == LogicalTypeKind::List
    }

    #[inline]
    pub fn is_struct(&self) -> bool {
        self.kind == LogicalTypeKind::Struct
    }

    /// Return the inner type for `List`, or `None`.
    #[inline]
    pub fn as_list(&self) -> Option<&LogicalType> {
        match &self.extra {
            Some(ExtraTypeInfo::List(inner)) => Some(inner),
            _ => None,
        }
    }

    /// Returns the fields for `Struct`, or `None`.
    #[inline]
    pub fn as_struct(&self) -> Option<&[(Arc<str>, LogicalType)]> {
        match &self.extra {
            Some(ExtraTypeInfo::Struct(fields)) => Some(fields),
            _ => None,
        }
    }

    pub fn materialize(&self) -> Self {
        match self.kind {
            LogicalTypeKind::VirtualNode => Self::NODE,
            LogicalTypeKind::VirtualRel => Self::REL,
            _ => self.clone(),
        }
    }

    pub fn signature(&self) -> std::string::String {
        match self.kind {
            LogicalTypeKind::Null => "null".to_string(),
            LogicalTypeKind::Bool => "b".to_string(),
            LogicalTypeKind::Integer => "i".to_string(),
            LogicalTypeKind::Float => "f".to_string(),
            LogicalTypeKind::Date => "d".to_string(),
            LogicalTypeKind::LocalTime => "t".to_string(),
            LogicalTypeKind::LocalDateTime => "dt".to_string(),
            LogicalTypeKind::ZonedTime => "zt".to_string(),
            LogicalTypeKind::ZonedDateTime => "zdt".to_string(),
            LogicalTypeKind::Duration => "dur".to_string(),
            LogicalTypeKind::String => "s".to_string(),
            LogicalTypeKind::TokenId => "u16".to_string(),
            LogicalTypeKind::Any => "any".to_string(),
            LogicalTypeKind::VirtualNode => "vnode".to_string(),
            LogicalTypeKind::VirtualRel => "vrel".to_string(),
            LogicalTypeKind::VirtualPath => "vpath".to_string(),
            LogicalTypeKind::Node => "node".to_string(),
            LogicalTypeKind::Rel => "rel".to_string(),
            LogicalTypeKind::Path => "path".to_string(),
            LogicalTypeKind::List => {
                let inner = self.as_list().expect("List must have inner type");
                format!("list({})", inner.signature())
            }
            LogicalTypeKind::Struct => {
                let fields = self.as_struct().expect("Struct must have fields");
                format!(
                    "struct({})",
                    fields
                        .iter()
                        .map(|(k, v)| format!("{}: {}", k, v.signature()))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
        }
    }
}

impl fmt::Display for LogicalType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            LogicalTypeKind::List => {
                let inner = self.as_list().expect("List must have inner type");
                write!(f, "List({})", inner)
            }
            LogicalTypeKind::Struct => {
                let fields = self.as_struct().expect("Struct must have fields");
                write!(
                    f,
                    "Struct({})",
                    fields
                        .iter()
                        .map(|(k, v)| format!("{}: {}", k, v))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
            _ => write!(f, "{}", self.kind),
        }
    }
}
