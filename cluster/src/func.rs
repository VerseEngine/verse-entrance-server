use sha3::digest::*;
use sha3::Shake256;

pub const NODE_ID_LEN: usize = 16;

pub fn get_node_id(ip: &str) -> String {
    let mut hasher = Shake256::default();
    hasher.update(ip.as_bytes());
    let res = hasher.finalize_boxed(NODE_ID_LEN / 2);
    hex::encode(res)
}
pub fn get_node_host(ip: &str, cluster_host: &str) -> String {
    let (prefix, base_domain) = split_host(cluster_host);
    if prefix.is_empty() {
        format!("{}.{}", get_node_id(ip), base_domain)
    } else {
        format!("{}-{}.{}", prefix, get_node_id(ip), base_domain)
    }
}
pub fn get_node_prefix(ip: &str, cluster_host: &str) -> String {
    let (prefix, _) = split_host(cluster_host);
    if prefix.is_empty() {
        get_node_id(ip)
    } else {
        format!("{}-{}", prefix, get_node_id(ip))
    }
}
pub fn split_host(host: &str) -> (String, String) {
    let ar: Vec<&str> = host.splitn(2, '.').collect();
    if ar.len() == 2 && ar[1].find('.').is_some() {
        (ar[0].into(), ar[1].into())
    } else {
        ("".into(), host.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_get_node_id() {
        assert_eq!(get_node_id(""), "46b9dd2b0ba88d13");
        assert_eq!(get_node_id("1.2.3.4"), "76f67dfc1573f0e7");
        assert_eq!(get_node_id("1.2.3.5"), "ca56ce073bbcd5d5");
    }
    #[test]
    fn test_get_node_host() {
        assert_eq!(
            get_node_host("1.2.3.4", "verseengine.cloud"),
            "76f67dfc1573f0e7.verseengine.cloud"
        );
        assert_eq!(
            get_node_host("1.2.3.4", "entrance.verseengine.cloud"),
            "entrance-76f67dfc1573f0e7.verseengine.cloud"
        );
    }
    #[test]
    fn test_split_host() {
        assert_eq!(
            split_host("verseengine.cloud"),
            ("".to_owned(), "verseengine.cloud".to_owned())
        );
        assert_eq!(
            split_host("entrance.verseengine.cloud"),
            ("entrance".to_owned(), "verseengine.cloud".to_owned())
        );
        assert_eq!(
            split_host("entrance.a.verseengine.cloud"),
            ("entrance".to_owned(), "a.verseengine.cloud".to_owned())
        );
    }
}
