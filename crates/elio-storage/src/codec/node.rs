//！NodeKey ::= <node_id>
//! NodeValue ::= <NodeHeader> <LabelBlocks> <<PropertyBlock>
//! Nodeheader ::= <NumLabels> <PropertySize>
//!
//! NumLabels ::= u16
//!
//! LabelBlock ::= <LabelId>{NumLabels}

use bytes::{BufMut, Bytes, BytesMut};
use elio_common::mapb::{PropertyMapMut, PropertyMapRef};
use elio_common::scalar::StructValueRef;
use elio_common::{LabelId, NodeId, TokenId};

pub struct NodeFormat;

impl NodeFormat {
    pub fn encode_node_key(node_id: NodeId) -> Bytes {
        let mut key = BytesMut::new();
        key.put_slice(crate::cf_property::NODE_KEY_PREFIX);
        // use big endian to let nodes ordered by node id
        key.put_u64(*node_id);
        key.freeze()
    }

    pub fn decode_node_key(buf: &[u8]) -> NodeId {
        assert_eq!(buf.len(), 9);
        NodeId::from_be_bytes(buf[1..9].try_into().unwrap())
    }

    pub fn encode_node_value(
        labels: &[LabelId],
        key_ids: &[TokenId],
        property_map: StructValueRef<'_>,
    ) -> Result<Bytes, String> {
        let mut buf = BytesMut::new();
        // serizlie
        let prop_buf = Self::encode_property_value(key_ids, property_map)?;

        // put header
        buf.put_u16_le(labels.len() as u16);
        buf.put_u32_le(prop_buf.len() as u32);
        // put labels
        for label_id in labels {
            buf.put_u16_le(*label_id);
        }
        // put properties
        buf.put_slice(&prop_buf);
        Ok(buf.freeze())
    }

    pub fn encode_property_value(key_ids: &[TokenId], property_map: StructValueRef<'_>) -> Result<Bytes, String> {
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

    pub fn decode_node_value(buf: &[u8]) -> Result<(LabelIdListRef<'_>, PropertyMapRef<'_>), String> {
        if buf.len() < 6 {
            return Err("buffer too short for header".to_string());
        }
        let label_len = u16::from_le_bytes(buf[0..2].try_into().unwrap()) as usize;
        let prop_byte_len = u32::from_le_bytes(buf[2..6].try_into().unwrap()) as usize;
        // check buf length
        let totoal_byte_len = 6 + label_len * 2 + prop_byte_len;
        if buf.len() < totoal_byte_len {
            return Err(format!(
                "buf len {} is less than totoal byte len {}",
                buf.len(),
                totoal_byte_len
            ));
        }
        let label_block = &buf[6..6 + label_len * 2];
        let prop_block = &buf[6 + label_len * 2..6 + label_len * 2 + prop_byte_len];
        // deserialize labels
        let label_ids = LabelIdListRef {
            data: label_block,
            len: label_len,
        };
        // deserialize properties
        let prop_map = PropertyMapRef::new(prop_block);
        Ok((label_ids, prop_map))
    }
}

pub struct LabelIdListRef<'a> {
    // # layout: | u16_le | u16_le | u16_le | ... |
    //           ^
    //         data
    data: &'a [u8],
    len: usize,
}

impl<'a> LabelIdListRef<'a> {
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn iter(&self) -> impl Iterator<Item = LabelId> {
        (0..self.len).map(move |i| LabelId::from_le_bytes(self.data[i * 2..i * 2 + 2].try_into().unwrap()))
    }
}
