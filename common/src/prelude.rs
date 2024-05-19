pub use crate::errors;
pub use crate::log::logmsg;
pub use crate::time::*;
pub use crate::with_log::WithLog;

pub fn url_join(base_url: &str, url: &str) -> String {
    if base_url.ends_with('/') {
        if let Some(url) = url.strip_prefix('/') {
            format!("{}{}", base_url, url)
        } else {
            format!("{}{}", base_url, url)
        }
    } else if url.starts_with('/') {
        format!("{}{}", base_url, url)
    } else {
        format!("{}/{}", base_url, url)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_url_join() {
        assert_eq!(&url_join("example.com", "hello"), "example.com/hello");
        assert_eq!(&url_join("example.com/", "/hello"), "example.com/hello");
        assert_eq!(&url_join("example.com/", "hello"), "example.com/hello");
        assert_eq!(&url_join("example.com", "/hello"), "example.com/hello");
    }
}
