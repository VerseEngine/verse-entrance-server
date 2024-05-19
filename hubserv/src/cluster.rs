use crate::args::Args;
use crate::state::SharedState;
use anyhow::Result;
use axum::response::{IntoResponse, Redirect};
#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};
use std::sync::Arc;
use tokio::time::sleep;

pub fn create_client(args: &Args) -> Option<Arc<verse_cluster::Client>> {
    let Some(cluster_host) = args.http_host.as_ref() else {
        return None;
    };
    let Some(cluster_node_host) = args.cluster_node_host.as_ref() else {
        return None;
    };
    assert_ne!(cluster_host, cluster_node_host);
    Some(Arc::new(verse_cluster::Client::new(
        cluster_node_host,
        cluster_host,
    )))
}

pub async fn start_client(args: &Args, app_state: SharedState) -> Result<()> {
    let Some(node_list_url) = args.cluster_node_list_url.clone() else {
        return Ok(());
    };
    if app_state.cluster_client.is_none() {
        warn!("cluster client is none");
        return Ok(());
    };

    for n in 0..5 {
        match load_node_list(&node_list_url, app_state.clone()).await {
            Ok(_) => break,
            Err(e) => {
                warn!("failed to load node list: {:?}", e);
                sleep(tokio::time::Duration::from_secs((n + 1) * 3)).await;
            }
        }
    }

    tokio::task::spawn({
        async move {
            loop {
                sleep(tokio::time::Duration::from_secs(60)).await;
                if let Err(e) = load_node_list(&node_list_url, app_state.clone()).await {
                    warn!("failed to update node list: {:?}", e);
                }
            }
        }
    });

    Ok(())
}

async fn load_node_list(url: &str, app_state: SharedState) -> Result<()> {
    let client = reqwest::Client::new();
    let res = client.get(url).send().await?;
    let data = res.json::<verse_cluster::data::NodeListData>().await?;
    if let Some(cluster_client) = app_state.cluster_client.as_ref() {
        if let Some(exists) = cluster_client.get_node_list() {
            if *exists == data.nodes {
                return Ok(());
            }
        }
        cluster_client.set_node_list(data);
        warn!("node list updated");
    }
    Ok(())
}

pub fn redirect_if_needed(
    cluster_client: &verse_cluster::Client,
    host: &str,
    world_url: &str,
    path: &str,
) -> Result<(), axum::response::Response> {
    match cluster_client.get_worker(world_url) {
        verse_cluster::Worker::Me => {}
        verse_cluster::Worker::Nothing => {
            trace!("[cluster] node nothing");
            return Err(axum::http::StatusCode::FORBIDDEN.into_response());
        }
        verse_cluster::Worker::Other(other_host) => {
            if cluster_client.can_redirect(host) {
                let redirect_to = format!("https://{}{}", other_host, path);
                trace!("[cluster] redirect to {}", redirect_to);
                return Err(Redirect::temporary(&redirect_to).into_response());
            } else {
                trace!("[cluster] bad request");
                return Err(axum::http::StatusCode::FORBIDDEN.into_response());
            }
        }
    }
    Ok(())
}
