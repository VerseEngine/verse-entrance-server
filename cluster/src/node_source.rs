use crate::aws::load_aws_config;
use anyhow::Result;
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Node {
    pub public_ip: String,
}

pub async fn load_nodes_from_ec2(stage: &str, role: &str, region: &str) -> Result<Vec<Node>> {
    use aws_sdk_ec2 as ec2;
    use aws_sdk_ec2::model::Filter;
    use futures::StreamExt;

    let config = load_aws_config(&Some(region.to_owned())).await;
    let client = ec2::Client::new(&config);
    let mut p = client
        .describe_instances()
        .set_filters(Some(vec![
            Filter::builder()
                .name("instance-state-name")
                .values("running")
                .build(),
            Filter::builder().name("tag:Role").values(role).build(),
            Filter::builder().name("tag:Stage").values(stage).build(),
        ]))
        .into_paginator()
        .send();
    let mut res = Vec::<Node>::new();
    while let Some(result) = p.next().await.transpose()? {
        let Some(reservations) = result.reservations() else {
            continue;
        };
        for reservation in reservations {
            for instance in reservation.instances().unwrap_or_default() {
                if let Some(public_ip) = instance.public_ip_address() {
                    res.push(Node {
                        public_ip: public_ip.to_owned(),
                    });
                }
            }
        }
    }
    Ok(res)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn test_load_nodes_from_ec2() {
        let res = load_nodes_from_ec2("dev", "CellServer", "ap-northeast-1").await;
        // println!("{:?}", res);
        assert!(res.is_ok());
        for node in res.unwrap().iter() {
            assert_ne!(node.public_ip.len(), 0);
        }
    }
}
