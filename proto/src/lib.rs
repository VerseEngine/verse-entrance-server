pub mod primitive {
    #![allow(clippy::all)]
    include!(concat!(env!("OUT_DIR"), "/primitive.rs"));

    pub trait IPosition3D {
        fn from_xyz(x: f64, y: f32, z: f64) -> Self;

        fn copy_from(&mut self, src: &Position3D);
    }
    impl IPosition3D for Position3D {
        fn from_xyz(x: f64, y: f32, z: f64) -> Self {
            Position3D { x, y, z }
        }
        fn copy_from(&mut self, src: &Position3D) {
            self.x = src.x;
            self.y = src.y;
            self.z = src.z;
        }
    }
}
pub mod person {
    #![allow(clippy::all)]
    include!(concat!(env!("OUT_DIR"), "/person.rs"));
}
pub mod rpc {
    #![allow(clippy::all)]
    include!(concat!(env!("OUT_DIR"), "/rpc.rs"));

    use anyhow::Result;
    #[allow(unused_imports)]
    use log::{debug, error, info, trace, warn};
    use prost::Message;
    use std::io::Cursor;
    use verse_common::compress::{compress_if_needed, decompress};

    pub trait IRpcPacket {
        fn set_request(&mut self, v: RpcRequest);
        fn set_response(&mut self, v: RpcResponse);
        fn encode_packet(&mut self) -> Vec<u8>;
        fn decode_packet(data: &[u8]) -> Result<Self>
        where
            Self: Default;
    }
    impl IRpcPacket for RpcPacket {
        fn set_request(&mut self, v: RpcRequest) {
            self.data = Some(rpc_packet::Data::Request(v));
        }
        fn set_response(&mut self, v: RpcResponse) {
            self.data = Some(rpc_packet::Data::Response(v));
        }
        fn encode_packet(&mut self) -> Vec<u8> {
            if !self.is_compressed {
                match self.data {
                    Some(rpc_packet::Data::Request(ref mut r)) => {
                        if let Ok(Some(compressed)) = compress_if_needed(&mut r.param) {
                            r.param = compressed;
                            self.is_compressed = true;
                        }
                    }
                    Some(rpc_packet::Data::Response(ref mut r)) => {
                        if let Ok(Some(compressed)) = compress_if_needed(&mut r.param) {
                            r.param = compressed;
                            self.is_compressed = true;
                        }
                    }
                    _ => {}
                }
            }
            self.encode_to_vec()
        }
        fn decode_packet(data: &[u8]) -> Result<Self>
        where
            Self: Default,
        {
            let mut p = RpcPacket::decode(Cursor::new(data))?;
            if p.is_compressed {
                match p.data {
                    Some(rpc_packet::Data::Request(ref mut r)) => {
                        if let Ok(decompressed) = decompress(&mut r.param) {
                            r.param = decompressed;
                            p.is_compressed = false;
                        }
                    }
                    Some(rpc_packet::Data::Response(ref mut r)) => {
                        if let Ok(decompressed) = decompress(&mut r.param) {
                            r.param = decompressed;
                            p.is_compressed = false;
                        }
                    }
                    _ => {}
                }
            }
            Ok(p)
        }
    }
}
pub mod signaling {
    #![allow(clippy::all)]
    include!(concat!(env!("OUT_DIR"), "/signaling.rs"));

    pub trait ITransferPayload {
        fn set_encrypted_payload(&mut self, v: Vec<u8>);
    }
    impl ITransferPayload for TransferPayload {
        fn set_encrypted_payload(&mut self, v: Vec<u8>) {
            self.data = Some(transfer_payload::Data::EncryptedPayload(v));
        }
    }
}

mod routing_info_ex;

pub mod swarm {
    #![allow(clippy::all)]
    include!(concat!(env!("OUT_DIR"), "/swarm.rs"));

    pub use super::routing_info_ex::*;

    pub trait IRoutingInfo {
        // fn count(&self) -> u32;
        fn set_count(&mut self, v: u32);
        // fn routing_infos(&self) -> &RoutingInfos;
        // fn set_routing_infos(&mut self, v: RoutingInfos);
        fn has_count(&self) -> bool;
        fn has_routing_infos(&self) -> bool;
    }
    /* pub trait IRoutingInfos {
        fn default_instance() -> &'static RoutingInfos;
    } */
    impl AsRef<RoutingInfo> for RoutingInfo {
        fn as_ref(&self) -> &RoutingInfo {
            self
        }
    }

    impl IRoutingInfo for RoutingInfo {
        fn has_count(&self) -> bool {
            match self.relation {
                Some(routing_info::Relation::Count(_)) => true,
                _ => false,
            }
        }
        fn has_routing_infos(&self) -> bool {
            match self.relation {
                Some(routing_info::Relation::RoutingInfos(_)) => true,
                _ => false,
            }
        }
        /* fn count(&self) -> u32 {
            match self.relation {
                Some(routing_info::Relation::RoutingInfos(ref v)) => v.infos.len() as u32,
                Some(routing_info::Relation::Count(v)) => v,
                _ => 0,
            }
        } */
        fn set_count(&mut self, v: u32) {
            self.relation = Some(routing_info::Relation::Count(v));
        }
        /* fn routing_infos(&self) -> &RoutingInfos {
            match self.relation {
                Some(routing_info::Relation::RoutingInfos(ref v)) => v,
                _ => RoutingInfos::default_instance(),
            }
        } */
        /* fn set_routing_infos(&mut self, v: RoutingInfos) {
            self.relation = Some(routing_info::Relation::RoutingInfos(v));
        } */
    }
    /* impl IRoutingInfos for RoutingInfos {
        fn default_instance() -> &'static RoutingInfos {
            static INSTANCE: RoutingInfos = RoutingInfos { infos: vec![] };
            &INSTANCE
        }
    } */

    impl TryFrom<&SignatureSet> for verse_session_id::SignatureSet {
        type Error = anyhow::Error;
        fn try_from(value: &SignatureSet) -> Result<Self, Self::Error> {
            Ok(verse_session_id::SignatureSet {
                signature: value.signature.clone().try_into().map_err(|_v| {
                    anyhow::anyhow!("signature convert: {0}:{1}", file!().to_string(), line!())
                })?,
                salt: value.salt.clone().try_into().map_err(|_v| {
                    anyhow::anyhow!("signature convert: {0}:{1}", file!().to_string(), line!())
                })?,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_position3d() {
        use primitive::*;
        let v0 = Position3D::from_xyz(1.0, 2.0, 3.0);
        let mut v1 = Position3D::default();
        assert_eq!(v0.x, 1.0);
        assert_eq!(v0.y, 2.0);
        assert_eq!(v0.z, 3.0);
        assert_ne!(v0, v1);
        v1.copy_from(&v0);
        assert_eq!(v0, v1);
    }
    #[test]
    fn test_rpcpacket() {
        use rpc::*;
        let mut v0: RpcPacket = Default::default();
        v0.set_request(RpcRequest {
            rpc_id: 1,
            param: Default::default(),
        });
        let Some(rpc_packet::Data::Request(ref req)) = v0.data else {
            unreachable!();
        };
        assert_eq!(req.rpc_id, 1);

        v0.set_response(RpcResponse {
            rpc_id: 2,
            param: Default::default(),
        });
        let Some(rpc_packet::Data::Response(ref req)) = v0.data else {
            unreachable!();
        };
        assert_eq!(req.rpc_id, 2);
    }
    #[test]
    fn test_rpcpacket_encode() {
        use rpc::*;
        {
            let mut p: RpcPacket = Default::default();
            let r = RpcRequest {
                rpc_id: 1,
                param: [1u8; 1024].to_vec(),
            };
            p.set_request(r.clone());
            let bin = p.encode_packet();
            assert!(bin.len() < 1024);
            let p = RpcPacket::decode_packet(&bin);
            assert!(p.is_ok());
            let p = p.unwrap();
            assert!(matches!(p.data.unwrap(), rpc_packet::Data::Request(r0) if r0 == r));
        }
        {
            let mut p: RpcPacket = Default::default();
            let r = RpcResponse {
                rpc_id: 1,
                param: [1u8; 1024].to_vec(),
            };
            p.set_response(r.clone());
            let bin = p.encode_packet();
            assert!(bin.len() < 1024);
            let p = RpcPacket::decode_packet(&bin);
            assert!(p.is_ok());
            let p = p.unwrap();
            assert!(matches!(p.data.unwrap(), rpc_packet::Data::Response(r0) if r0 == r));
        }
    }
    #[test]
    fn test_signaling() {
        use signaling::*;
        let mut v: TransferPayload = Default::default();
        v.set_encrypted_payload(vec![1, 2, 3]);
        let Some(transfer_payload::Data::EncryptedPayload(ref req)) = v.data else {
            unreachable!();
        };
        assert_eq!(req.clone(), vec![1u8, 2u8, 3u8]);
    }
    #[test]
    fn test_swarm() {
        use swarm::*;
        let mut v: RoutingInfo = Default::default();
        assert!(!v.has_count());
        v.set_count(10);
        assert!(v.has_count());
        assert_eq!(v.get_relation_count(), 10);

        assert!(!v.has_routing_infos());
        assert_eq!(v.as_ref(), &v);
    }
    #[test]
    fn test_ss() {
        use swarm::*;
        let ss = SignatureSet {
            signature: [1; verse_session_id::SIGNATURE_SIZE].to_vec(),
            salt: [2; verse_session_id::SIGNATURE_SALT_SIZE].to_vec(),
            ..Default::default()
        };
        let res = verse_session_id::SignatureSet::try_from(&ss);
        assert!(res.is_ok());
        let res = res.unwrap();
        assert_eq!(res.signature.to_vec(), ss.signature);
        assert_eq!(res.salt.to_vec(), ss.salt);

        let ss = SignatureSet {
            signature: [1; verse_session_id::SIGNATURE_SIZE - 1].to_vec(),
            salt: [2; verse_session_id::SIGNATURE_SALT_SIZE].to_vec(),
            ..Default::default()
        };
        let res = verse_session_id::SignatureSet::try_from(&ss);
        assert!(res.is_err());

        let ss = SignatureSet {
            signature: [1; verse_session_id::SIGNATURE_SIZE].to_vec(),
            salt: [2; verse_session_id::SIGNATURE_SALT_SIZE - 1].to_vec(),
            ..Default::default()
        };
        let res = verse_session_id::SignatureSet::try_from(&ss);
        assert!(res.is_err());
    }
}
