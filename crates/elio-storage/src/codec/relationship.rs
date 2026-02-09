//! Relationship stored in key-value database.
//!
//! RelationshipKey ::= <cf_topology::REL_KEY_PREFIX> <src_node_id> <DIRECTION> <reltype_id> <dst_node_id> <rel_id>
//! DIRECTION ::= <cf_topology::DIR_OUT> | <cf_topology::DIR_IN>
//!
//! RelationshipValue ::= <PropertyBlock>

use bytes::{BufMut, Bytes, BytesMut};
use elio_common::mapb::{PropertyMapMut, PropertyMapRef};
use elio_common::scalar::StructValueRef;
use elio_common::store_types::RelDirection;
use elio_common::{NodeId, RelationshipId, SemanticDirection, TokenId};

use crate::cf_topology;

pub struct RelFormat;

impl RelFormat {
    pub fn encode_key(
        src_node_id: NodeId,
        direction: RelDirection,
        reltype: TokenId,
        dst_node_id: NodeId,
        rel_id: RelationshipId,
    ) -> Bytes {
        let mut bytes = BytesMut::new();
        bytes.put_u8(cf_topology::REL_KEY_PREFIX);
        bytes.put_u64(*src_node_id);
        bytes.put_u8(direction as u8);
        bytes.put_u16(reltype);
        bytes.put_u64(*dst_node_id);
        bytes.put_u64(*rel_id);
        bytes.freeze()
    }

    pub fn decode_key(buf: &[u8]) -> (NodeId, RelDirection, TokenId, NodeId, RelationshipId) {
        assert!(buf.len() >= 25);
        let src_node_id = NodeId::from_be_bytes(buf[1..9].try_into().unwrap());
        let direction = RelDirection::from(buf[9]);
        let reltype = TokenId::from_be_bytes(buf[10..12].try_into().unwrap());
        let dst_node_id = NodeId::from_be_bytes(buf[12..20].try_into().unwrap());
        let rel_id = RelationshipId::from_be_bytes(buf[20..28].try_into().unwrap());
        (src_node_id, direction, reltype, dst_node_id, rel_id)
    }

    pub fn encode_value(key_ids: &[TokenId], property_map: StructValueRef<'_>) -> Result<Bytes, String> {
        let mut buf = BytesMut::new();
        let mut mapb_mut = PropertyMapMut::with_capacity(key_ids.len());

        assert_eq!(key_ids.len(), property_map.len());

        // put properties
        for (key_id, (_, prop)) in key_ids.iter().zip(property_map.iter()) {
            mapb_mut.insert(*key_id, Some(&prop))?;
        }
        mapb_mut.freeze().write(&mut buf);
        Ok(buf.freeze())
    }

    pub fn decode_value(buf: &[u8]) -> PropertyMapRef<'_> {
        PropertyMapRef::new(buf)
    }

    // node out rel prefix: node_id and direction
    pub fn node_rel_iter_prefix(node_id: NodeId, dir: SemanticDirection) -> Bytes {
        let mut bytes = BytesMut::new();
        bytes.put_u8(cf_topology::REL_KEY_PREFIX);
        bytes.put_u64(*node_id);
        match dir {
            SemanticDirection::Outgoing => bytes.put_u8(RelDirection::Out as u8),
            SemanticDirection::Incoming => bytes.put_u8(RelDirection::In as u8),
            SemanticDirection::Both => ()/* put nothing */,
        }
        bytes.freeze()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_encode_decode_key() {
        let key = RelFormat::encode_key(
            NodeId::from(1),
            RelDirection::Out,
            2,
            NodeId::from(3),
            RelationshipId::from(4),
        );
        let (src_node_id, direction, reltype, dst_node_id, rel_id) = RelFormat::decode_key(&key);
        assert_eq!(src_node_id, NodeId::from(1));
        assert_eq!(direction, RelDirection::Out);
        assert_eq!(reltype, 2);
        assert_eq!(dst_node_id, NodeId::from(3));
        assert_eq!(rel_id, RelationshipId::from(4));
    }
}
