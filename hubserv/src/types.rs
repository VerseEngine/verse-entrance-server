use anyhow::{Error, Result};
#[allow(unused_imports)]
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use url::Url;
use verse_session_id::*;
use webrtc::ice_transport::ice_candidate::RTCIceCandidateInit;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;

const MAX_URL_LEN: usize = 4096;

#[derive(Deserialize)]
pub struct SignedRequest {
    #[serde(rename = "sessionId")]
    pub session_id: String,
    pub sign: SignatureSet,
    pub payload: String,
}

impl SignedRequest {
    pub fn verify<T>(&self) -> Result<(verse_session_id::SessionId, T), Error>
    where
        T: for<'a> serde::Deserialize<'a> + RequestPayload,
    {
        let session_id = self.session_id.parse::<SessionId>()?;
        session_id.verify(vec![self.payload.as_bytes()], &self.sign)?;

        let mut res: T = serde_json::from_str(&self.payload)?;
        res.normalize();
        Ok((session_id, res))
    }
}

pub trait RequestPayload {
    fn normalize(&mut self) {}
}

#[derive(Default, Serialize, Deserialize)]
pub struct EnterRequestPayload {
    pub url: String,
    pub sdp: RTCSessionDescription,

    #[serde(skip)]
    pub raw_url: String,
}
impl RequestPayload for EnterRequestPayload {
    fn normalize(&mut self) {
        // 収集するURLはnormalize前の状態を使う
        self.raw_url = self.url.clone();

        if self.raw_url.len() > MAX_URL_LEN
            || self.raw_url.contains('\r')
            || self.raw_url.contains('\n')
        {
            warn!(
                "Incorrect URL received. {}...",
                if self.raw_url.len() > 128 {
                    &self.raw_url[..128]
                } else {
                    &self.raw_url
                }
            );
            self.url = "".into(); // Bad Requestにする
            return;
        }

        self.url = normalize_url(&self.url);
    }
}

#[derive(Default, Serialize, Deserialize)]
pub struct CandidateRequestPayload {
    pub url: String,
    pub sdp: RTCIceCandidateInit,

    #[serde(skip)]
    pub raw_url: String,
}
impl RequestPayload for CandidateRequestPayload {
    fn normalize(&mut self) {
        // 収集するURLはnormalize前の状態を使う
        self.raw_url = self.url.clone();

        if self.raw_url.len() > MAX_URL_LEN
            || self.raw_url.contains('\r')
            || self.raw_url.contains('\n')
        {
            warn!(
                "Incorrect URL received. {}...",
                if self.raw_url.len() > 128 {
                    &self.raw_url[..128]
                } else {
                    &self.raw_url
                }
            );
            self.url = "".into(); // Bad Requestにする
            return;
        }

        self.url = normalize_url(&self.url);
    }
}

#[derive(Serialize, Deserialize)]
pub struct EnterResponse {
    pub sdp: RTCSessionDescription,
}
#[derive(Serialize, Deserialize)]
pub struct EmptyResponse {}

pub fn normalize_url(u: &str) -> String {
    let mut res = String::with_capacity(u.len());
    let Ok(u) = Url::parse(u.to_string().trim()) else {
        return "".into();
    };

    res.push_str(u.scheme());
    res.push_str("://");
    let Some(host) = u.host_str() else {
        return "".into();
    };
    res.push_str(host);
    res.push_str(u.path());
    if res.ends_with('/') {
        res.pop();
    }
    res
}

#[cfg(test)]
mod tests {
    use super::*;
    // use verse_session_id::*;
    #[test]
    fn test_signed_request() {
        let payload = EnterRequestPayload {
            url: "https://example.com".to_string(),
            raw_url: "https://example.com".to_string(),
            sdp: Default::default(),
        };
        let session_id_pair = new_session_id_pair().unwrap();
        let payload_str = serde_json::to_string(&payload).unwrap();
        let sign = session_id_pair.sign(vec![payload_str.as_bytes()]).unwrap();

        let req = SignedRequest {
            session_id: session_id_pair.get_id().to_string(),
            payload: payload_str,
            sign,
        };

        let (id, p1) = req.verify::<EnterRequestPayload>().unwrap();
        assert_eq!(id, session_id_pair.get_id());
        assert_eq!(payload.url, p1.url);
    }
    #[test]
    fn test_normalize_url() {
        assert_eq!(
            &normalize_url(r##"https://example.com"##),
            r##"https://example.com"##
        );
        assert_eq!(
            &normalize_url(r##"https://example.com/"##),
            r##"https://example.com"##
        );
        assert_eq!(
            &normalize_url(r##" https://example.com/ "##),
            r##"https://example.com"##
        );
        assert_eq!(
            &normalize_url(r##"　https://example.com/　　　"##),
            r##"https://example.com"##
        );
        assert_eq!(
            &normalize_url(r##"https://example.com/index.html"##),
            r##"https://example.com/index.html"##
        );
        assert_eq!(
            &normalize_url(r##"https://example.com/index.html?a=1"##),
            r##"https://example.com/index.html"##
        );
        assert_eq!(
            &normalize_url(r##"https://example.com/index.html?#hash"##),
            r##"https://example.com/index.html"##
        );
    }
}
