use crate::data::{NodeListData, NodeListNode};
use parking_lot::Mutex;
use std::sync::Arc;

pub enum Worker {
    Me,
    Other(String),
    Nothing,
}

pub struct Client {
    pub node_host: String,
    cluster_host: String,
    node_list: Mutex<Option<Arc<Vec<NodeListNode>>>>,
}

impl Client {
    pub fn new(node_host: &str, cluster_host: &str) -> Self {
        Client {
            node_host: node_host.into(),
            cluster_host: cluster_host.into(),
            node_list: Mutex::new(None),
        }
    }
    pub fn set_node_list(&self, data: NodeListData) {
        *self.node_list.lock() = Some(Arc::new(data.nodes));
    }
    pub fn get_node_list(&self) -> Option<Arc<Vec<NodeListNode>>> {
        self.node_list.lock().as_ref().cloned()
    }
    pub fn get_assigned_node(&self, world_url: &str) -> Option<NodeListNode> {
        let Some(ar) = self.get_node_list() else {
            return None;
        };
        if ar.len() == 0 {
            return None;
        }
        Some(ar[to_hash(world_url) as usize % ar.len()].clone())
    }

    pub fn is_my_work(&self, world_url: &str) -> bool {
        let Some(assigned_node) = self.get_assigned_node(world_url) else {
            return false;
        };
        assigned_node.host == self.node_host
    }
    pub fn get_worker(&self, world_url: &str) -> Worker {
        let Some(assigned_node) = self.get_assigned_node(world_url) else {
            return Worker::Nothing;
        };
        if assigned_node.host == self.node_host {
            return Worker::Me;
        }
        Worker::Other(assigned_node.host)
    }

    pub fn can_redirect(&self, request_host: &str) -> bool {
        self.cluster_host == request_host
    }
}

fn to_hash(key: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_client() {
        let client0 = Client::new("node0", "all");
        assert_eq!(client0.get_node_list(), None);
        assert!(!client0.is_my_work("a"));
        assert!(client0.can_redirect("all"));
        assert!(!client0.can_redirect("node0"));
    }
    #[test]
    fn test_clients() {
        const NUM_CLIENTS: usize = 10;
        const NUM_REQUESTS: usize = 300;
        let clients: Vec<_> = (0..NUM_CLIENTS)
            .map(|i| {
                let n = format!("node{}", i);
                Client::new(&n, "all")
            })
            .collect();
        let data = NodeListData {
            nodes: (0..NUM_CLIENTS)
                .map(|i| {
                    let n = format!("node{}", i);
                    NodeListNode { host: n }
                })
                .collect(),
        };
        for c in clients.iter() {
            c.set_node_list(data.clone());
        }
        let mut count = 0;
        let mut counts = [0; NUM_CLIENTS];
        for i in 0..NUM_REQUESTS {
            for (ci, c) in clients.iter().enumerate() {
                let n = format!("{}", i);
                if c.is_my_work(&n) {
                    count += 1;
                    counts[ci] += 1;
                }
            }
        }
        assert_eq!(count, NUM_REQUESTS);
        for count in counts {
            assert!(count > 10);
        }
    }
}
