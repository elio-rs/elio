use bytes::{BufMut, Bytes, BytesMut};
use elio_common::{LabelId, NodeId};

use crate::kv::graph_keys::LABEL_INDEX_PREFIX;

pub struct LabelIndexCodec;

impl LabelIndexCodec {
    /// Encode a label index key: `[0x01][label_id u16 LE][node_id u64 BE]`
    pub fn encode_key(label_id: LabelId, node_id: NodeId) -> Bytes {
        let mut key = BytesMut::with_capacity(1 + 2 + 8);
        key.put_u8(LABEL_INDEX_PREFIX);
        key.put_u16_le(label_id);
        key.put_u64(*node_id);
        key.freeze()
    }

    /// Encode a label index prefix for prefix scan: `[0x01][label_id u16 LE]`
    pub fn encode_prefix(label_id: LabelId) -> Bytes {
        let mut key = BytesMut::with_capacity(1 + 2);
        key.put_u8(LABEL_INDEX_PREFIX);
        key.put_u16_le(label_id);
        key.freeze()
    }

    /// Decode a label index key into (label_id, node_id)
    pub fn decode_key(buf: &[u8]) -> (LabelId, NodeId) {
        assert_eq!(buf.len(), 11);
        assert_eq!(buf[0], LABEL_INDEX_PREFIX);
        let label_id = u16::from_le_bytes(buf[1..3].try_into().unwrap());
        let node_id = NodeId::from_be_bytes(buf[3..11].try_into().unwrap());
        (label_id, node_id)
    }
}
