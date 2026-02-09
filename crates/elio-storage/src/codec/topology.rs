//! topology storage.
//! Here we inline relationship properties with topology.
//!
//! TopologyKey ::= TOPO_KEY_PREFIX SrcNodeId Direction RelTypeId DstNodeId RelationshipId
//! TopologyValue ::= [PropertyBlock]

use bytemuck::{Pod, Zeroable};
use bytes::{BufMut, Bytes, BytesMut};
use elio_common::store_types::RelDirection;
use elio_common::{NodeId, RelationshipId, RelationshipTypeId};

use crate::codec::TOPO_KEY_PREFIX;

const INCOMING_SUFFIX: u8 = 0x01;
const OUTGOING_SUFFIX: u8 = 0x02;

pub struct TopoFormat;

impl TopoFormat {
    pub fn encode_topo_key(
        src_id: NodeId,
        dir: RelDirection,
        reltype: RelationshipTypeId,
        dst_id: NodeId,
        relid: RelationshipId,
    ) -> Bytes {
        let mut bytes = BytesMut::new();
        bytes.put_u8(TOPO_KEY_PREFIX);
        bytes.put_u64_le(src_id);
        match dir {
            RelDirection::Incoming => bytes.put_u8(INCOMING_SUFFIX),
            RelDirection::Outgoing => bytes.put_u8(OUTGOING_SUFFIX),
        }
        bytes.put_u16(reltype);
        bytes.put_u64_le(dst_id);
        bytes.put_u64_le(relid);
        bytes.freeze()
    }

    pub fn decode_topo_key(buf: &[u8]) -> (NodeId, RelDirection, RelationshipTypeId, NodeId, RelationshipId) {
        assert_eq!(buf.len(), 28);
        let src_id = u64::from_le_bytes(buf[1..9].try_into().unwrap());
        let dir = match buf[9] {
            INCOMING_SUFFIX => RelDirection::Incoming,
            OUTGOING_SUFFIX => RelDirection::Outgoing,
            _ => panic!("invalid direction suffix"),
        };
        let rel_type = u16::from_le_bytes(buf[10..12].try_into().unwrap());
        let dst_id = u64::from_le_bytes(buf[12..20].try_into().unwrap());
        let relid = u64::from_le_bytes(buf[20..28].try_into().unwrap());
        (src_id, dir, rel_type, dst_id, relid)
    }
}

#[repr(C, packed(1))]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct TopoHeader {
    pub num: u32,
}

pub struct TopoFormatWriter<'a> {
    buf: &'a mut BytesMut,
    header: &'a mut TopoHeader,
}

impl<'a> TopoFormatWriter<'a> {
    pub fn new(buf: &'a mut BytesMut) -> Self {
        if buf.is_empty() {
            // initialize header
            let topo = TopoHeader::zeroed();
            buf.put_slice(bytemuck::bytes_of(&topo));
        }
        if buf.len() != size_of::<TopoHeader>() {
            panic!("topology value must be {} bytes", size_of::<TopoHeader>());
        }
        let header_buf = buf.as_mut_ptr() as *mut TopoHeader;
        let header = unsafe { &mut *header_buf };
        Self { buf, header }
    }

    pub fn add_relationship(&mut self, rel_id: RelationshipId, other_node_id: NodeId) {
        self.buf.put_u64_le(rel_id);
        self.buf.put_u64_le(other_node_id);
        self.header.num += 1;
    }

    pub fn delete_relationship(&mut self, _rel_id: RelationshipId) {
        // TODO: implement delete relationship
    }
}
