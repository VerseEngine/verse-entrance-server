use crate::ids::*;
use crate::state::{ClientData, State};
use crate::swarm::on_swarm_message;
use anyhow::Result;
use prost::Message;
use std::io::Cursor;
use std::sync::Arc;
use verse_proto::rpc::*;
use verse_proto::rpc::{rpc_packet, RpcPacket};
use verse_proto::swarm::*;

pub async fn on_rtc_message(state: Arc<State>, cd: Arc<ClientData>, data: Vec<u8>) -> Result<()> {
    let packet = RpcPacket::decode_packet(&data)?;
    let Some(rpc_packet::Data::Request(req)) = packet.data else {
        // bad request
        return Ok(());
    };
    if req.rpc_id == RPC_ID_KEEP_ALIVE {
        return Ok(());
    }
    let res = match req.rpc_id {
        RPC_ID_SWARM => {
            let res = on_swarm_message(
                state.clone(),
                cd.clone(),
                SwarmPacket::decode(Cursor::new(&req.param))?,
            )
            .await?;
            res.map(|res| res.encode_to_vec())
        }
        _ => None,
    };
    if let Some(res) = res {
        cd.send_rpc_response(req.rpc_id, res).await?;
    };
    Ok(())
}
