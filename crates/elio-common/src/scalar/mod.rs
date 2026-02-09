#![allow(clippy::double_parens)]
// since EnumAsInner generate double () codes, we ignore the clippy warning here

// Copyright 2022 Alex Chi. Licensed under Apache-2.0.

//! Contains types for single values
//!
//! This crate contains two reciprocal traits -- Scalar and ScalarRef. As it is named, Scalar is an
//! owned value of ScalarRef, and ScalarRef is a reference to Scalar. We associate Scalar and
//! ScalarRef with Array types, and present examples on how to use these traits.
//! This file is copied from type-exercise-in-rust and modified to fit the needs of elio.

use std::hash::Hash;
use std::sync::Arc;

use common_macros::ScalarPartialOrd;
use enum_as_inner::EnumAsInner;
use itertools::Itertools;

use crate::array::{Array, ArrayImpl, StructArray};
use crate::data_type::{DataType, F64};
use crate::store_types::RelDirection;
use crate::{IrToken, NodeId, RelationshipId};

pub mod node;
pub use node::*;
pub mod rel;
pub use rel::*;
pub mod struct_;
pub use struct_::*;
pub mod path;
pub use path::*;
pub mod list;
pub use list::*;
pub mod temporal;
pub use temporal::*;

pub trait ScalarVTable:
    std::fmt::Debug + std::fmt::Display + Clone + Send + Sync + 'static + Into<ScalarValue>
{
    /// The corresponding [`ScalarRef`] type.
    type RefType<'a>: ScalarRefVTable<'a, ScalarType = Self>;

    /// Get a reference of the current value.
    fn as_scalar_ref(&self) -> Self::RefType<'_>;
}

/// An borrowed value.
///
/// For example, `i32`, `&str` both implements [`ScalarRef`].
pub trait ScalarRefVTable<'a>:
    std::fmt::Debug + std::fmt::Display + Clone + Copy + Send + 'a + Into<ScalarRef<'a>>
{
    /// The corresponding [`Scalar`] type.
    type ScalarType: ScalarVTable<RefType<'a> = Self>;

    /// Convert the reference into an owned value.
    fn to_owned_scalar(&self) -> Self::ScalarType;
}

/// ScalarTypes which are non nested and the data type can be known at compile time
pub trait LeafScalarType: ScalarVTable {
    const TYPE: DataType;
}

impl ScalarVTable for ScalarValue {
    type RefType<'a> = ScalarRef<'a>;

    fn as_scalar_ref(&self) -> Self::RefType<'_> {
        self.as_scalar_ref()
    }
}

impl<'a> ScalarRefVTable<'a> for ScalarRef<'a> {
    type ScalarType = ScalarValue;

    fn to_owned_scalar(&self) -> Self::ScalarType {
        self.to_owned_scalar()
    }
}

impl LeafScalarType for ScalarValue {
    const TYPE: DataType = DataType::Any;
}

#[derive(derive_more::Display, Debug, Clone, Default, EnumAsInner, Eq, PartialEq, Hash)]
pub enum ScalarValue {
    // this is the place holder for null values
    #[default]
    #[display("null")]
    Unknown,
    // primitives
    Bool(bool),
    Integer(i64),
    Float(F64),
    // temporal
    Date(Date),
    LocalTime(LocalTime),
    LocalDateTime(LocalDateTime),
    ZonedDateTime(ZonedDateTime),
    Duration(Duration),
    #[display("'{}'", _0)]
    String(String),
    // graph
    #[display("VirtualNode{{_0}}")]
    VirtualNode(NodeId),
    VirtualRel(VirtualRel),
    VirtualPath(VirtualPath),
    Node(Box<NodeValue>),
    Rel(Box<RelValue>),
    Path(Box<PathValue>),
    // nested
    List(Box<ListValue>),
    Struct(Box<StructValue>),
}

macro_rules! impl_scalar_value_convert {
    (box $abc:ty, $Abc:ident) => {
        impl From<$abc> for ScalarValue {
            fn from(value: $abc) -> Self {
                Self::$Abc(Box::new(value))
            }
        }
    };

    ($abc:ty, $Abc:ident) => {
        impl From<$abc> for ScalarValue {
            fn from(value: $abc) -> Self {
                Self::$Abc(value)
            }
        }
    };
}

impl_scalar_value_convert!(bool, Bool);
impl_scalar_value_convert!(i64, Integer);
impl_scalar_value_convert!(F64, Float);
impl_scalar_value_convert!(Date, Date);
impl_scalar_value_convert!(LocalTime, LocalTime);
impl_scalar_value_convert!(LocalDateTime, LocalDateTime);
impl_scalar_value_convert!(ZonedDateTime, ZonedDateTime);
impl_scalar_value_convert!(Duration, Duration);
impl_scalar_value_convert!(String, String);
impl_scalar_value_convert!(NodeId, VirtualNode);
impl_scalar_value_convert!(VirtualRel, VirtualRel);
impl_scalar_value_convert!(VirtualPath, VirtualPath);
impl_scalar_value_convert!(box NodeValue, Node);
impl_scalar_value_convert!(box RelValue, Rel);
impl_scalar_value_convert!(box PathValue, Path);
impl_scalar_value_convert!(box ListValue, List);
impl_scalar_value_convert!(box StructValue, Struct);

macro_rules! impl_scalar_dispatch {
    ($({$Abc:ident, $abc:ty}),*) => {
        impl ScalarValue {
            pub fn as_scalar_ref(&self) -> ScalarRef<'_> {
                match self {
                    $(Self::$Abc(x) => ScalarRef::$Abc(x.as_scalar_ref()),)*
                    _ => ScalarRef::Null,
                }
            }
        }
    };
}

impl_scalar_dispatch!(
    {Bool, bool},
    {Integer, i64},
    {Float, F64},
    {Date, Date},
    {LocalTime, LocalTime},
    {LocalDateTime, LocalDateTime},
    {ZonedDateTime, ZonedDateTime},
    {Duration, Duration},
    {String, &'a str},
    // graph
    {VirtualNode, NodeId},
    {VirtualRel, VirtualRelRef<'a>},
    {VirtualPath, VirtualPathRef<'a>},
    {Node, NodeValueRef<'a>},
    {Rel, RelValueRef<'a>},
    {Path, PathValueRef<'a>},
    {List, ListValueRef<'a>},
    {Struct, StructValueRef<'a>}
);

#[derive(Debug, EnumAsInner, Clone, Copy, PartialEq, Eq, Hash, derive_more::Display)]
pub enum ScalarRef<'a> {
    #[display("null")]
    Null,
    Bool(bool),
    Integer(i64),
    Float(F64),
    // temporal
    Date(Date),
    LocalTime(LocalTime),
    LocalDateTime(LocalDateTime),
    ZonedDateTime(ZonedDateTime),
    Duration(Duration),
    #[display("'{}'", _0)]
    String(&'a str),
    // graph
    VirtualNode(NodeId),
    VirtualRel(VirtualRelRef<'a>),
    VirtualPath(VirtualPathRef<'a>),
    Node(NodeValueRef<'a>),
    Rel(RelValueRef<'a>),
    Path(PathValueRef<'a>),
    //
    List(ListValueRef<'a>),
    Struct(StructValueRef<'a>),
}

impl<'a> ScalarRef<'a> {
    pub fn get_node_id(&self) -> Option<NodeId> {
        match self {
            ScalarRef::VirtualNode(node_id) => Some(*node_id),
            ScalarRef::Node(node_value_ref) => Some(node_value_ref.id),
            _ => None,
        }
    }
}

macro_rules! impl_into_for_scalar_ref {
    // without lifetime
    ($({$AbcRef:ty, $Abc:ident}),*) => {
        $(impl<'a> From<$AbcRef> for ScalarRef<'a> {
            fn from(value: $AbcRef) -> ScalarRef<'a> {
                ScalarRef::$Abc(value)
            }
        })*
    };

    // with lifetime
    ($({&'a $AbcRef:ty, $Abc:ident}),*) => {
        $(impl<'a> From<&'a $AbcRef> for ScalarRef<'a> {
            fn from(value: &'a $AbcRef) -> ScalarRef<'a> {
                ScalarRef::$Abc(value)
            }
        })*
    };
}

impl_into_for_scalar_ref!(
    {bool, Bool},
    {i64, Integer},
    {F64, Float},
    {Date, Date},
    {LocalTime, LocalTime},
    {LocalDateTime, LocalDateTime},
    {ZonedDateTime, ZonedDateTime},
    {Duration, Duration},
    {&'a str, String},
    {NodeId, VirtualNode},
    {VirtualRelRef<'a>, VirtualRel},
    {NodeValueRef<'a>, Node},
    {VirtualPathRef<'a>, VirtualPath},
    {RelValueRef<'a>, Rel},
    {ListValueRef<'a>, List},
    {PathValueRef<'a>, Path},
    {StructValueRef<'a>, Struct}
);

macro_rules! impl_scalar_ref_dispatch {
    ($({$Abc:ident}),*) => {
            impl<'a> ScalarRef<'a>{
                pub fn to_owned_scalar(&self) -> ScalarValue{
                    match self{
                        $(Self::$Abc(x) => x.to_owned_scalar().into(),)*
                        Self::Null => ScalarValue::Unknown,
                    }
                }
            }
    };
}

impl_scalar_ref_dispatch!(
    { Bool },
    { Integer },
    { Float },
    { Date },
    { LocalTime },
    { LocalDateTime },
    { ZonedDateTime },
    { Duration },
    { String },
    { VirtualNode },
    { VirtualRel },
    { VirtualPath },
    { Node },
    { Rel },
    { Path },
    { List },
    { Struct }
);

macro_rules! impl_scalar_for_primitive {
    ($({$value:ident, $ref:ty}),*) => {
        $(
            impl ScalarVTable for $value {
                type RefType<'a> = $ref;

                fn as_scalar_ref(&self) -> Self::RefType<'_> {
                    *self
                }
            }

            impl<'a> ScalarRefVTable<'a> for $ref {
                type ScalarType = $value;

                fn to_owned_scalar(&self) -> Self::ScalarType {
                    *self
                }
            }
        )*
    };
}

impl_scalar_for_primitive!(
    {bool,bool},
    {i64,i64},
    {F64,F64},
    {Date, Date},
    {LocalTime, LocalTime},
    {LocalDateTime, LocalDateTime},
    {ZonedDateTime, ZonedDateTime},
    {Duration, Duration},
    {NodeId, NodeId}
);

impl ScalarVTable for String {
    type RefType<'a> = &'a str;

    fn as_scalar_ref(&self) -> Self::RefType<'_> {
        self.as_str()
    }
}

impl<'a> ScalarRefVTable<'a> for &'a str {
    type ScalarType = String;

    fn to_owned_scalar(&self) -> Self::ScalarType {
        self.to_string()
    }
}

macro_rules! impl_leaf_scalar_type {
    ($({$type:ty, $type_name:ident}),*) => {
        $(
            impl LeafScalarType for $type {
                const TYPE: DataType = DataType::$type_name;
            }
        )*
    }
}

impl_leaf_scalar_type!(
    {bool, Bool},
    {i64, Integer},
    {F64, Float},
    {Date, Date},
    {LocalTime, LocalTime},
    {LocalDateTime, LocalDateTime},
    {ZonedDateTime, ZonedDateTime},
    {Duration, Duration},
    {String, String},
    {NodeId, VirtualNode},
    {VirtualRel, VirtualRel},
    {NodeValue, Node},
    {VirtualPath, VirtualPath},
    {RelValue, Rel},
    {PathValue, Path}
);

pub type Row = Vec<Option<ScalarValue>>;

pub type Datum = Option<ScalarValue>;
pub type DatumRef<'a> = Option<&'a ScalarValue>;

pub trait EntityScalarRef {
    fn has_ir_label(&self, label: &IrToken) -> bool;
}

pub trait ScalarPartialOrd {
    fn scalar_partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering>;
}

impl<'a> ScalarPartialOrd for ScalarRef<'a> {
    fn scalar_partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        macro_rules! match_same_type {
            ($($variant:ident),*) => {
                match (self, other) {
                    // Null != Null
                    (ScalarRef::Null, ScalarRef::Null) => None,
                    (ScalarRef::Bool(x), ScalarRef::Bool(y)) => x.partial_cmp(y),
                    (ScalarRef::Integer(x), ScalarRef::Integer(y)) => x.partial_cmp(y),
                    (ScalarRef::Float(x), ScalarRef::Float(y)) => x.partial_cmp(y),
                    (ScalarRef::String(x), ScalarRef::String(y)) => x.partial_cmp(y),
                    (ScalarRef::VirtualNode(x), ScalarRef::VirtualNode(y)) => x.partial_cmp(y),
                    // different types, cast to upper
                    (ScalarRef::Integer(x), ScalarRef::Float(y)) =>{
                        (F64::from(*x as f64)).partial_cmp(y)
                    }
                    (ScalarRef::Float(x), ScalarRef::Integer(y)) =>{
                        x.partial_cmp(&F64::from(*y as f64))
                    }
                    $(
                        (Self::$variant(x), Self::$variant(y)) => x.scalar_partial_cmp(y),
                    )*
                    _ => None,
                }
            };
        }

        match_same_type!(
            Date,
            LocalTime,
            LocalDateTime,
            ZonedDateTime,
            Duration,
            VirtualRel,
            VirtualPath,
            Node,
            Rel,
            Path,
            List,
            Struct
        )
    }
}
