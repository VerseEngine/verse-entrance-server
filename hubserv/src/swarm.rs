use crate::ids::*;
use crate::state::{ClientData, State};
use anyhow::Result;
#[allow(unused_imports)]
use log::{debug, error, info, warn};
use prost::Message;
use std::io::Cursor;
use std::sync::Arc;
use verse_proto::swarm::*;
use verse_session_id::*;

pub async fn on_swarm_message(
    state: Arc<State>,
    cd: Arc<ClientData>,
    packet: SwarmPacket,
) -> Result<Option<SwarmPacket>> {
    let Some(swarm_packet::Data::Request(req)) = packet.data else {
        return Ok(None);
    };

    let res = match req.rpc_id {
        RPC_ID_TRANSFER => Some(
            transfer(
                state.clone(),
                cd.clone(),
                TransferRequest::decode(Cursor::new(&req.param))?,
            )
            .await?
            .encode_to_vec(),
        ),
        RPC_ID_EXCHANGE_ROUTING_INFO => Some(
            exchange_routeing_info(
                state.clone(),
                cd.clone(),
                RoutingInfo::decode(Cursor::new(&req.param))?,
            )
            .await?
            .encode_to_vec(),
        ),
        _ => None,
    };
    if let Some(res) = res {
        Ok(Some(SwarmPacket {
            data: Some(swarm_packet::Data::Response(SwarmResponse {
                rpc_id: req.rpc_id,
                param: res,
            })),
        }))
    } else {
        Ok(None)
    }
}
async fn transfer(
    state: Arc<State>,
    cd: Arc<ClientData>,
    req: TransferRequest,
) -> Result<TransferResponse> {
    let to_session_id = req.to_session_id.clone();
    let result = _transfer(state.clone(), cd.clone(), req).await?;
    Ok(TransferResponse {
        result,
        dest_session_id: to_session_id,
    })
}
async fn _transfer(state: Arc<State>, _cd: Arc<ClientData>, req: TransferRequest) -> Result<bool> {
    let to_session_id = SessionId::try_from(&req.to_session_id)?;

    let signature = req
        .signature
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("bad signature"))?;
    let ss = verse_session_id::SignatureSet::try_from(signature)?;
    let from_session_id = SessionId::try_from(&signature.from_session_id)?;
    from_session_id.verify(vec![&req.to_session_id, &req.payload], &ss)?;

    if req.ttl < 1 {
        return Ok(false);
    }
    let req = TransferRequest {
        ttl: req.ttl - 1,
        ..req
    };
    let req = SwarmRequest {
        rpc_id: RPC_ID_TRANSFER,
        param: req.encode_to_vec(),
    };
    let packet = SwarmPacket {
        data: Some(swarm_packet::Data::Request(req)),
    };
    state
        .send_rpc_response(&to_session_id, RPC_ID_SWARM, packet.encode_to_vec())
        .await
}
async fn exchange_routeing_info(
    state: Arc<State>,
    cd: Arc<ClientData>,
    req: RoutingInfo,
) -> Result<RoutingInfo> {
    cd.set_routing_info(req);
    let ud = if let Some(ud) = state.get_url_data(&cd.url) {
        ud
    } else {
        unreachable!("url data not found");
    };

    ud.update_routing_info_if_needed();

    let ri = ud.get_routing_info();

    if let Some(relation) = ri.get_relations() {
        if state.max_routing_results < relation.len() {
            use rand::seq::SliceRandom;
            let rng = &mut rand::thread_rng();
            let relation: Vec<RoutingInfo> = relation
                .choose_multiple(rng, state.max_routing_results)
                .cloned()
                .collect();

            let mut ri1: RoutingInfo = (*ri).clone();
            ri1.set_relations(relation);
            return Ok(ri1);
        }
    }

    Ok((*ri).clone())
}
