use crate::args::Args;
use crate::state::SharedState;
use crate::version;
use axum::{
    extract::{FromRef, State},
    routing::get,
    Json, Router,
};
#[allow(unused_imports)]
use log::{debug, error, info, warn};
use std::collections::HashMap;
use std::net::{Ipv6Addr, SocketAddr};
use std::sync::atomic::Ordering;
use std::sync::Arc;

pub struct ServerContext {
    pub prometheus_prefix: Option<String>,
}

#[derive(Clone, FromRef)]
struct States {
    server_context: Arc<ServerContext>,
    app_state: SharedState,
}

pub async fn start_server(args: &Args, app_state: SharedState) {
    let app = Router::new()
        .route("/", get(index))
        .route("/metrics", get(prometheus))
        .with_state(States {
            app_state,
            server_context: Arc::new(ServerContext {
                prometheus_prefix: args.prometheus_prefix.clone(),
            }),
        });
    let addr = SocketAddr::from((Ipv6Addr::UNSPECIFIED, args.status_port));
    info!("status server 0.0.0.0:{}", args.status_port);
    axum_server::bind(addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}
async fn index(
    State(_context): State<Arc<ServerContext>>,
    State(state): State<SharedState>,
) -> Result<Json<HashMap<String, serde_json::Value>>, axum::http::StatusCode> {
    let mut res = HashMap::<String, serde_json::Value>::new();
    res.insert("version".into(), version::VERSION.into());
    for v in get_metrics(state).await {
        res.insert(v.0, v.1.into());
    }
    Ok(Json(res))
}
async fn prometheus(
    State(context): State<Arc<ServerContext>>,
    State(state): State<SharedState>,
) -> Result<String, axum::http::StatusCode> {
    let mut ar = vec![(
        format!("instance{{version=\"{}\"}}", version::VERSION),
        1i64,
    )];
    ar.append(&mut get_metrics(state).await.into_iter().collect());

    let prefix = context
        .prometheus_prefix
        .as_ref()
        .map_or("".to_string(), |v| v.clone());

    Ok(ar
        .iter()
        .map(|v| format!("{}{} {}", prefix, v.0, v.1))
        .collect::<Vec<String>>()
        .join("\n"))
}

async fn get_metrics(state: SharedState) -> Vec<(String, i64)> {
    /* let mut res = Vec::<(String, i64)>::new();
    res.push((
        "client_count".to_string(),
        state.client_count.load(Ordering::Relaxed) as i64,
    ));

    res */

    vec![(
        "client_count".to_string(),
        state.client_count.load(Ordering::Relaxed) as i64,
    )]
}
