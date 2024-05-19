#[allow(unused_imports)]
use log::{debug, error, info, warn};
use std::sync::Arc;
use tokio::net::UdpSocket;
use webrtc::{dtls_transport::dtls_role::DTLSRole, ice::mdns::MulticastDnsMode};
mod entrance_server_router;
mod ids;
mod rtc_api;
mod state;
mod types;
use crate::state::State;
mod args;
use args::Args;
mod api_server;
mod cluster;
mod dns;
mod status_server;
mod swarm;
mod version;

// Ex: https://github.com/FlorianUekermann/rustls-acme/tree/main/examples

#[tokio::main]
async fn main() {
    // console_subscriber::init(); // tokio-console
    env_logger::init();
    let args = Args::load().await.unwrap();
    info!("start hub\n{:#?}", args);

    let mut cluster_manager: Option<Arc<verse_cluster::manager::Manager>> = None;
    if args.cluster_node_host.is_some() {
        let http_host = args.http_host.as_ref().unwrap();
        let cf_email = args.cloudflare_email.as_ref().unwrap();
        let cf_api_key = args.cloudflare_api_key.as_ref().unwrap();
        let aws_region = args.aws_ec2_region.as_ref().unwrap();
        let role = args.cluster_node_role.as_ref().unwrap();
        let stage = args.cluster_node_stage.as_ref().unwrap();
        let cluster_json_s3_bucket = args.cluster_json_s3_bucket.as_ref().unwrap();
        let cluster_json_s3_key = args.cluster_json_s3_key.as_ref().unwrap();
        let cm = verse_cluster::manager::Manager::new(
            http_host,
            stage,
            role,
            aws_region,
            verse_cluster::manager::S3Path {
                bucket: cluster_json_s3_bucket.into(),
                key: cluster_json_s3_key.into(),
            },
            verse_cluster::manager::CfAuthInfo {
                email: cf_email.to_owned(),
                api_key: cf_api_key.to_owned(),
            },
        );
        cm.update().await.unwrap();
        cluster_manager = Some(Arc::new(cm));
    } else if let (
        Some(http_host),
        Some(public_ip),
        Some(cloudflare_email),
        Some(cloudflare_api_key),
    ) = (
        args.http_host.as_ref(),
        args.public_ip.as_ref(),
        args.cloudflare_email.as_ref(),
        args.cloudflare_api_key.as_ref(),
    ) {
        dns::set_arecord(
            http_host,
            public_ip,
            cloudflare_email,
            cloudflare_api_key,
            true,
            // !args.use_https,
        )
        .await
        .unwrap();
    }

    info!("listen udp 0.0.0.0:{}", args.udp_port);
    let udp_socket = UdpSocket::bind(format!("0.0.0.0:{}", args.udp_port))
        .await
        .unwrap();

    let app_state = State::new(
        create_webrtc_api(args.public_ip.clone(), Some(udp_socket)),
        args.max_connections,
        args.max_connections_by_url,
        args.max_routing_results,
        args.ice_servers.clone(),
        args.access_log_path.as_deref(),
        cluster::create_client(&args),
        cluster_manager,
    );

    cluster::start_client(&args, app_state.clone())
        .await
        .unwrap();
    tokio::join!(
        api_server::start_server(&args, app_state.clone()),
        status_server::start_server(&args, app_state),
    );
}

fn create_webrtc_api(public_ip: Option<String>, sock: Option<UdpSocket>) -> webrtc::api::API {
    use webrtc::ice::udp_mux::{UDPMuxDefault, UDPMuxParams};
    let mut se = webrtc::api::setting_engine::SettingEngine::default();
    if let Some(sock) = sock {
        let udp_mux = UDPMuxDefault::new(UDPMuxParams::new(sock));
        se.set_udp_network(webrtc::ice::udp_network::UDPNetwork::Muxed(udp_mux));
    }
    if let Some(public_ip) = public_ip {
        info!("set publicip: {}", public_ip);
        se.set_nat_1to1_ips(
            vec![public_ip],
            webrtc::ice_transport::ice_candidate_type::RTCIceCandidateType::Host,
        );
        // se.set_ice_multicast_dns_mode(MulticastDnsMode::QueryOnly);
    } else {
        info!("no publicip");
        // se.set_ice_multicast_dns_mode(MulticastDnsMode::QueryAndGather);
    }
    se.set_ice_multicast_dns_mode(MulticastDnsMode::Disabled);
    se.set_network_types(vec![webrtc::ice::network_type::NetworkType::Udp4]);

    se.set_lite(true);
    se.disable_certificate_fingerprint_verification(true);
    se.set_answering_dtls_role(DTLSRole::Server).unwrap();

    webrtc::api::APIBuilder::new()
        .with_setting_engine(se)
        .build()
}
