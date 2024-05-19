use super::ClientData;
#[allow(unused_imports)]
use log::{debug, error, info, warn};
use parking_lot::{Mutex, RwLock};
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::Arc;
use verse_proto::swarm::*;
const ROUTING_INFO_UPDATE_INTERVAL_SECONDS: i64 = 5;
const ROUTING_INFO_MAX: usize = 1000;

pub struct UrlData {
    clients: Mutex<Vec<Arc<ClientData>>>,
    client_count: AtomicU64,
    routing_info_updated: AtomicI64,
    routing_info: RwLock<Arc<RoutingInfo>>,
}

impl UrlData {
    pub fn new(cd: Arc<ClientData>) -> Arc<Self> {
        Arc::new(UrlData {
            clients: Mutex::new(vec![cd]),
            client_count: AtomicU64::new(1),
            routing_info_updated: AtomicI64::new(0),
            routing_info: RwLock::new(Arc::new(Self::create_default_routing_info())),
        })
    }
    #[cfg(test)]
    pub fn new_empty() -> Arc<Self> {
        Arc::new(UrlData {
            clients: Mutex::new(vec![]),
            client_count: AtomicU64::new(1),
            routing_info_updated: AtomicI64::new(0),
            routing_info: RwLock::new(Arc::new(Self::create_default_routing_info())),
        })
    }

    pub fn update_routing_info_if_needed(self: &Arc<Self>) {
        let now = get_now_sec();
        let routing_info_updated = self.routing_info_updated.load(Ordering::Relaxed);
        if now - routing_info_updated < ROUTING_INFO_UPDATE_INTERVAL_SECONDS {
            return;
        }
        if self
            .routing_info_updated
            .compare_exchange(
                routing_info_updated,
                now,
                Ordering::Acquire,
                Ordering::Relaxed,
            )
            .is_err()
        {
            return;
        }
        self.update_routing_info();
    }
    fn update_routing_info(self: &Arc<Self>) {
        let relation = {
            let clients = {
                let v = self.clients.lock();
                v.clone()
            };
            let mut relation: Vec<RoutingInfo> = Vec::with_capacity(clients.len());
            for cd in clients.iter().take(ROUTING_INFO_MAX) {
                let Some(ri) = cd.get_routing_info() else {
                    continue;
                };
                relation.push(ri.as_ref().clone());
            }
            relation
        };

        *self.routing_info.write() = Arc::new(RoutingInfo {
            node_type: NodeType::Tracker.into(),
            relation: Some(verse_proto::swarm::routing_info::Relation::RoutingInfos(
                RoutingInfos { infos: relation },
            )),
            ..Default::default()
        });
    }
    pub fn get_routing_info(&self) -> Arc<RoutingInfo> {
        self.routing_info.read().clone()
    }
    fn create_default_routing_info() -> RoutingInfo {
        RoutingInfo {
            node_type: NodeType::Tracker.into(),
            relation: Some(verse_proto::swarm::routing_info::Relation::RoutingInfos(
                RoutingInfos { infos: vec![] },
            )),
            ..Default::default()
        }
    }

    pub fn is_empty(&self) -> bool {
        self.get_client_count() == 0
    }
    pub fn get_client_count(&self) -> usize {
        self.client_count.load(Ordering::SeqCst) as usize
    }

    pub fn add_connection(
        self: &Arc<Self>,
        cd: Arc<ClientData>,
        max_connections_by_url: Option<usize>,
    ) -> bool {
        loop {
            let client_count = self.client_count.load(Ordering::SeqCst);
            if max_connections_by_url.unwrap_or(usize::MAX) <= client_count as usize {
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
            self.clients.lock().push(cd);
            return true;
        }
    }
    pub fn remove_connection(self: &Arc<Self>, session_id: &verse_session_id::SessionId) {
        self.client_count.fetch_sub(1, Ordering::SeqCst);
        {
            let mut clients = self.clients.lock();
            clients.retain(|v| !v.session_id.eq(session_id));
        }
        let mut routing_info = self.routing_info.write();
        let mut new_ri = (&*routing_info as &RoutingInfo).clone();
        if let Some(routing_info::Relation::RoutingInfos(v)) = new_ri.relation.as_mut() {
            v.infos.retain(|v| {
                v.session_id
                    .as_ref()
                    .map_or(true, |id| !session_id.eq_slice(id))
            });
        };
        *routing_info = Arc::new(new_ri);
    }
}

fn get_now_sec() -> i64 {
    chrono::Local::now().timestamp()
}

#[cfg(test)]
mod tests {
    use super::*;
    use webrtc::api::APIBuilder;
    use webrtc::peer_connection::configuration::RTCConfiguration;
    #[tokio::test]
    async fn test_update_routing_info_if_needed() {
        let ud = UrlData::new_empty();
        let routing_info_updated = ud.routing_info_updated.load(Ordering::Relaxed);
        assert_eq!(routing_info_updated, 0);
        assert_eq!(ud.get_routing_info().get_relations().unwrap().len(), 0);

        ud.update_routing_info_if_needed();
        let routing_info_updated = ud.routing_info_updated.load(Ordering::Relaxed);
        assert_ne!(routing_info_updated, 0);
        assert_eq!(ud.get_routing_info().get_relations().unwrap().len(), 0);
    }
    #[tokio::test]
    async fn test_update_routing_info() {
        let config = RTCConfiguration::default();
        let api = APIBuilder::new().build();
        let pc = Arc::new(api.new_peer_connection(config).await.unwrap());

        let cd0 = ClientData::new([0; 32].into(), pc.clone(), "".to_string());
        cd0.set_routing_info(RoutingInfo {
            session_id: Some(cd0.session_id.clone().to_vec()),
            relation: Some(verse_proto::swarm::routing_info::Relation::Count(1)),
            ..Default::default()
        });
        let cd1 = ClientData::new([1; 32].into(), pc, "".to_string());
        cd1.set_routing_info(RoutingInfo {
            session_id: Some(cd1.session_id.clone().to_vec()),
            relation: Some(verse_proto::swarm::routing_info::Relation::Count(3)),
            ..Default::default()
        });

        let ud = Arc::new(UrlData {
            client_count: AtomicU64::new(2),
            clients: parking_lot::Mutex::new(vec![cd0, cd1]),
            routing_info_updated: AtomicI64::new(0),
            routing_info: RwLock::new(Arc::new(UrlData::create_default_routing_info())),
        });

        ud.update_routing_info_if_needed();
        let routing_info_updated = ud.routing_info_updated.load(Ordering::Relaxed);
        assert_ne!(routing_info_updated, 0);
        {
            let ri = ud.get_routing_info();
            assert_eq!(ri.get_relations().unwrap().len(), 2);
            assert_eq!(ri.get_relations().unwrap()[0].get_relation_count(), 1);
            assert_eq!(ri.get_relations().unwrap()[1].get_relation_count(), 3);
        }
    }
    #[tokio::test]
    async fn test_add_remove_connection() {
        let config = RTCConfiguration::default();
        let api = APIBuilder::new().build();
        let pc = Arc::new(api.new_peer_connection(config).await.unwrap());
        let ud = Arc::new(UrlData {
            client_count: AtomicU64::new(0),
            clients: parking_lot::Mutex::new(Vec::new()),
            routing_info_updated: AtomicI64::new(0),
            routing_info: RwLock::new(Arc::new(UrlData::create_default_routing_info())),
        });

        let cd0 = ClientData::new([0; 32].into(), pc.clone(), "".to_string());
        let cd1 = ClientData::new([1; 32].into(), pc, "".to_string());
        assert_eq!(ud.clients.lock().len(), 0);
        ud.add_connection(cd0, None);
        assert_eq!(ud.clients.lock().len(), 1);
        ud.add_connection(cd1, None);
        assert_eq!(ud.clients.lock().len(), 2);
        assert_eq!(ud.clients.lock()[0].session_id, [0; 32].into());
        assert_eq!(ud.clients.lock()[1].session_id, [1; 32].into());
        ud.remove_connection(&[0; 32].into());
        assert_eq!(ud.clients.lock().len(), 1);
        assert_eq!(ud.clients.lock()[0].session_id, [1; 32].into());
        ud.remove_connection(&[1; 32].into());
        assert_eq!(ud.clients.lock().len(), 0);
    }
    #[tokio::test]
    async fn test_add_remove_connection2() {
        let config = RTCConfiguration::default();
        let api = APIBuilder::new().build();
        let pc = Arc::new(api.new_peer_connection(config).await.unwrap());
        let ud = Arc::new(UrlData {
            client_count: AtomicU64::new(0),
            clients: parking_lot::Mutex::new(Vec::new()),
            routing_info_updated: AtomicI64::new(0),
            routing_info: RwLock::new(Arc::new(UrlData::create_default_routing_info())),
        });

        let cd0 = ClientData::new([0; 32].into(), pc.clone(), "".to_string());
        cd0.set_routing_info(RoutingInfo {
            session_id: Some(cd0.session_id.to_vec()),
            ..Default::default()
        });
        let cd1 = ClientData::new([1; 32].into(), pc, "".to_string());
        cd1.set_routing_info(RoutingInfo {
            session_id: Some(cd1.session_id.to_vec()),
            ..Default::default()
        });
        ud.add_connection(cd0, None);
        ud.add_connection(cd1, None);
        ud.update_routing_info();
        assert_eq!(ud.clients.lock().len(), 2);
        assert_eq!(ud.get_routing_info().node_type(), NodeType::Tracker);

        let infos = {
            let ri = ud.get_routing_info();
            ri.get_relations().unwrap().to_vec()
        };
        assert_eq!(infos.len(), 2,);
        assert_eq!(
            infos[0].session_id.as_ref().unwrap().clone(),
            [0; 32].to_vec(),
        );
        assert_eq!(
            infos[1].session_id.as_ref().unwrap().clone(),
            [1; 32].to_vec(),
        );

        ud.remove_connection(&[0; 32].into());
        assert_eq!(ud.get_routing_info().get_relations().unwrap().len(), 1);

        ud.remove_connection(&[1; 32].into());

        assert_eq!(ud.clients.lock().len(), 0);
        assert_eq!(ud.get_routing_info().node_type(), NodeType::Tracker);
        assert_eq!(ud.get_routing_info().get_relations().unwrap().len(), 0,);
    }
}
