use crate::cluster;
use crate::rtc_api;
use crate::state::{ClientData, SharedState};
use crate::types;
use axum::response::IntoResponse;
use axum::response::Response;
use axum::{
    extract::{Host, State},
    http::header::HeaderMap,
    routing::post,
    Json, Router,
};
#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;
use verse_common::prelude::*;
use verse_session_id::SessionId;
use webrtc::data_channel::data_channel_message::DataChannelMessage;
use webrtc::data_channel::RTCDataChannel;
use webrtc::ice_transport::ice_connection_state::RTCIceConnectionState;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::peer_connection::RTCPeerConnection;

// Debugging handler type errors
// https://docs.rs/axum/latest/axum/handler/index.html#debugging-handler-type-errors
// use axum::debug_handler;

pub fn create_router(app_state: &SharedState) -> Router {
    Router::new()
        .route("/enter", post(enter))
        .route("/candidate", post(candidate))
        .with_state(app_state.clone())
}

// #[debug_handler]
async fn enter(
    Host(host): Host,
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<types::SignedRequest>,
) -> Result<Json<types::EnterResponse>, axum::response::Response> {
    let (session_id, payload) = req.verify::<types::EnterRequestPayload>().map_err(|ex| {
        warn!("failed: verify payload: {:?}", ex);
        axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response()
    })?;

    if payload.url.is_empty() || payload.sdp.sdp.is_empty() {
        return Err(axum::http::StatusCode::BAD_REQUEST.into_response());
    }
    if let Some(cluster_client) = state.cluster_client.as_ref() {
        cluster::redirect_if_needed(cluster_client, &host, &payload.url, "/enter")?;
    }

    if !state.is_new_connection_available(&payload.url) {
        return Err(axum::http::StatusCode::SERVICE_UNAVAILABLE.into_response());
    }

    let pc = Arc::new(
        state
            .api
            .new_peer_connection(RTCConfiguration {
                ice_servers: vec![RTCIceServer {
                    // ice liteの場合はice serverを指定しない
                    // urls: state.ice_servers.clone(),
                    ..Default::default()
                }],
                ..Default::default()
            })
            .await
            .map_err(|ex| {
                warn!("failed: new_peer_connection: {:?}", ex);
                axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response()
            })?,
    );

    {
        let raw_url = payload.raw_url.clone();
        let url = payload.url.clone();
        let is_access_log_sended = AtomicU64::new(0);
        {
            let state = state.clone();
            pc.on_ice_connection_state_change(Box::new(move |s: RTCIceConnectionState| {
                if s == RTCIceConnectionState::Disconnected
                    || s == RTCIceConnectionState::Failed
                    || s == RTCIceConnectionState::Closed
                {
                    let state = state.clone();
                    return Box::pin(async move {
                        state.remove_connection(&session_id);
                    });
                }

                Box::pin(async {})
            }));
        }
        let state = state.clone();
        pc.on_peer_connection_state_change(Box::new(move |s: RTCPeerConnectionState| {
            if log::log_enabled!(log::Level::Debug) {
                trace!("state change: {}: {:?}", session_id.to_debug_string(), s);
            }
            if s == RTCPeerConnectionState::Connected
                && is_access_log_sended
                    .compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed)
                    .is_ok()
            {
                let client_count = if let Some(ud) = state.get_url_data(&url) {
                    ud.get_client_count()
                } else {
                    1
                };
                state.append_access_log(client_count, &raw_url, &headers);
            }
            if s == RTCPeerConnectionState::Failed
                || s == RTCPeerConnectionState::Disconnected
                || s == RTCPeerConnectionState::Closed
            {
                let state = state.clone();
                Box::pin(async move {
                    state.remove_connection(&session_id);
                })
            } else {
                Box::pin(async {})
            }
        }));
    }

    let answer = match timeout(
        Duration::from_millis(5000),
        setup_pc(state.clone(), session_id, &pc, payload.sdp),
    )
    .await
    {
        Ok(answer) => answer,
        Err(_) => {
            warn!("pc setup timeout");
            pc.close()
                .await
                .map_err(anyhow::Error::from)
                .if_err_info(logmsg!("can't close pc2"));
            return Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response());
        }
    };
    let answer = match answer {
        Ok(answer) => answer,
        Err(e) => {
            warn!("pc setup error: {:?}", e);
            pc.close()
                .await
                .map_err(anyhow::Error::from)
                .if_err_info(logmsg!("can't close pc1"));
            return Err(e);
        }
    };

    {
        /* let mut map = state.connection_map.lock().await;
        if let Some(cd) = map.remove(&session_id) {
            cd.dispose();
        } */
        state.clone().remove_connection(&session_id);
        if !state
            .clone()
            .add_connection(ClientData::new(session_id, pc.clone(), payload.url))
        {
            pc.close()
                .await
                .map_err(anyhow::Error::from)
                .if_err_info(logmsg!("can't close pc0"));
            return Err(axum::http::StatusCode::SERVICE_UNAVAILABLE.into_response());
        }
    }

    let res = types::EnterResponse { sdp: answer };
    Ok(Json(res))
}
async fn candidate(
    Host(host): Host,
    State(state): State<SharedState>,
    Json(req): Json<types::SignedRequest>,
) -> Result<Json<types::EmptyResponse>, axum::response::Response> {
    let (session_id, payload) = req
        .verify::<types::CandidateRequestPayload>()
        .map_err(|ex| {
            warn!("failed: verify payload: {:?}", ex);
            axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response()
        })?;
    if payload.url.is_empty() || payload.sdp.candidate.is_empty() {
        return Err(axum::http::StatusCode::UNPROCESSABLE_ENTITY.into_response());
    }
    if let Some(cluster_client) = state.cluster_client.as_ref() {
        cluster::redirect_if_needed(cluster_client, &host, &payload.url, "/candidate")?;
    }

    let Some(cd) = state.get_connection(&session_id) else {
            debug!("no connection");
            return Err(axum::http::StatusCode::BAD_REQUEST.into_response());
    };
    cd.add_ice_candidate(payload.sdp)
        .await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response())?;

    Ok(Json(types::EmptyResponse {}))
}

async fn setup_pc(
    state: SharedState,
    session_id: SessionId,
    pc: &Arc<RTCPeerConnection>,
    sdp: RTCSessionDescription,
) -> Result<RTCSessionDescription, axum::response::Response> {
    {
        let state = Arc::downgrade(&state);
        let pc0 = Arc::downgrade(pc);
        pc.on_data_channel(Box::new(move |dc: Arc<RTCDataChannel>| {
            let state = state.clone();
            let cd = {
                if let Some(state) = state.upgrade() {
                    if let Some(cd) = state.get_connection(&session_id) {
                        cd.set_dc(dc.clone());
                        Arc::downgrade(&cd)
                    } else {
                        warn!("client data not found");
                        return Box::pin(async move {});
                    }
                } else {
                    warn!("shared state disposed");
                    return Box::pin(async move {});
                }
            };
            let dc0 = Arc::downgrade(&dc);
            let pc0 = pc0.clone();

            Box::pin(async move {
                let Some(dc) = dc0.upgrade() else {
                    return;
                };
                let state = state.clone();
                let dc0 = dc0.clone();
                let pc0 = pc0.clone();
                dc.on_message(Box::new(move |m: DataChannelMessage| {
                    let state = state.clone();
                    let dc0 = dc0.clone();
                    let pc0 = pc0.clone();
                    let cd = cd.clone();
                    Box::pin(async move {
                        let Some(cd) = cd.upgrade() else {
                            return;
                        };
                        let Some(state) = state.upgrade() else {
                            return;
                        };

                        if let Some(cluster_client) = state.cluster_client.as_ref() {
                            if !cluster_client.is_my_work(&cd.url) {
                                // trace!("[cluster] change worker");
                                info!("[cluster] change worker");
                                if let Some(dc0) = dc0.upgrade() {
                                    dc0.close()
                                        .await
                                        .map_err(anyhow::Error::from)
                                        .if_err_info(logmsg!("can't close dc0 a"));
                                }
                                if let Some(pc0) = pc0.upgrade() {
                                    pc0.close()
                                        .await
                                        .map_err(anyhow::Error::from)
                                        .if_err_info(logmsg!("can't close pc0 a"));
                                };
                                state.remove_connection(&session_id);
                                return;
                            }
                        }

                        rtc_api::on_rtc_message(state, cd, m.data.to_vec())
                            .await
                            .if_err_info(logmsg!());
                    })
                }));
            })
        }));
    }
    pc.set_remote_description(sdp).await.map_err(err_to_res)?;

    let answer = pc.create_answer(None).await.map_err(err_to_res_internal)?;

    let mut gather_complete = pc.gathering_complete_promise().await;

    pc.set_local_description(answer.clone())
        .await
        .map_err(err_to_res_internal)?;
    let _ = gather_complete.recv().await;
    let answer = pc
        .local_description()
        .await
        .ok_or(axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response())?;

    Ok(answer)
}

fn err_to_res(e: webrtc::Error) -> Response {
    warn!("{:?}", e);
    axum::http::StatusCode::BAD_REQUEST.into_response()
}
fn err_to_res_internal(e: webrtc::Error) -> Response {
    warn!("{:?}", e);
    axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response()
}
