use anyhow::Result;
use axum::http::header::HeaderMap;
use dashmap::DashMap;
use ftlog::{
    appender::{Duration, FileAppender, Period},
    FtLogFormat, LevelFilter, Logger,
};
use fxhash::FxBuildHasher;
#[allow(unused_imports)]
use log::{debug, error, info, warn};
use log::{Level, Log, Record};
use std::borrow::Cow;
use std::fmt::Display;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use verse_session_id::SessionId;

mod client_data;
pub use client_data::ClientData;
mod url_data;
pub use url_data::UrlData;

pub struct State {
    pub api: webrtc::api::API,
    connection_map: DashMap<SessionId, Arc<ClientData>, FxBuildHasher>,
    url_data_map: DashMap<String, Arc<UrlData>, FxBuildHasher>,
    pub client_count: AtomicU64,

    pub max_connections: Option<usize>,
    pub max_connections_by_url: Option<usize>,

    pub max_routing_results: usize,

    pub ice_servers: Vec<String>,

    pub ft_logger: Option<Logger>,

    pub cluster_client: Option<Arc<verse_cluster::Client>>,
    pub cluster_manager: Option<Arc<verse_cluster::manager::Manager>>,
}
impl State {
    pub fn new(
        api: webrtc::api::API,
        max_connections: Option<usize>,
        max_connections_by_url: Option<usize>,
        max_routing_results: usize,
        ice_servers: Vec<String>,
        access_log_path: Option<&str>,
        cluster_client: Option<Arc<verse_cluster::Client>>,
        cluster_manager: Option<Arc<verse_cluster::manager::Manager>>,
    ) -> SharedState {
        let ft_logger = access_log_path.map(|access_log_path| {
            ftlog::builder()
                .max_log_level(LevelFilter::Info)
                .format(AccessLogFormatter {})
                .bounded(100_000, false)
                .root(FileAppender::rotate_with_expire(
                    access_log_path,
                    Period::Day,
                    Duration::days(30),
                ))
                .utc()
                .build()
                .expect("logger build or set failed")
        });

        Arc::new(State {
            api,
            connection_map: DashMap::with_hasher(FxBuildHasher::default()),
            url_data_map: DashMap::with_hasher(FxBuildHasher::default()),
            client_count: AtomicU64::new(0),
            max_connections,
            max_connections_by_url,
            max_routing_results,
            ice_servers,
            ft_logger,
            cluster_client,
            cluster_manager,
        })
    }
    pub fn is_new_connection_available(&self, url: &str) -> bool {
        let client_count = self.client_count.load(Ordering::Relaxed) as usize;
        if self.max_connections.unwrap_or(usize::MAX) <= client_count {
            return false;
        }
        if let Some(ud) = self.get_url_data(url) {
            if self.max_connections_by_url.unwrap_or(usize::MAX) <= ud.get_client_count() {
                return false;
            }
        }
        true
    }
    pub fn add_connection(self: &Arc<Self>, cd: Arc<ClientData>) -> bool {
        loop {
            let client_count = self.client_count.load(Ordering::SeqCst);
            if self.max_connections.unwrap_or(usize::MAX) <= client_count as usize {
                return false;
            }
            if self
                .client_count
                .compare_exchange(
                    client_count,
                    client_count + 1,
                    Ordering::Acquire,
                    Ordering::Relaxed,
                )
                .is_err()
            {
                continue;
            }
            break;
        }
        self.connection_map.insert(cd.session_id, cd.clone());

        if !match self.url_data_map.entry(cd.url.clone()) {
            dashmap::mapref::entry::Entry::Occupied(ref ud) => ud
                .get()
                .add_connection(cd.clone(), self.max_connections_by_url),
            dashmap::mapref::entry::Entry::Vacant(v) => {
                let ud = UrlData::new(cd.clone());
                v.insert(ud);
                true
            }
        } {
            self.client_count.fetch_sub(1, Ordering::SeqCst);
            self.connection_map.remove(&cd.session_id);
            return false;
        }
        true
    }
    pub fn remove_connection(self: &Arc<Self>, session_id: &SessionId) {
        if let Some((_, cd)) = self.connection_map.remove(session_id) {
            self.client_count.fetch_sub(1, Ordering::SeqCst);
            cd.dispose();

            if let Some(ud) = self.url_data_map.get(&cd.url).map(|v| v.clone()) {
                ud.remove_connection(session_id);
            }
            self.url_data_map.remove_if(&cd.url, |_, ud| ud.is_empty());
        }
    }
    pub fn get_connection(&self, session_id: &SessionId) -> Option<Arc<ClientData>> {
        self.connection_map.get(session_id).map(|v| v.clone())
    }
    pub fn get_url_data(&self, url: &str) -> Option<Arc<UrlData>> {
        self.url_data_map.get(url).map(|v| v.clone())
    }
    pub async fn send_rpc_response(
        &self,
        to_session_id: &SessionId,
        rpc_id: u32,
        param: Vec<u8>,
    ) -> Result<bool> {
        let Some(to_cd) = self.get_connection(to_session_id) else {
            return Ok(false);
        };
        to_cd.send_rpc_response(rpc_id, param).await
    }

    pub fn append_access_log(&self, client_count: usize, url: &str, headers: &HeaderMap) {
        if let Some(ft_logger) = self.ft_logger.as_ref() {
            let cf_ip_country = if let Some(cf_ip_country) = headers.get("cf-ipcountry") {
                cf_ip_country.to_str().unwrap_or("")
            } else {
                ""
            };

            ft_logger.log(
                &Record::builder()
                    .args(format_args!("{} {} {}", cf_ip_country, client_count, url))
                    .level(Level::Info)
                    .target("access")
                    .build(),
            );
        }
    }
}
pub type SharedState = Arc<State>;

struct AccessLogFormatter {}
impl FtLogFormat for AccessLogFormatter {
    #[inline]
    fn msg(&self, record: &Record) -> Box<dyn Send + Sync + Display> {
        Box::new(AccessLogMessage {
            args: record
                .args()
                .as_str()
                .map(Cow::Borrowed)
                .unwrap_or_else(|| Cow::Owned(format!("{}", record.args()))),
        })
    }
}
struct AccessLogMessage {
    args: Cow<'static, str>,
}

impl Display for AccessLogMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!("access {}", self.args))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use verse_session_id::*;
    use webrtc::api::APIBuilder;
    use webrtc::peer_connection::configuration::RTCConfiguration;

    #[tokio::test]
    async fn test_state_connections() {
        let state = State::new(
            APIBuilder::new().build(),
            Some(3),
            Some(2),
            10,
            vec![],
            "/dev/null".into(),
            None,
            None,
        );

        let config = RTCConfiguration::default();
        let pc = Arc::new(state.api.new_peer_connection(config).await.unwrap());

        state.add_connection(ClientData::new(
            sid(1),
            pc.clone(),
            "https://example.domain/1".to_string(),
        ));
        assert!(state.is_new_connection_available("https://example.domain/1"));
        state.add_connection(ClientData::new(
            sid(2),
            pc.clone(),
            "https://example.domain/1".to_string(),
        ));
        assert!(!state.is_new_connection_available("https://example.domain/1"));
        assert!(state.is_new_connection_available("https://example.domain/2"));

        state.add_connection(ClientData::new(
            sid(3),
            pc,
            "https://example.domain/3".to_string(),
        ));
        assert!(!state.is_new_connection_available("https://example.domain/1"));
        assert!(!state.is_new_connection_available("https://example.domain/2"));
        assert!(!state.is_new_connection_available("https://example.domain/3"));

        state.remove_connection(&sid(3));
        assert!(!state.is_new_connection_available("https://example.domain/1"));
        assert!(state.is_new_connection_available("https://example.domain/2"));
        assert!(state.is_new_connection_available("https://example.domain/3"));

        state.remove_connection(&sid(2));
        assert!(state.is_new_connection_available("https://example.domain/1"));
    }

    fn sid(v: u8) -> SessionId {
        let mut res: RawSessionId = Default::default();
        res[0] = v;
        res.into()
    }
}
