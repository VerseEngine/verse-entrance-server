use crate::aws::load_aws_config;
use crate::cf;
pub use crate::cf::CfAuthInfo;
use crate::data;
use crate::func::*;
use crate::node_source;
use anyhow::{anyhow, bail, Result};

pub struct S3Path {
    pub bucket: String,
    pub key: String,
}

pub struct Manager {
    cluster_host: String,
    role: String,
    stage: String,
    aws_region: String,

    cluster_json_s3_path: S3Path,

    cf_auth: CfAuthInfo,
}

impl Manager {
    pub fn new(
        cluster_host: &str,
        role: &str,
        stage: &str,
        aws_region: &str,
        cluster_json_s3_path: S3Path,
        cf_auth: CfAuthInfo,
    ) -> Self {
        Manager {
            cluster_host: cluster_host.into(),
            role: role.into(),
            stage: stage.into(),
            aws_region: aws_region.into(),
            cluster_json_s3_path,
            cf_auth,
        }
    }

    pub async fn update(&self) -> Result<()> {
        let nodes = self.get_nodes().await?;
        // node listのjsonファイルをs3にアップロードする
        self.update_cluster_json(&nodes).await?;
        self.update_dns(&nodes).await?;
        Ok(())
    }

    async fn update_dns(&self, nodes: &[node_source::Node]) -> Result<()> {
        let (prefix, base_domain) = split_host(&self.cluster_host);

        let cf_client = cf::create_client(&self.cf_auth)?;
        let Some(cf_zone_id) = cf::get_zone_id(&cf_client, &base_domain).await? else {
            bail!(format!("{} not found", base_domain));
        };

        let records = cf::get_arecords(&cf_client, &cf_zone_id).await?;
        let RecordGroups {
            cluster_records,
            single_records,
        } = split_records(&records, &prefix, &base_domain, &self.cluster_host);

        let current_ips: Vec<&str> = nodes.iter().map(|v| v.public_ip.as_str()).collect();

        let DiffResult { adds, dels } = diff_records(&cluster_records, &current_ips);
        for v in dels {
            cf::delete_arecord(&cf_client, &cf_zone_id, &v.id).await?;
        }
        for v in adds {
            cf::add_arecord(&cf_client, &cf_zone_id, &prefix, v, true).await?;
        }

        let DiffResult { adds, dels } = diff_records(&single_records, &current_ips);
        for v in dels {
            cf::delete_arecord(&cf_client, &cf_zone_id, &v.id).await?;
        }
        for v in adds {
            let prefix = get_node_prefix(v, &self.cluster_host);
            cf::add_arecord(&cf_client, &cf_zone_id, &prefix, v, true).await?;
        }
        Ok(())
    }
    async fn update_cluster_json(&self, nodes: &[node_source::Node]) -> Result<()> {
        let js = self.create_cluster_json(nodes)?;
        self.upload_cluster_json(&js).await?;
        Ok(())
    }
    async fn upload_cluster_json(&self, js: &str) -> Result<()> {
        use aws_sdk_s3 as s3;

        let config = load_aws_config(&Some(self.aws_region.to_owned())).await;
        let client = s3::Client::new(&config);
        use s3::types::ByteStream;
        let body = ByteStream::from(js.as_bytes().to_vec());
        client
            .put_object()
            .bucket(self.cluster_json_s3_path.bucket.to_owned())
            .key(self.cluster_json_s3_path.key.to_owned())
            .content_type("application/json".to_owned())
            .body(body)
            .send()
            .await?;
        Ok(())
    }
    fn create_cluster_json(&self, nodes: &[node_source::Node]) -> Result<String> {
        let data = data::NodeListData {
            nodes: nodes
                .iter()
                .map(|v| data::NodeListNode {
                    host: get_node_host(&v.public_ip, &self.cluster_host),
                })
                .collect(),
        };
        serde_json::to_string(&data).map_err(|e| anyhow!(e))
    }
    async fn get_nodes(&self) -> Result<Vec<node_source::Node>> {
        node_source::load_nodes_from_ec2(&self.role, &self.stage, &self.aws_region).await
    }
}

struct RecordGroups<'a> {
    cluster_records: Vec<&'a cf::CfARecord>,
    single_records: Vec<&'a cf::CfARecord>,
}
fn split_records<'a>(
    records: &'a [cf::CfARecord],
    prefix: &str,
    base_domain: &str,
    cluster_host: &str,
) -> RecordGroups<'a> {
    let mut cluster_records = Vec::<&cf::CfARecord>::new();
    let mut single_records = Vec::<&cf::CfARecord>::new();

    for record in records {
        if record.name == cluster_host {
            cluster_records.push(record);
        }
    }

    let suffix = format!(".{}", base_domain);
    if prefix.is_empty() {
        let ln = NODE_ID_LEN + suffix.len();
        for record in records {
            if record.name.len() == ln && record.name.ends_with(&suffix) {
                single_records.push(record);
            }
        }
    } else {
        let prefix = format!("{}-", prefix);
        let ln = NODE_ID_LEN + prefix.len() + suffix.len();
        for record in records {
            if record.name.len() == ln
                && record.name.starts_with(&prefix)
                && record.name.ends_with(&suffix)
            {
                single_records.push(record);
            }
        }
    }

    RecordGroups {
        cluster_records,
        single_records,
    }
}

struct DiffResult<'a> {
    adds: Vec<&'a str>,
    dels: Vec<&'a cf::CfARecord>,
}

fn diff_records<'a>(records: &'a [&cf::CfARecord], current_ips: &'a [&str]) -> DiffResult<'a> {
    use std::collections::HashSet;
    let prev_ip_set: HashSet<&str> = HashSet::from_iter(records.iter().map(|v| v.ip.as_str()));
    let current_ip_set: HashSet<&str> = HashSet::from_iter(current_ips.iter().cloned());
    let mut adds = Vec::<&str>::new();
    let mut dels = Vec::<&cf::CfARecord>::new();

    for v in current_ips {
        if !prev_ip_set.contains(v) {
            adds.push(v);
        }
    }
    for v in records {
        if !current_ip_set.contains(v.ip.as_str()) {
            dels.push(v);
        }
    }

    DiffResult { adds, dels }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    #[tokio::test]
    async fn test_manager() {
        let cf_email = env::var("CLOUDFLARE_EMAIL").unwrap();
        let cf_api_key = env::var("CLOUDFLARE_API_KEY").unwrap();

        let mgr = Manager::new(
            "entrance.verseengine.cloud",
            "dev",
            "CellServer",
            "ap-northeast-1",
            S3Path {
                bucket: "mdev-test-data".into(),
                key: "cluster/cluster-dev.json".into(),
            },
            CfAuthInfo {
                email: cf_email,
                api_key: cf_api_key,
            },
        );
        mgr.update().await.unwrap();
    }
    #[test]
    fn test_split_records() {
        let records = vec![
            cf::CfARecord {
                id: "1".into(),
                name: "entrance.verseengine.cloud".into(),
                ip: "1.2.3.4".into(),
            },
            cf::CfARecord {
                id: "2".into(),
                name: "entrance-1111111111111111.verseengine.cloud".into(),
                ip: "1.2.3.4".into(),
            },
            cf::CfARecord {
                id: "3".into(),
                name: "entrance-1111111111111112.verseengine.cloud".into(),
                ip: "1.2.3.4".into(),
            },
            cf::CfARecord {
                id: "4".into(),
                name: "entrance-1.verseengine.cloud".into(),
                ip: "1.2.3.4".into(),
            },
        ];
        let RecordGroups {
            cluster_records,
            single_records,
        } = split_records(
            &records,
            "entrance",
            "verseengine.cloud",
            "entrance.verseengine.cloud",
        );
        assert_eq!(cluster_records.len(), 1);
        assert_eq!(&cluster_records[0].id, "1");
        assert_eq!(single_records.len(), 2);
        assert_eq!(&single_records[0].id, "2");
        assert_eq!(&single_records[1].id, "3");
        //
    }
    #[test]
    fn test_diff_records() {
        let records = vec![];
        let current_ips = vec!["1.1.1.0"];
        let DiffResult { adds, dels } = diff_records(&records, &current_ips);
        assert!(dels.is_empty());
        assert_eq!(adds.len(), 1);
        assert_eq!(adds[0], "1.1.1.0");

        let records = vec![
            cf::CfARecord {
                id: "1".into(),
                name: "entrance.verseengine.cloud".into(),
                ip: "1.1.1.0".into(),
            },
            cf::CfARecord {
                id: "2".into(),
                name: "entrance.verseengine.cloud".into(),
                ip: "1.1.1.1".into(),
            },
        ];
        let record_refs: Vec<&cf::CfARecord> = records.iter().collect();
        let current_ips = vec![];
        let DiffResult { adds, dels } = diff_records(&record_refs, &current_ips);
        assert!(adds.is_empty());
        assert_eq!(dels.len(), 2);

        let records = vec![
            cf::CfARecord {
                id: "1".into(),
                name: "entrance.verseengine.cloud".into(),
                ip: "1.1.1.0".into(),
            },
            cf::CfARecord {
                id: "2".into(),
                name: "entrance.verseengine.cloud".into(),
                ip: "1.1.1.1".into(),
            },
        ];
        let current_ips = vec!["1.1.1.1", "1.1.1.2"];
        let record_refs: Vec<&cf::CfARecord> = records.iter().collect();
        let DiffResult { adds, dels } = diff_records(&record_refs, &current_ips);
        assert_eq!(adds.len(), 1);
        assert_eq!(dels.len(), 1);
        assert_eq!(&dels[0].id, "1");
        assert_eq!(adds[0], "1.1.1.2");
    }
}
