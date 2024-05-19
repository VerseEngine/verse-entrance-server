use anyhow::Result;
use clap::Parser;
#[allow(unused_imports)]
use log::{debug, error, info, warn};
use std::fmt;
use std::path::PathBuf;

#[derive(Parser)]
pub struct Args {
    #[clap(long, env)]
    pub http_host: Option<String>,
    #[clap(long, env)]
    pub use_https: bool,
    #[clap(long)]
    pub lets_encrypt_email: Vec<String>,

    #[clap(long, value_parser)]
    pub cache: Option<PathBuf>,

    #[clap(long, default_value = "8000")]
    pub http_port: u16,
    #[clap(long, default_value = "8000")]
    pub udp_port: u16,
    #[clap(long, default_value = "9098")]
    pub status_port: u16,

    #[clap(long)]
    pub max_connections: Option<usize>,
    #[clap(long)]
    pub max_connections_by_url: Option<usize>,

    #[clap(long, default_value = "10")]
    pub max_routing_results: usize,

    #[clap(long, env)]
    pub public_ip: Option<String>,

    #[clap(long, default_value = "stun:stun.l.google.com:19302")]
    pub ice_servers: Vec<String>,

    #[clap(long, env)]
    pub cloudflare_api_key: Option<String>,
    #[clap(long, env)]
    pub cloudflare_email: Option<String>,

    #[clap(long, env)]
    pub aws_secretsmanager_name: Option<String>,
    #[clap(long, env)]
    pub aws_secretsmanager_region: Option<String>,
    #[clap(long, env)]
    pub aws_secretsmanager_enabled: bool,

    #[clap(long, env)]
    pub aws_ec2_instance_id: Option<String>,
    #[clap(long, env)]
    pub aws_ec2_region: Option<String>,

    #[clap(long, env)]
    pub prometheus_prefix: Option<String>,

    #[clap(long, env)]
    pub access_log_path: Option<String>,

    #[clap(long, env)]
    pub http_log_path: Option<String>,

    #[clap(long, env)]
    pub cluster_node_list_url: Option<String>,

    pub cluster_node_host: Option<String>,

    #[clap(long, env)]
    pub cluster_node_role: Option<String>,
    #[clap(long, env)]
    pub cluster_node_stage: Option<String>,
    #[clap(long, env)]
    pub cluster_json_s3_bucket: Option<String>,
    #[clap(long, env)]
    pub cluster_json_s3_key: Option<String>,
    #[clap(long, env)]
    pub update_cluster_key: Option<String>,
}

impl Args {
    pub async fn load() -> Result<Self> {
        let mut args = Args::parse();
        let tags = if let Some(ref instance_id) = args.aws_ec2_instance_id {
            load_ec2_tags(instance_id, &args.aws_ec2_region).await?
        } else {
            Vec::new()
        };

        if let Some(v) = args.aws_secretsmanager_name {
            args.aws_secretsmanager_name = Some(Self::replace_tags(v, &tags));
        }
        if let Some(v) = args.aws_secretsmanager_region {
            args.aws_secretsmanager_region = Some(Self::replace_tags(v, &tags));
        }

        args.load_to_env().await?;

        let mut args = Args::parse();
        args.set_other_args();

        Ok(args)
    }
    fn set_other_args(&mut self) {
        if let (Some(http_host), Some(public_ip)) =
            (self.http_host.as_ref(), self.public_ip.as_ref())
        {
            self.cluster_node_host = Some(verse_cluster::get_node_host(public_ip, http_host));
        }
    }
    pub async fn load_to_env(&self) -> Result<()> {
        if self.aws_secretsmanager_enabled {
            if let Some(ref secret_name) = self.aws_secretsmanager_name {
                load_secrets(secret_name, &self.aws_secretsmanager_region).await?;
            }
        }
        Ok(())
    }
    fn replace_tags(v: String, tags: &Vec<(String, String)>) -> String {
        let mut v = v;
        for t in tags {
            let old = format!("{{tag:{}}}", t.0);
            v = v.replace(&old, &t.1);
        }
        v
    }
}

impl fmt::Debug for Args {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Args")
            .field("http_host", &self.http_host)
            .field("use_https", &self.use_https)
            .field("lets_encrypt_email", &self.lets_encrypt_email)
            .field("cache", &self.cache)
            .field("http_port", &self.http_port)
            .field("udp_port", &self.udp_port)
            .field("status_port", &self.status_port)
            .field("max_connections", &self.max_connections)
            .field("max_connections_by_url", &self.max_connections_by_url)
            .field("public_ip", &self.public_ip)
            .field("ice_servers", &self.ice_servers)
            .field(
                "cloudflare_api_key",
                &self
                    .cloudflare_api_key
                    .as_ref()
                    .map(|v| "*".repeat(v.len())),
            )
            .field("cloudflare_email", &self.cloudflare_email)
            .field(
                "aws_secretsmanager_enabled",
                &self.aws_secretsmanager_enabled,
            )
            .field("aws_secretsmanager_name", &self.aws_secretsmanager_name)
            .field("aws_secretsmanager_region", &self.aws_secretsmanager_region)
            .field("aws_ec2_instance_id", &self.aws_ec2_instance_id)
            .field("aws_ec2_region", &self.aws_ec2_region)
            .field("access_log_path", &self.access_log_path)
            .field("http_log_path", &self.http_log_path)
            .field("cluster_node_list_url", &self.cluster_node_list_url)
            .field("cluster_node_host", &self.cluster_node_host)
            .field("cluster_node_role", &self.cluster_node_role)
            .field("cluster_node_stage", &self.cluster_node_stage)
            .field("cluster_json_s3_bucket", &self.cluster_json_s3_bucket)
            .field("cluster_json_s3_key", &self.cluster_json_s3_key)
            .finish()
    }
}

async fn load_ec2_tags(
    instance_id: &str,
    region: &Option<String>,
) -> anyhow::Result<Vec<(String, String)>> {
    use aws_sdk_ec2 as ec2;
    use aws_sdk_ec2::model::Filter;
    use futures::StreamExt;

    let config = load_aws_config(region).await;
    let client = ec2::Client::new(&config);
    let mut p = client
        .describe_tags()
        .set_filters(Some(vec![
            Filter::builder()
                .name("resource-id")
                .values(instance_id)
                .build(),
            Filter::builder()
                .name("resource-type")
                .values("instance")
                .build(),
        ]))
        .into_paginator()
        .send();
    let mut res = Vec::<(String, String)>::new();
    while let Some(result) = p.next().await.transpose()? {
        let Some(ref tags) = result.tags else {
            continue;
        };
        for t in tags {
            let Some(key) = t.key() else {
                continue;
            };
            let Some(value) = t.value() else {
                continue;
            };

            res.push((key.to_owned(), value.to_owned()));
        }
    }

    Ok(res)
}

async fn load_secrets(secret_name: &str, region: &Option<String>) -> anyhow::Result<()> {
    use aws_sdk_secretsmanager as secretsmanager;
    use std::collections::HashMap;
    use std::env;

    let config = load_aws_config(region).await;
    let client = secretsmanager::Client::new(&config);
    let resp = client
        .get_secret_value()
        .secret_id(secret_name)
        .send()
        .await?;

    let Some(secret_string) = resp.secret_string() else {
        return Ok(());
    };

    let js: HashMap<String, String> = serde_json::from_str(secret_string)?;

    for (k, v) in js.iter() {
        let k = k.replace('-', "_").to_uppercase();
        env::set_var(k, v);
    }

    Ok(())
}

async fn load_aws_config(region: &Option<String>) -> aws_config::SdkConfig {
    use aws_types::region::Region;
    if let Some(ref region) = region {
        aws_config::from_env()
            .region(Region::new(region.clone()))
            .load()
            .await
    } else {
        aws_config::from_env().load().await
    }
}
