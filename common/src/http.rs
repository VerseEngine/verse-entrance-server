cfg_if::cfg_if! {
    if #[cfg(target_family = "wasm")] {
        pub use js::post_string as post_string;
    } else {
        pub use nojs::post_string as post_string;
    }
}

#[cfg(target_family = "wasm")]
pub mod js {
    use crate as verse_common;
    use crate::errors;
    use anyhow::Result;
    use wasm_bindgen::{JsCast, JsValue};
    use wasm_bindgen_futures::*;
    use web_sys::{Request, RequestInit, RequestMode, Response};
    pub async fn post_string(url: String, body: String, content_type: String) -> Result<String> {
        let mut opts = RequestInit::new();
        opts.method("POST");
        opts.mode(RequestMode::Cors);
        opts.body(Some(&JsValue::from_str(&body)));
        let request = Request::new_with_str_and_init(&url, &opts).map_err(errors::js!())?;
        /* request
        .headers()
        .set("Accept", "application/json")
        .map_err(errors::js!())?; */
        request
            .headers()
            .set("Content-Type", &content_type)
            .map_err(errors::js!())?;

        let window = web_sys::window().ok_or_else(errors::required!())?;
        let resp_value = JsFuture::from(window.fetch_with_request(&request))
            .await
            .map_err(errors::js!())?;
        let resp: Response = resp_value.dyn_into().map_err(errors::js!())?;
        if !resp.ok() {
            return Err(errors::http!(&resp));
        }
        let json = JsFuture::from(resp.text().map_err(errors::js!())?)
            .await
            .map_err(errors::js!())?
            .as_string()
            .ok_or_else(errors::required!())?;
        Ok(json)
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        use wasm_bindgen_test::wasm_bindgen_test_configure;
        use wasm_bindgen_test::*;
        wasm_bindgen_test_configure!(run_in_browser);

        #[wasm_bindgen_test]
        async fn test_js() {
            let res = post_string(
                "https://httpbin.org/post".to_string(),
                r#"{"input": "hello"}"#.to_string(),
                "application/json".to_string(),
            )
            .await;
            println!("{:?}", res);
            assert!(res.is_ok());
            assert!(
                res.as_ref().unwrap().contains(r#""hello""#),
                "post response check: {}",
                res.as_ref().unwrap()
            );
        }
    }
}
#[cfg(not(target_family = "wasm"))]
pub mod nojs {
    use crate as verse_common;
    use anyhow::Result;
    pub async fn post_string(url: String, body: String, content_type: String) -> Result<String> {
        let client = reqwest::Client::new();
        let res = client
            .post(url)
            .header(reqwest::header::CONTENT_TYPE, content_type)
            .body(body)
            .send()
            .await?;
        if !res.status().is_success() {
            return Err(verse_common::errors::_http(
                res.status().as_u16(),
                file!().to_string(),
                line!(),
            ));
        }

        let res = res.text().await?;
        Ok(res)
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        #[tokio::test]
        async fn test_nojs() {
            let res = post_string(
                "https://dns.google/dns-query".to_string(),
                "q80BAAABAAAAAAAAA3d3dwdleGFtcGxlA2NvbQAAAQAB".to_string(),
                "application/dns-message".to_string(),
            )
            .await;
            // println!("{:?}", res);
            assert!(res.is_ok());
        }
    }
}
