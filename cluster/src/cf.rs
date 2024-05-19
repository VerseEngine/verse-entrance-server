use anyhow::Result;
use cloudflare::endpoints::{dns, zone};
use cloudflare::framework::{
    async_api::{ApiClient, Client},
    auth::Credentials,
    Environment, HttpApiClientConfig,
};
use std::net::Ipv4Addr;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CfAuthInfo {
    pub email: String,
    pub api_key: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CfARecord {
    pub id: String,
    pub name: String,
    pub ip: String,
}

pub fn create_client(cf_auth: &CfAuthInfo) -> Result<Client> {
    let credentials = Credentials::UserAuthKey {
        email: cf_auth.email.clone(),
        key: cf_auth.api_key.clone(),
    };
    Client::new(
        credentials,
        HttpApiClientConfig::default(),
        Environment::Production,
    )
}

pub async fn get_arecords(client: &Client, zone_id: &str) -> Result<Vec<CfARecord>> {
    let res: Vec<dns::DnsRecord> = client
        .request(&dns::ListDnsRecords {
            zone_identifier: zone_id,
            params: dns::ListDnsRecordsParams {
                per_page: Some(50000),
                // name: Some(name.to_string()),
                // record_type: Some(dns::DnsContent::A { content: ip }),
                ..Default::default()
            },
        })
        .await?
        .result
        .into_iter()
        .filter(|v| matches!(v.content, dns::DnsContent::A { content: _ }))
        .collect();
    Ok(res
        .into_iter()
        .filter_map(|v| match v.content {
            dns::DnsContent::A { content } => Some(CfARecord {
                id: v.id,
                name: v.name,
                ip: content.to_string(),
            }),
            _ => None,
        })
        .collect())
}
pub async fn add_arecord(
    client: &Client,
    zone_id: &str,
    prefix: &str,
    ip: &str,
    proxy: bool,
) -> Result<()> {
    let ip: Ipv4Addr = ip.parse()?;
    client
        .request(&dns::CreateDnsRecord {
            zone_identifier: zone_id,
            params: dns::CreateDnsRecordParams {
                proxied: Some(proxy),
                name: prefix,
                content: dns::DnsContent::A { content: ip },
                ttl: None,
                priority: None,
            },
        })
        .await?;
    Ok(())
}
pub async fn delete_arecord(client: &Client, zone_id: &str, id: &str) -> Result<()> {
    client
        .request(&dns::DeleteDnsRecord {
            zone_identifier: zone_id,
            identifier: id,
        })
        .await?;
    Ok(())
}
pub async fn get_zone_id(client: &Client, domain: &str) -> Result<Option<String>> {
    let res = client
        .request(&zone::ListZones {
            params: zone::ListZonesParams {
                name: Some(domain.to_owned()),
                ..Default::default()
            },
        })
        .await?;
    if res.result.len() != 1 {
        return Ok(None);
    }
    Ok(Some(res.result[0].id.to_owned()))
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    #[tokio::test]
    async fn test_get_arecords() {
        let cf_email = env::var("CLOUDFLARE_EMAIL").unwrap();
        let cf_api_key = env::var("CLOUDFLARE_API_KEY").unwrap();
        let cf_client = create_client(&CfAuthInfo {
            email: cf_email,
            api_key: cf_api_key,
        })
        .unwrap();
        let cf_zone_id = get_zone_id(&cf_client, "verseengine.cloud")
            .await
            .unwrap()
            .unwrap();

        let res = get_arecords(&cf_client, &cf_zone_id).await.unwrap();
        assert!(!res.is_empty());
        println!("{:?}", res);
    }
}
