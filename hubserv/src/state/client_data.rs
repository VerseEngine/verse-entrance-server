use anyhow::Result;
#[allow(unused_imports)]
use log::{debug, error, info, warn};
use once_cell::race::OnceBox;
use parking_lot::Mutex;
use std::sync::Arc;
use verse_common::prelude::*;
use verse_proto::rpc::*;
use verse_proto::rpc::{rpc_packet, RpcPacket, RpcResponse};
use verse_proto::swarm::*;
use webrtc::data_channel::data_channel_message::DataChannelMessage;
use webrtc::data_channel::RTCDataChannel;
use webrtc::ice_transport::ice_candidate::RTCIceCandidateInit;
use webrtc::peer_connection::RTCPeerConnection;

pub struct ClientData {
    pub session_id: verse_session_id::SessionId,
    pc: Arc<RTCPeerConnection>,
    dc: OnceBox<Arc<RTCDataChannel>>,
    pub url: String,
    routing_info: Mutex<Option<Arc<RoutingInfo>>>,
}
impl Drop for ClientData {
    fn drop(&mut self) {
        debug!(
            "drop client {}, {}",
            Arc::strong_count(&self.pc),
            self.dc.get().map_or_else(|| 0, Arc::strong_count),
        );
    }
}

impl ClientData {
    pub fn new(
        session_id: verse_session_id::SessionId,
        pc: Arc<RTCPeerConnection>,
        url: String,
    ) -> Arc<Self> {
        Arc::new(ClientData {
            session_id,
            pc,
            dc: Default::default(),
            url,
            routing_info: Mutex::new(None),
        })
    }
    pub fn get_dc(&self) -> Option<Arc<RTCDataChannel>> {
        self.dc.get().cloned()
    }
    pub fn set_dc(&self, dc: Arc<RTCDataChannel>) {
        if self.dc.set(Box::new(dc)).is_err() {
            error!("dc already set");
        }
    }
    pub fn set_routing_info(&self, mut ri: RoutingInfo) {
        ri.set_count(ri.get_relation_count() as u32);
        ri.known_gateway_session_ids.clear();
        *self.routing_info.lock() = Some(Arc::new(ri));
    }
    pub fn get_routing_info(&self) -> Option<Arc<RoutingInfo>> {
        self.routing_info.lock().as_ref().cloned()
    }
    pub async fn add_ice_candidate(&self, candidate: RTCIceCandidateInit) -> Result<()> {
        self.pc
            .add_ice_candidate(candidate)
            .await
            .map_err(anyhow::Error::from)
    }
    pub fn dispose(&self) {
        {
            self.pc
                .on_data_channel(Box::new(move |_: Arc<RTCDataChannel>| {
                    Box::pin(async move {})
                }));
            if let Some(dc) = self.get_dc() {
                dc.on_message(Box::new(move |_: DataChannelMessage| {
                    Box::pin(async move {})
                }));
            }
        }

        let pc = self.pc.clone();
        let dc = self.get_dc();
        if let Some(dc) = dc {
            let dc = Arc::downgrade(&dc);
            tokio::spawn(async move {
                if let Some(dc) = dc.upgrade() {
                    dc.close()
                        .await
                        .map_err(anyhow::Error::from)
                        .if_err_info(logmsg!("can't close dc"));
                }
                pc.close()
                    .await
                    .map_err(anyhow::Error::from)
                    .if_err_info(logmsg!("can't close pc"));
                let mut dcc = 0;
                if let Some(dc) = dc.upgrade() {
                    dcc = Arc::strong_count(&dc);
                }
                let pcc = Arc::strong_count(&pc);
                if pcc > 1 || dcc > 0 {
                    warn!("pc closed and leak {}, {}", pcc, dcc);
                } else {
                    debug!("pc closed {}, {}", pcc, dcc);
                }
            });
        } else {
            tokio::spawn(async move {
                pc.close()
                    .await
                    .map_err(anyhow::Error::from)
                    .if_err_info(logmsg!("can't close pc"));
                let pcc = Arc::strong_count(&pc);
                if pcc > 1 {
                    warn!("pc closed and leak {}, none", pcc);
                } else {
                    debug!("pc closed {}, none", pcc);
                }
            });
        }
    }

    pub async fn send_rpc_response(&self, rpc_id: u32, param: Vec<u8>) -> Result<bool> {
        let res_packet = RpcPacket {
            data: Some(rpc_packet::Data::Response(RpcResponse { rpc_id, param })),
            ..Default::default()
        }
        .encode_packet();
        if res_packet.len() > 65535 {
            error!("large data: {}", res_packet.len());
        }

        if let Some(dc) = self.get_dc() {
            dc.send(&bytes::Bytes::from(res_packet)).await?;
        } else {
            return Ok(false);
        }

        Ok(true)
    }
}
