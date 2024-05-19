use thiserror::Error;

#[cfg(target_family = "wasm")]
use wasm_bindgen::prelude::JsError;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("js error: {0} at {1}:{2}")]
    Js(String, String, u32),
    #[error("js bind error: {0}:{1}")]
    JsBind(String, u32),
    #[error("rtc error: {0}:{1}")]
    Rtc(String, u32),
    #[error("http bad request error: {0} at {1}:{2}")]
    HttpBadRequest(u16, String, u32),
    #[error("http server error: {0} at {1}:{2}")]
    HttpServer(u16, String, u32),
    #[error("http error: {0} at {1}:{2}")]
    Http(u16, String, u32),

    #[error("timeout: {0}:{1}")]
    Timeout(String, u32),
    #[error("required: {0}:{1}")]
    Required(String, u32),
    #[error("weak nil: {0}:{1}")]
    WeakNil(String, u32),
    #[error("not impl: {0}:{1}")]
    NotImpl(String, u32),
    #[error("convert: {0}:{1}")]
    Convert(String, u32),
}

impl AppError {
    pub fn is_service_unavailable(&self) -> bool {
        if let AppError::HttpServer(code, _, _) = self {
            return *code == 503;
        }
        false
    }
}

#[macro_export]
macro_rules! weak {
    () => {
        || {
            anyhow::anyhow!(verse_common::errors::AppError::WeakNil(
                file!().to_string(),
                line!()
            ))
        }
    };
}
pub use weak;

#[macro_export]
macro_rules! notimpl {
    () => {
        anyhow::anyhow!(verse_common::errors::AppError::NotImpl(
            file!().to_string(),
            line!()
        ))
    };
}
pub use notimpl;

#[macro_export]
macro_rules! required {
    () => {
        || {
            anyhow::anyhow!(verse_common::errors::AppError::Required(
                file!().to_string(),
                line!()
            ))
        }
    };
}
pub use required;

#[macro_export]
macro_rules! timeout {
    () => {
        || {
            anyhow::anyhow!(verse_common::errors::AppError::Timeout(
                file!().to_string(),
                line!()
            ))
        }
    };
    ( $v:expr ) => {
        || {
            anyhow::anyhow!(verse_common::errors::AppError::Timeout(
                format!("{},   {}", $v, file!().to_string()),
                line!()
            ))
        }
    };
}
pub use timeout;

#[macro_export]
macro_rules! convert {
    () => {
        anyhow::Error::from(verse_common::errors::AppError::Convert(
            file!().to_string(),
            line!(),
        ))
    };
    ( $v:expr ) => {
        anyhow::anyhow!(verse_common::errors::AppError::Convert(
            format!("{},   {}", $v, file!().to_string()),
            line!()
        ))
    };
}
pub use convert;

#[macro_export]
macro_rules! js {
    () => {
        |v| {
            anyhow::anyhow!(verse_common::errors::AppError::Js(
                format!("{:?}", v),
                file!().to_string(),
                line!()
            ))
        }
    };
}
pub use js;

pub fn _http(status: u16, file: String, line: u32) -> anyhow::Error {
    match status {
        400..=499 => anyhow::anyhow!(AppError::HttpBadRequest(status, file, line)),
        500.. => anyhow::anyhow!(AppError::HttpServer(status, file, line)),
        _ => anyhow::anyhow!(AppError::Http(status, file, line)),
    }
}

#[cfg(target_family = "wasm")]
#[macro_export]
macro_rules! http {
    ( $v:expr ) => {
        verse_common::errors::_http($v.status(), file!().to_string(), line!())
    };
}
#[cfg(target_family = "wasm")]
pub use http;

#[cfg(target_family = "wasm")]
pub fn jserr(v: anyhow::Error) -> JsError {
    JsError::new(format!("{0:?}", v).as_str())
}

#[macro_export]
macro_rules! rtc {
    () => {
        anyhow::anyhow!(verse_common::errors::AppError::Rtc(
            file!().to_string(),
            line!()
        ))
    };
    ( $v:expr ) => {{
        anyhow::anyhow!(verse_common::errors::AppError::Rtc(
            format!("{},   {}", $v, file!().to_string()),
            line!()
        ))
    }};
}
pub use rtc;

#[macro_export]
macro_rules! jsbind {
    () => {
        anyhow::anyhow!(verse_common::errors::AppError::JsBind(
            file!().to_string(),
            line!()
        ))
    };
}
pub use jsbind;
