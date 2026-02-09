#![allow(clippy::double_parens)]
// Copyright 2022 Alex Chi. Licensed under Apache-2.0.

//! Contains array types for the system
//!
//! This crate contains two category of structs -- ArrayBuilder and Array. Developers may use
//! ArrayBuilder to create an Array. ArrayBuilder and Array are reciprocal traits. We can associate
//! an Array with an ArrayBuilder at compile time. This module also contains examples on how to use
//! generics around the Array and ArrayBuilder.
//! This file is derived from type-exercise-in-rust and modified by elio.

pub mod any;
pub mod bool;
pub mod chunk;
// pub mod datum;
pub mod list;
pub mod node;
pub mod path;
pub mod rel;
pub mod struct_;

pub mod iter;

use std::sync::Arc;

pub use any::*;
use bitvec::prelude::*;
pub use bool::*;
pub use chunk::*;
use enum_as_inner::EnumAsInner;
pub use list::*;
pub use node::*;
pub use path::*;
pub use rel::*;
pub use struct_::*;

use super::scalar::*;
use crate::array::iter::ArrayIterator;
use crate::scalar::ScalarRefVTable;
use crate::{NodeId, RelationshipId};

/// [`Array`] is a collection of data of the same type.
pub trait Array: Send + Sync + Sized + 'static + Into<ArrayImpl> + std::fmt::Debug + Clone
// where
// for<'a> Self::OwnedItem: Scalar<RefType<'a> = Self::RefItem<'a>>,
{
    // The corresponding [`ArrayBuilder`] of this [`Array`].
    //
    // We constriant the associated type so that `Self::Builder::Array = Self`.
    // type Builder: ArrayBuilder<Array = Self>;
    // The owned item of this array.
    // type OwnedItem: Scalar<ArrayType = Self>;

    /// Type of the item that can be retrieved from the [`Array`]. For example, we can get a `i32`
    /// from [`I32Array`], while [`StringArray`] produces a `&str`. As we need a lifetime that is
    /// the same as `self` for `&str`, we use GAT here.
    type RefItem<'a>;

    /// Retrieve a reference to value.
    /// TODO(pgao): use get_unchecked to implement get
    fn get(&self, idx: usize) -> Option<Self::RefItem<'_>>;

    /// Get a reference to value without bounds check.
    /// # SAFETY
    ///    Caller must ensure that `idx` is valid.
    unsafe fn get_unchecked(&self, idx: usize) -> Self::RefItem<'_>;

    /// Number of items of array.
    fn len(&self) -> usize;

    /// Indicates whether this array is empty
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get iterator of this array.
    fn iter(&self) -> ArrayIterator<'_, Self> {
        ArrayIterator::new(self)
    }

    fn physical_type(&self) -> PhysicalType;

    fn compact(&self, visibility: &BitVec, new_len: usize) -> Self;
}

pub type ArrayRef = Arc<ArrayImpl>;

#[derive(EnumAsInner, Clone, Debug, PartialEq, Eq)]
pub enum ArrayImpl {
    Any(AnyArray),
    Bool(BoolArray),
    // graph
    VirtualNode(VirtualNodeArray),
    VirtualRel(VirtualRelArray),
    VirtualPath(VirtualPathArray),
    Node(NodeArray),
    Rel(RelArray),
    Path(PathArray),
    // structure
    List(ListArray),
    Struct(StructArray),
}

#[macro_export]
macro_rules! impl_partial_eq_for_variant {
    ($($AbcArray:ident),*) => {

        $(
            impl PartialEq for $AbcArray {
                fn eq(&self, other: &Self) -> bool {
                    self.len() == other.len() && self.iter().eq(other.iter())
                }
            }

            impl Eq for $AbcArray {}
        )*
    };
}

impl_partial_eq_for_variant!(
    AnyArray,
    BoolArray,
    VirtualNodeArray,
    VirtualRelArray,
    VirtualPathArray,
    NodeArray,
    RelArray,
    PathArray,
    ListArray,
    StructArray
);

macro_rules! impl_array_dispatch {
    ($($variant:ident),*) => {
        impl ArrayImpl {
            pub fn physical_type(&self) -> PhysicalType {
                match self {
                    $(ArrayImpl::$variant(a) => a.physical_type(),)*
                }
            }

            pub fn valid_map(&self) -> &BitVec {
                match self {
                    $(ArrayImpl::$variant(a) => a.valid_map(),)*
                }
            }

            pub fn set_valid_map(&mut self, valid: BitVec) {
                match self {
                    $(ArrayImpl::$variant(a) => a.set_valid_map(valid),)*
                }
            }

            pub fn get(&self, idx: usize) -> Option<ScalarRef<'_>> {
                match self {
                    $(ArrayImpl::$variant(a) => a.get(idx).map(ScalarRef::from),)*
                }
            }

            #[allow(clippy::len_without_is_empty)]
            pub fn len(&self) -> usize {
                match self {
                    $(ArrayImpl::$variant(a) => a.len(),)*
                }
            }

            pub fn compact(&self, visibility: &BitVec, new_len: usize) -> Self {
                match self {
                    $(ArrayImpl::$variant(a) => a.compact(visibility, new_len).into(),)*
                }
            }
        }
    };
}

impl_array_dispatch!(
    Any,
    Bool,
    VirtualNode,
    VirtualRel,
    VirtualPath,
    Node,
    Rel,
    Path,
    List,
    Struct
);

macro_rules! impl_array_convert {
    ($({$Abc:ident, $AbcArray:ident}),*) => {

        $(
            impl From<$AbcArray> for ArrayImpl {
                fn from(array: $AbcArray) -> Self {
                    ArrayImpl::$Abc(array)
                }
            }
        )*
    };
}

impl_array_convert!(
{Any, AnyArray},
{Bool, BoolArray},
{VirtualNode, VirtualNodeArray},
{VirtualRel, VirtualRelArray},
{VirtualPath, VirtualPathArray},
{Node, NodeArray},
{Rel, RelArray},
{Path, PathArray},
{List, ListArray},
{Struct, StructArray});

#[derive(Debug, EnumAsInner)]
pub enum ArrayBuilderImpl {
    Any(AnyArrayBuilder),
    Bool(BoolArrayBuilder),
    // graph
    VirtualNode(VirtualNodeArrayBuilder),
    VirtualRel(VirtualRelArrayBuilder),
    VirtualPath(VirtualPathArrayBuilder),
    Node(NodeArrayBuilder),
    Rel(RelArrayBuilder),
    Path(PathArrayBuilder),
    // structure
    List(ListArrayBuilder),
    Struct(StructArrayBuilder),
}

impl ArrayBuilderImpl {
    pub fn push_n(&mut self, item: Option<ScalarRef<'_>>, repeat: usize) {
        match self {
            ArrayBuilderImpl::Any(any) => {
                any.push_n(item, repeat);
            }
            ArrayBuilderImpl::Bool(b) => {
                let item = item.map(|x| x.into_bool().expect("type mismatch expected bool"));
                b.push_n(item, repeat);
            }
            ArrayBuilderImpl::VirtualNode(vnode) => {
                let item = item.map(|x| x.into_virtual_node().expect("type mismatch expected virtual node"));
                vnode.push_n(item, repeat);
            }
            ArrayBuilderImpl::VirtualRel(vrel) => {
                let item = item.map(|x| x.into_virtual_rel().expect("type mismatch expected virtual rel"));
                vrel.push_n(item, repeat);
            }
            ArrayBuilderImpl::VirtualPath(vpath) => {
                let item = item.map(|x| x.into_virtual_path().expect("type mismatch expected virtual path"));
                vpath.push_n(item, repeat);
            }
            ArrayBuilderImpl::Node(node) => {
                let item = item.map(|x| x.into_node().expect("type mismatch expected node"));
                node.push_n(item, repeat);
            }
            ArrayBuilderImpl::Rel(rel) => {
                let item = item.map(|x| x.into_rel().expect("type mismatch expected rel"));
                rel.push_n(item, repeat);
            }
            ArrayBuilderImpl::Path(path) => {
                let item = item.map(|x| x.into_path().expect("type mismatch expected path"));
                path.push_n(item, repeat);
            }
            ArrayBuilderImpl::List(list) => {
                let item = item.map(|x| x.into_list().expect("type mismatch expected list"));
                list.push_n(item, repeat);
            }
            ArrayBuilderImpl::Struct(struct_array_builder) => {
                let item = item.map(|x| x.into_struct().expect("type mismatch expected struct"));
                struct_array_builder.push_n(item, repeat);
            }
        }
    }

    pub fn push(&mut self, item: Option<ScalarRef<'_>>) {
        self.push_n(item, 1);
    }
}

macro_rules! impl_array_builder_dispatch {
    ($($variant:ident),*) => {
        impl ArrayBuilderImpl {
            pub fn finish(self) -> ArrayImpl {
                match self{
                    $(ArrayBuilderImpl::$variant(b) => ArrayImpl::$variant(b.finish()),)*
                }
            }
        }
    };
}

impl_array_builder_dispatch!(
    Any,
    Bool,
    VirtualNode,
    VirtualRel,
    VirtualPath,
    Node,
    Rel,
    Path,
    List,
    Struct
);

macro_rules! impl_array_builder_convert {
    ($({ $Abc:ident, $AbcBuilder:ident }),*) => {
        $(
            impl From<$AbcBuilder> for ArrayBuilderImpl {
                fn from(builder: $AbcBuilder) -> Self {
                    ArrayBuilderImpl::$Abc(builder)
                }
            }
        )*
    };
}

impl_array_builder_convert!(
{Any, AnyArrayBuilder},
{Bool, BoolArrayBuilder},
{VirtualNode, VirtualNodeArrayBuilder},
{VirtualRel, VirtualRelArrayBuilder},
{VirtualPath, VirtualPathArrayBuilder},
{Node, NodeArrayBuilder},
{Rel, RelArrayBuilder},
{Path, PathArrayBuilder},
{List, ListArrayBuilder},
{Struct, StructArrayBuilder});

// physical array type
#[derive(Debug, PartialEq, Eq)]
pub enum PhysicalType {
    // basic
    Any,
    Bool, // for filter
    // graph
    VirtualNode,
    VirtualRel,
    VirtualPath,
    Node,
    Rel,
    Path,
    // structure
    List(Box<PhysicalType>),
    // (field_name, field type)
    Struct(Box<[(Arc<str>, PhysicalType)]>),
}

impl PhysicalType {
    pub fn array_builder(&self, capacity: usize) -> ArrayBuilderImpl {
        match self {
            PhysicalType::Any => ArrayBuilderImpl::Any(AnyArrayBuilder::with_capacity(capacity)),
            PhysicalType::Bool => ArrayBuilderImpl::Bool(BoolArrayBuilder::with_capacity(capacity)),
            PhysicalType::VirtualNode => {
                ArrayBuilderImpl::VirtualNode(VirtualNodeArrayBuilder::with_capacity(capacity))
            }
            PhysicalType::VirtualRel => ArrayBuilderImpl::VirtualRel(VirtualRelArrayBuilder::with_capacity(capacity)),
            PhysicalType::VirtualPath => {
                ArrayBuilderImpl::VirtualPath(VirtualPathArrayBuilder::with_capacity(capacity))
            }
            PhysicalType::Node => ArrayBuilderImpl::Node(NodeArrayBuilder::with_capacity(capacity)),
            PhysicalType::Rel => ArrayBuilderImpl::Rel(RelArrayBuilder::with_capacity(capacity)),
            PhysicalType::Path => ArrayBuilderImpl::Path(PathArrayBuilder::with_capacity(capacity)),
            PhysicalType::List(inner) => {
                let child = inner.array_builder(capacity);
                ArrayBuilderImpl::List(ListArrayBuilder::new(Box::new(child)))
            }
            PhysicalType::Struct(fields) => {
                let mut field_builders = Vec::with_capacity(fields.len());
                for (name, ty) in fields {
                    field_builders.push((name.clone(), ty.array_builder(capacity)));
                }
                ArrayBuilderImpl::Struct(StructArrayBuilder::new(field_builders.into_iter()))
            }
        }
    }
}

// Node or Rel Array
pub trait EntityArray: Array
where
    for<'a> <Self as Array>::RefItem<'a>: EntityScalarRef,
{
}
