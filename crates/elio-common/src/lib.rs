#![feature(trusted_len)]
#![allow(clippy::double_parens)] // this is because EnumAsInner will generate extra parens

use std::sync::Arc;

use derive_more::PartialEq;
use enum_as_inner::EnumAsInner;
use serde::{Deserialize, Serialize};

pub mod array;
pub mod catalog;
pub mod data_type;
pub mod mapb;
pub mod order;
pub mod scalar;
pub mod schema;
pub mod store_types;
pub mod value;
pub mod variable;

pub type TokenId = u16;
pub type LabelId = TokenId;
pub type RelationshipTypeId = TokenId;
pub type PropertyKeyId = TokenId;

#[derive(Debug, Clone, Hash, PartialEq, Eq, derive_more::Display)]
#[display("ResolvedIrToken({name}, {id})")]
pub struct ResolvedIrToken {
    pub name: Arc<str>,
    pub id: TokenId,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, EnumAsInner, derive_more::Display)]
pub enum IrToken {
    #[display("Resolved({name}, {token})")]
    Resolved {
        name: Arc<str>,
        token: TokenId,
    },
    Unresolved(Arc<str>),
}

impl IrToken {
    pub fn name(&self) -> &Arc<str> {
        match self {
            Self::Resolved { name, .. } => name,
            Self::Unresolved(name) => name,
        }
    }

    pub fn token_id(&self) -> Option<TokenId> {
        match self {
            Self::Resolved { token, .. } => Some(*token),
            Self::Unresolved(_) => None,
        }
    }
}

impl From<(String, TokenId)> for IrToken {
    fn from(token: (String, TokenId)) -> Self {
        Self::Resolved {
            name: token.0.into(),
            token: token.1,
        }
    }
}

#[derive(Copy, Debug, Clone, Hash, PartialEq, Eq)]
pub enum TokenKind {
    Label,
    RelationshipType,
    PropertyKey,
}

pub type PropertyKey = String;
pub type Label = String;
pub type RelationshipType = String;

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntityKind {
    Node,
    Rel,
}

#[derive(
    derive_more::Display,
    derive_more::Deref,
    derive_more::From,
    derive_more::Into,
    Debug,
    Copy,
    Clone,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
)]
#[repr(transparent)]
pub struct NodeId(pub u64);

impl NodeId {
    pub fn from_le_bytes(bytes: [u8; 8]) -> Self {
        Self(u64::from_le_bytes(bytes))
    }

    pub fn from_be_bytes(bytes: [u8; 8]) -> Self {
        Self(u64::from_be_bytes(bytes))
    }
}

#[derive(
    derive_more::Display,
    derive_more::Deref,
    derive_more::From,
    derive_more::Into,
    Debug,
    Copy,
    Clone,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
)]
#[repr(transparent)]
pub struct RelationshipId(pub u64);

impl RelationshipId {
    pub fn from_be_bytes(bytes: [u8; 8]) -> Self {
        Self(u64::from_be_bytes(bytes))
    }
}

#[derive(Default, Debug, Copy, Clone, PartialEq, Eq, Hash, derive_more::Display)]
pub enum SemanticDirection {
    #[default]
    #[display("->")]
    Outgoing, // ->
    #[display("<-")]
    Incoming, // <-
    #[display("-")]
    Both, // -
}

impl SemanticDirection {
    pub fn is_both(&self) -> bool {
        matches!(self, Self::Both)
    }

    pub fn rev(&self) -> Self {
        match self {
            Self::Outgoing => Self::Incoming,
            Self::Incoming => Self::Outgoing,
            Self::Both => Self::Both,
        }
    }

    pub fn l_arrow(&self) -> String {
        match self {
            Self::Outgoing => "-".to_string(),
            Self::Incoming => "<-".to_string(),
            Self::Both => "-".to_string(),
        }
    }

    pub fn r_arrow(&self) -> String {
        match self {
            Self::Outgoing => "->".to_string(),
            Self::Incoming => "-".to_string(),
            Self::Both => "-".to_string(),
        }
    }
}
