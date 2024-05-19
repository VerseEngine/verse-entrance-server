use crate::args::Args;
use crate::entrance_server_router;
use crate::state::SharedState;
use axum::error_handling::HandleErrorLayer;
use axum::{extract::State, routing::get, Router};
use axum::{
    http::{HeaderMap, Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
};
// use axum_server::AddrIncomingConfig;
use ftlog::{
    appender::{FileAppender, Period},
    LevelFilter, Logger,
};
#[allow(unused_imports)]
use log::{debug, error, info, warn};
use log::{Level, Log, Record};
use rustls::ServerConfig;
use rustls_acme::caches::DirCache;
use rustls_acme::AcmeConfig;
use std::net::{Ipv6Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;
use tokio_stream::StreamExt;
use tower_http::cors;
use tower_http::timeout::TimeoutLayer;
use tower_http::{compression::CompressionLayer, decompression::DecompressionLayer};

pub async fn start_server(args: &Args, app_state: SharedState) {
    let ft_logger = args.http_log_path.as_ref().map(|http_log_path| {
        Arc::new(
            ftlog::builder()
                .max_log_level(LevelFilter::Info)
                .bounded(100_000, false)
                .root(FileAppender::rotate_with_expire(
                    http_log_path,
                    Period::Day,
                    ftlog::appender::Duration::days(30),
                ))
                .utc()
                .build()
                .expect("logger build or set failed"),
        )
    });

    let er = entrance_server_router::create_router(&app_state)
        .layer(middleware::from_fn_with_state(
            ft_logger.clone(),
            log_middleware,
        ))
        .layer(HandleErrorLayer::new(|error| async move {
            warn!("request error: {:?}", error);
            error
        }))
        .layer(TimeoutLayer::new(Duration::from_secs(10)))
        .layer(DecompressionLayer::new())
        .layer(CompressionLayer::new())
        .layer(
            cors::CorsLayer::new()
                .allow_origin(cors::Any)
                .allow_methods(vec![http::Method::POST, http::Method::OPTIONS])
                .allow_headers([http::header::CONTENT_TYPE])
                .max_age(Duration::from_secs(86400)),
        );
    let update_cluster_path = format!(
        "/update-cluster-{}",
        args.update_cluster_key.clone().unwrap_or("".into())
    );
    let cr = Router::new()
        .route(&update_cluster_path, get(update_cluster))
        .with_state(app_state.clone())
        .layer(middleware::from_fn_with_state(ft_logger, log_middleware));

    let app = Router::new().merge(er).merge(cr);

    if args.use_https {
        let Some(http_host) = &args.http_host else {
            warn!("http_host is not set");
            return;
        };
        let addr = SocketAddr::from((Ipv6Addr::UNSPECIFIED, 443));
        let hosts = if let Some(cluster_node_host) = &args.cluster_node_host {
            vec![http_host, cluster_node_host]
        } else {
            vec![http_host]
        };
        info!(
            "api server {}",
            hosts
                .iter()
                .map(|v| format!("{}:443", v))
                .collect::<Vec<_>>()
                .join(", ")
        );
        let mut acme_state = AcmeConfig::new(hosts)
            .contact(
                args.lets_encrypt_email
                    .iter()
                    .map(|e| format!("mailto:{}", e)),
            )
            .cache_option(args.cache.clone().map(DirCache::new))
            .directory_lets_encrypt(true)
            .state();
        let mut rustls_config = ServerConfig::builder()
            .with_safe_defaults()
            .with_no_client_auth()
            .with_cert_resolver(acme_state.resolver());
        rustls_config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

        let acceptor = acme_state.axum_acceptor(Arc::new(rustls_config));

        tokio::spawn(async move {
            loop {
                match acme_state.next().await.unwrap() {
                    Ok(ok) => log::info!("event: {:?}", ok),
                    Err(err) => log::error!("error: {:?}", err),
                }
            }
        });
        axum_server::bind(addr)
            /* .addr_incoming_config(
                AddrIncomingConfig::default()
                    .tcp_keepalive(Some(Duration::from_secs(10)))
                    .build(),
            ) */
            .acceptor(acceptor)
            .serve(app.into_make_service())
            .await
            .unwrap();
    } else {
        let addr = SocketAddr::from((Ipv6Addr::UNSPECIFIED, args.http_port));
        info!("api server 0.0.0.0:{}", args.http_port);
        axum_server::bind(addr)
            /* .addr_incoming_config(
                AddrIncomingConfig::default()
                    .tcp_keepalive(Some(Duration::from_secs(10)))
                    .build(),
            ) */
            .serve(app.into_make_service())
            .await
            .unwrap();
    }
}
async fn update_cluster(
    State(state): State<SharedState>,
    // ) -> Result<(StatusCode, &'static str), StatusCode> {
) -> Result<impl IntoResponse, StatusCode> {
    let headers = create_update_cluster_response_headers().map_err(|ex| {
        warn!("failed: update cluster: {:?}", ex);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if let Some(cluster_manager) = state.cluster_manager.clone() {
        cluster_manager.update().await.map_err(|ex| {
            warn!("failed: update cluster: {:?}", ex);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        return Ok((
            StatusCode::OK,
            headers,
            r#"{
        "update": true
    }"#,
        ));
    }
    Ok((
        axum::http::StatusCode::OK,
        headers,
        r#"{
        "update": false
    }"#,
    ))
}

fn create_update_cluster_response_headers() -> anyhow::Result<HeaderMap> {
    let expires = (chrono::Utc::now() + chrono::Duration::minutes(1)).format("%a, %d %b %Y %T %Z");
    let mut headers = HeaderMap::new();
    headers.insert("Content-Type", "application/json".parse()?);
    headers.insert("Cache-Control", "no-transform".parse()?);
    headers.insert("Expires", format!("{}", expires).parse()?);
    Ok(headers)
}

async fn log_middleware<B>(
    State(logger): State<Option<Arc<Logger>>>,
    // you can add more extractors here but the last
    // extractor must implement `FromRequest` which
    // `Request` does
    req: Request<B>,
    next: Next<B>,
) -> Response {
    let uri = req.uri().clone();
    let method = req.method().clone();
    let ua = req.headers().get("User-Agent").map_or_else(
        || "-".to_string(),
        |v| v.to_str().unwrap_or_default().to_string(),
    );
    let referer = req.headers().get("referer").map_or_else(
        || "-".to_string(),
        |v| v.to_str().unwrap_or_default().to_string(),
    );
    let xff = req.headers().get("X-Forwarded-For").map_or_else(
        || "-".to_string(),
        |v| v.to_str().unwrap_or_default().to_string(),
    );

    let res = next.run(req).await;

    if let Some(logger) = logger {
        logger.log(
            &Record::builder()
                .args(format_args!(
                    "{} {} {} {} {} {}",
                    res.status(),
                    method,
                    uri,
                    ua,
                    referer,
                    xff
                ))
                .level(Level::Info)
                .target("access")
                .build(),
        );
    }

    res
}
