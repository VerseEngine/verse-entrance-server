use anyhow::Result;
use cloudflare::endpoints::{dns, zone};
use cloudflare::framework::{
    async_api::{ApiClient, Client},
    auth::Credentials,
    Environment, HttpApiClientConfig,
};
use std::net::Ipv4Addr;

pub async fn set_arecord(
    name: &str,
    ip: &str,
    cf_email: &str,
    cf_api_key: &str,
    proxy: bool,
) -> Result<()> {
    let credentials = Credentials::UserAuthKey {
        email: cf_email.to_string(),
        key: cf_api_key.to_string(),
    };
    let client = Client::new(
        credentials,
        HttpApiClientConfig::default(),
        Environment::Production,
    )?;

    let ar: Vec<&str> = name.splitn(2, '.').collect();
    let domain = ar[ar.len() - 1].to_string();

    let res = client
        .request(&zone::ListZones {
            params: zone::ListZonesParams {
                name: Some(domain.clone()),
                ..Default::default()
            },
        })
        .await?;
    if res.result.len() != 1 {
        return Err(anyhow::anyhow!("unsupported host name: {}", domain));
    }
    let zone_id = res.result[0].id.to_owned();

    let ip: Ipv4Addr = ip.parse()?;

    let res: Vec<dns::DnsRecord> = client
        .request(&dns::ListDnsRecords {
            zone_identifier: &zone_id,
            params: dns::ListDnsRecordsParams {
                name: Some(name.to_string()),
                // record_type: Some(dns::DnsContent::A { content: ip }),
                ..Default::default()
            },
        })
        .await?
        .result
        .into_iter()
        .filter(|v| matches!(v.content, dns::DnsContent::A { content: _ }))
        .collect();
    if res.is_empty() {
        client
            .request(&dns::CreateDnsRecord {
                zone_identifier: &zone_id,
                params: dns::CreateDnsRecordParams {
                    proxied: Some(proxy),
                    name: ar[0],
                    content: dns::DnsContent::A { content: ip },
                    ttl: None,
                    priority: None,
                },
            })
            .await?;
    } else {
        client
            .request(&dns::UpdateDnsRecord {
                zone_identifier: &zone_id,
                identifier: &res[0].id,
                params: dns::UpdateDnsRecordParams {
                    proxied: Some(proxy),
                    name: ar[0],
                    content: dns::DnsContent::A { content: ip },
                    ttl: None,
                },
            })
            .await?;
    }

    Ok(())
}
