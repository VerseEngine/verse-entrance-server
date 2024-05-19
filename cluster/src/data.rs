use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct NodeListData {
    pub nodes: Vec<NodeListNode>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct NodeListNode {
    pub host: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_node_list_data() {
        let data = NodeListData {
            nodes: vec![
                NodeListNode {
                    host: "entrance-8247f23c98f7f944.verseengine.cloud".into(),
                },
                NodeListNode {
                    host: "entrance-0000000000000000.verseengine.cloud".into(),
                },
            ],
        };
        let payload = serde_json::to_string(&data);
        assert_eq!(
            payload.as_ref().unwrap(),
            r#"{"nodes":[{"host":"entrance-8247f23c98f7f944.verseengine.cloud"},{"host":"entrance-0000000000000000.verseengine.cloud"}]}"#
        );
        let data1: NodeListData = serde_json::from_str(payload.as_ref().unwrap()).unwrap();
        assert_eq!(data, data1);
    }
}
