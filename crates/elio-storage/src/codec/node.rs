//! Adaptive node storage codec.
//!
//! Graph tree key: `[node_id: u64 BE][segment: u8]`
//! - segment 0x00: header (labels + properties, and edges in packed mode)
//! - segment 0x01: out edges (split mode only)
//! - segment 0x02: in edges (split mode only)
//!
//! Packed mode (flags=0x00, total < 4KB):
//!   `[flags:u8][num_labels:u16 LE][label_ids...][prop_size:u32 LE][prop_bytes...]`
//!   `[out_edge_count:u32 LE][out_edges...][in_edge_count:u32 LE][in_edges...]`
//!
//! Split mode (flags=0x01):
//!   segment 0x00: `[flags:u8][num_labels:u16 LE][label_ids...][prop_size:u32 LE][prop_bytes...]`
//!   segment 0x01: `[edge_count:u32 LE][out_edges...]`
//!   segment 0x02: `[edge_count:u32 LE][in_edges...]`
//!
//! EdgeEntry: `[rel_id:u64 LE][other_node_id:u64 LE][rel_type_id:u16 LE][prop_len:u32 LE][prop_bytes...]`

use bytes::{BufMut, Bytes, BytesMut};
use elio_common::mapb::{PropertyMapMut, PropertyMapRef};
use elio_common::scalar::StructValueRef;
use elio_common::{LabelId, NodeId, RelationshipId, TokenId};

use crate::kv::{ADAPTIVE_THRESHOLD, graph_keys, node_flags};

/// Minimum edge entry size (rel_id + other_node_id + rel_type_id + prop_len)
pub const EDGE_ENTRY_MIN_SIZE: usize = 8 + 8 + 2 + 4;

/// An edge entry: relationship info + inline properties
#[derive(Debug, Clone)]
pub struct EdgeEntry {
    pub rel_id: RelationshipId,
    pub other_node_id: NodeId,
    pub rel_type_id: TokenId,
    pub prop_bytes: Bytes,
}

impl EdgeEntry {
    pub fn encoded_size(&self) -> usize {
        EDGE_ENTRY_MIN_SIZE + self.prop_bytes.len()
    }

    pub fn encode_to(&self, buf: &mut BytesMut) {
        buf.put_u64_le(*self.rel_id);
        buf.put_u64_le(*self.other_node_id);
        buf.put_u16_le(self.rel_type_id);
        buf.put_u32_le(self.prop_bytes.len() as u32);
        buf.put_slice(&self.prop_bytes);
    }

    pub fn decode(data: &[u8]) -> Result<(Self, usize), String> {
        if data.len() < EDGE_ENTRY_MIN_SIZE {
            return Err("buffer too short for edge entry".into());
        }
        let rel_id = RelationshipId::from(u64::from_le_bytes(data[0..8].try_into().unwrap()));
        let other_node_id = NodeId::from_le_bytes(data[8..16].try_into().unwrap());
        let rel_type_id = u16::from_le_bytes(data[16..18].try_into().unwrap());
        let prop_len = u32::from_le_bytes(data[18..22].try_into().unwrap()) as usize;

        let total = EDGE_ENTRY_MIN_SIZE + prop_len;
        if data.len() < total {
            return Err(format!(
                "buffer too short for edge props: need {total}, have {}",
                data.len()
            ));
        }
        let prop_bytes = Bytes::copy_from_slice(&data[22..22 + prop_len]);
        Ok((
            Self {
                rel_id,
                other_node_id,
                rel_type_id,
                prop_bytes,
            },
            total,
        ))
    }
}

/// Graph tree key codec
pub struct GraphKeyCodec;

impl GraphKeyCodec {
    /// Encode a graph key: [node_id BE][segment]
    pub fn encode(node_id: NodeId, segment: u8) -> [u8; 9] {
        let mut key = [0u8; 9];
        key[..8].copy_from_slice(&node_id.to_be_bytes());
        key[8] = segment;
        key
    }

    pub fn encode_header_key(node_id: NodeId) -> [u8; 9] {
        Self::encode(node_id, graph_keys::SEGMENT_HEADER)
    }

    pub fn encode_out_edges_key(node_id: NodeId) -> [u8; 9] {
        Self::encode(node_id, graph_keys::SEGMENT_OUT_EDGES)
    }

    pub fn encode_in_edges_key(node_id: NodeId) -> [u8; 9] {
        Self::encode(node_id, graph_keys::SEGMENT_IN_EDGES)
    }

    /// Decode node_id and segment from a 9-byte key
    pub fn decode(key: &[u8]) -> (NodeId, u8) {
        assert!(key.len() >= 9);
        let node_id = NodeId::from_be_bytes(key[0..8].try_into().unwrap());
        let segment = key[8];
        (node_id, segment)
    }

    /// Prefix for scanning all segments of a node
    pub fn node_prefix(node_id: NodeId) -> [u8; 8] {
        node_id.to_be_bytes()
    }
}

/// Adaptive node codec
pub struct AdaptiveNodeCodec;

impl AdaptiveNodeCodec {
    /// Encode a node in packed mode (header + props + edges all in one value).
    /// Returns None if the total size exceeds the threshold (caller should use split mode).
    pub fn try_encode_packed(
        labels: &[LabelId],
        prop_bytes: &[u8],
        out_edges: &[EdgeEntry],
        in_edges: &[EdgeEntry],
    ) -> Option<Bytes> {
        // Estimate size
        let header_size = 1 + 2 + labels.len() * 2 + 4 + prop_bytes.len();
        let out_edges_size: usize = 4 + out_edges.iter().map(|e| e.encoded_size()).sum::<usize>();
        let in_edges_size: usize = 4 + in_edges.iter().map(|e| e.encoded_size()).sum::<usize>();
        let total = header_size + out_edges_size + in_edges_size;

        if total >= ADAPTIVE_THRESHOLD {
            return None;
        }

        let mut buf = BytesMut::with_capacity(total);
        // flags
        buf.put_u8(node_flags::PACKED);
        // labels
        buf.put_u16_le(labels.len() as u16);
        for label_id in labels {
            buf.put_u16_le(*label_id);
        }
        // props
        buf.put_u32_le(prop_bytes.len() as u32);
        buf.put_slice(prop_bytes);
        // out edges
        buf.put_u32_le(out_edges.len() as u32);
        for edge in out_edges {
            edge.encode_to(&mut buf);
        }
        // in edges
        buf.put_u32_le(in_edges.len() as u32);
        for edge in in_edges {
            edge.encode_to(&mut buf);
        }

        Some(buf.freeze())
    }

    /// Encode header-only value (split mode): flags + labels + props
    pub fn encode_header(labels: &[LabelId], prop_bytes: &[u8]) -> Bytes {
        let mut buf = BytesMut::with_capacity(1 + 2 + labels.len() * 2 + 4 + prop_bytes.len());
        buf.put_u8(node_flags::SPLIT);
        buf.put_u16_le(labels.len() as u16);
        for label_id in labels {
            buf.put_u16_le(*label_id);
        }
        buf.put_u32_le(prop_bytes.len() as u32);
        buf.put_slice(prop_bytes);
        buf.freeze()
    }

    /// Encode an edge list value: [edge_count:u32 LE][edges...]
    pub fn encode_edge_list(edges: &[EdgeEntry]) -> Bytes {
        let total: usize = 4 + edges.iter().map(|e| e.encoded_size()).sum::<usize>();
        let mut buf = BytesMut::with_capacity(total);
        buf.put_u32_le(edges.len() as u32);
        for edge in edges {
            edge.encode_to(&mut buf);
        }
        buf.freeze()
    }

    /// Decode header from segment 0x00 value.
    /// Returns (is_packed, labels, prop_bytes, remaining_data_for_edges_if_packed)
    pub fn decode_header(buf: &[u8]) -> Result<(bool, LabelIdListRef<'_>, &[u8], &[u8]), String> {
        if buf.is_empty() {
            return Err("empty buffer".into());
        }
        let flags = buf[0];
        let is_packed = flags == node_flags::PACKED;

        if buf.len() < 4 {
            return Err("buffer too short for header".into());
        }
        let num_labels = u16::from_le_bytes(buf[1..3].try_into().unwrap()) as usize;
        let labels_end = 3 + num_labels * 2;
        if buf.len() < labels_end + 4 {
            return Err("buffer too short for labels+prop_size".into());
        }

        let label_data = &buf[3..labels_end];
        let prop_size = u32::from_le_bytes(buf[labels_end..labels_end + 4].try_into().unwrap()) as usize;
        let prop_start = labels_end + 4;
        let prop_end = prop_start + prop_size;

        if buf.len() < prop_end {
            return Err("buffer too short for properties".into());
        }

        let prop_bytes = &buf[prop_start..prop_end];
        let remaining = &buf[prop_end..];

        let labels = LabelIdListRef {
            data: label_data,
            len: num_labels,
        };

        Ok((is_packed, labels, prop_bytes, remaining))
    }

    /// Decode edges from the packed remainder (after props in packed mode).
    /// Format: [out_count:u32][out_edges...][in_count:u32][in_edges...]
    pub fn decode_packed_edges(data: &[u8]) -> Result<(Vec<EdgeEntry>, Vec<EdgeEntry>), String> {
        if data.len() < 4 {
            return Err("buffer too short for out_edge_count".into());
        }
        let out_count = u32::from_le_bytes(data[0..4].try_into().unwrap()) as usize;
        let mut offset = 4;
        let mut out_edges = Vec::with_capacity(out_count);
        for _ in 0..out_count {
            let (entry, size) = EdgeEntry::decode(&data[offset..])?;
            out_edges.push(entry);
            offset += size;
        }

        if data.len() < offset + 4 {
            return Err("buffer too short for in_edge_count".into());
        }
        let in_count = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap()) as usize;
        offset += 4;
        let mut in_edges = Vec::with_capacity(in_count);
        for _ in 0..in_count {
            let (entry, size) = EdgeEntry::decode(&data[offset..])?;
            in_edges.push(entry);
            offset += size;
        }

        Ok((out_edges, in_edges))
    }

    /// Decode edge list from segment 0x01 or 0x02 value (split mode).
    /// Format: [edge_count:u32][edges...]
    pub fn decode_edge_list(data: &[u8]) -> Result<Vec<EdgeEntry>, String> {
        if data.len() < 4 {
            return Err("buffer too short for edge_count".into());
        }
        let count = u32::from_le_bytes(data[0..4].try_into().unwrap()) as usize;
        let mut offset = 4;
        let mut edges = Vec::with_capacity(count);
        for _ in 0..count {
            let (entry, size) = EdgeEntry::decode(&data[offset..])?;
            edges.push(entry);
            offset += size;
        }
        Ok(edges)
    }

    /// Encode property map to bytes (used by both node create and relationship create)
    pub fn encode_property_value(key_ids: &[TokenId], property_map: StructValueRef<'_>) -> Result<Bytes, String> {
        let mut buf = BytesMut::new();
        let mut mapb_mut = PropertyMapMut::with_capacity(key_ids.len());

        assert_eq!(key_ids.len(), property_map.len());

        for (key_id, (_, prop)) in key_ids.iter().zip(property_map.iter()) {
            mapb_mut.insert(*key_id, Some(&prop))?;
        }
        mapb_mut.freeze().write(&mut buf);
        Ok(buf.freeze())
    }

    /// Decode property map from bytes
    pub fn decode_property_map(buf: &[u8]) -> PropertyMapRef<'_> {
        PropertyMapRef::new(buf)
    }
}

pub struct LabelIdListRef<'a> {
    data: &'a [u8],
    len: usize,
}

impl<'a> LabelIdListRef<'a> {
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn iter(&self) -> impl Iterator<Item = LabelId> + '_ {
        (0..self.len).map(move |i| LabelId::from_le_bytes(self.data[i * 2..i * 2 + 2].try_into().unwrap()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edge_entry_encode_decode() {
        let entry = EdgeEntry {
            rel_id: RelationshipId::from(42),
            other_node_id: NodeId::from(99),
            rel_type_id: 7,
            prop_bytes: Bytes::from_static(&[1, 2, 3]),
        };
        let mut buf = BytesMut::new();
        entry.encode_to(&mut buf);
        let data = buf.freeze();

        let (decoded, size) = EdgeEntry::decode(&data).unwrap();
        assert_eq!(size, data.len());
        assert_eq!(*decoded.rel_id, 42);
        assert_eq!(*decoded.other_node_id, 99);
        assert_eq!(decoded.rel_type_id, 7);
        assert_eq!(&decoded.prop_bytes[..], &[1, 2, 3]);
    }

    #[test]
    fn test_packed_encode_decode_roundtrip() {
        let labels = vec![1u16, 2u16];
        let prop_bytes = vec![10u8, 20, 30];
        let out_edges = vec![EdgeEntry {
            rel_id: RelationshipId::from(1),
            other_node_id: NodeId::from(2),
            rel_type_id: 3,
            prop_bytes: Bytes::new(),
        }];
        let in_edges = vec![EdgeEntry {
            rel_id: RelationshipId::from(4),
            other_node_id: NodeId::from(5),
            rel_type_id: 6,
            prop_bytes: Bytes::from_static(&[7, 8]),
        }];

        let packed = AdaptiveNodeCodec::try_encode_packed(&labels, &prop_bytes, &out_edges, &in_edges).unwrap();
        let (is_packed, label_ref, props, remaining) = AdaptiveNodeCodec::decode_header(&packed).unwrap();
        assert!(is_packed);
        assert_eq!(label_ref.len(), 2);
        let label_vec: Vec<_> = label_ref.iter().collect();
        assert_eq!(label_vec, vec![1, 2]);
        assert_eq!(props, &[10, 20, 30]);

        let (decoded_out, decoded_in) = AdaptiveNodeCodec::decode_packed_edges(remaining).unwrap();
        assert_eq!(decoded_out.len(), 1);
        assert_eq!(*decoded_out[0].rel_id, 1);
        assert_eq!(decoded_in.len(), 1);
        assert_eq!(*decoded_in[0].rel_id, 4);
        assert_eq!(&decoded_in[0].prop_bytes[..], &[7, 8]);
    }

    #[test]
    fn test_split_encode_decode_roundtrip() {
        let labels = vec![10u16];
        let prop_bytes = vec![0u8; 100];

        let header = AdaptiveNodeCodec::encode_header(&labels, &prop_bytes);
        let (is_packed, label_ref, props, remaining) = AdaptiveNodeCodec::decode_header(&header).unwrap();
        assert!(!is_packed);
        assert_eq!(label_ref.len(), 1);
        assert_eq!(props.len(), 100);
        assert!(remaining.is_empty());

        let edges = vec![
            EdgeEntry {
                rel_id: RelationshipId::from(10),
                other_node_id: NodeId::from(20),
                rel_type_id: 30,
                prop_bytes: Bytes::new(),
            },
            EdgeEntry {
                rel_id: RelationshipId::from(11),
                other_node_id: NodeId::from(21),
                rel_type_id: 31,
                prop_bytes: Bytes::from_static(&[1, 2, 3, 4]),
            },
        ];
        let edge_data = AdaptiveNodeCodec::encode_edge_list(&edges);
        let decoded = AdaptiveNodeCodec::decode_edge_list(&edge_data).unwrap();
        assert_eq!(decoded.len(), 2);
        assert_eq!(*decoded[0].rel_id, 10);
        assert_eq!(*decoded[1].rel_id, 11);
    }

    #[test]
    fn test_graph_key_encode_decode() {
        let node_id = NodeId::from(12345);
        let key = GraphKeyCodec::encode_header_key(node_id);
        let (decoded_id, segment) = GraphKeyCodec::decode(&key);
        assert_eq!(decoded_id, node_id);
        assert_eq!(segment, graph_keys::SEGMENT_HEADER);
    }

    #[test]
    fn test_threshold_triggers_none() {
        let labels = vec![1u16];
        let prop_bytes = vec![0u8; 100];
        // Create enough edges to exceed 4KB
        let big_edges: Vec<EdgeEntry> = (0..200)
            .map(|i| EdgeEntry {
                rel_id: RelationshipId::from(i as u64),
                other_node_id: NodeId::from(i as u64 + 1000),
                rel_type_id: 1,
                prop_bytes: Bytes::new(),
            })
            .collect();

        let result = AdaptiveNodeCodec::try_encode_packed(&labels, &prop_bytes, &big_edges, &[]);
        assert!(result.is_none(), "should return None when exceeding threshold");
    }
}
