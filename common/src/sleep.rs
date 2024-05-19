/*
use verse_common::prelude::*;
use verse_common::SignalFuture;
use anyhow::Error; */

cfg_if::cfg_if! {
    if #[cfg(target_family = "wasm")] {
        pub fn sleep(ms: u32) -> impl std::future::Future {
            use js_sys::Promise;
            use wasm_bindgen_futures::JsFuture;

            let p = Promise::new(&mut |resolve, _| {
                web_sys::window()
                    .unwrap()
                    .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms as i32)
                    .unwrap();
            });
            JsFuture::from(p)
        }
    } else {
        pub fn sleep(ms: u32) -> impl std::future::Future {
            use tokio::time::{sleep, Duration};
            sleep(Duration::from_millis(ms as u64))
        }
    }
}
/* #[allow(dead_code)]
pub async fn timeout<'a, F>(timeout: i32, f: &'a F) -> Result<(), Error>
where
    F: Future<Output = Result<(), Error>>,
{
    let sf = SignalFuture::<(), Error>::new();
    let sf1 = sf.clone();
    wasm_bindgen_futures::spawn_local(async move {
        sleep(timeout).await;
        sf1.reject(errors::timeout!()());
    });
    let sf2 = sf.clone();
    wasm_bindgen_futures::spawn_local(async move {
        match f.await {
            Ok(v) => {
                sf2.resolve(v);
            }
            Err(v) => {
                sf2.reject(v);
            }
        }
    });
    sf.await
} */

/* #[allow(dead_code)]
pub async fn timeout<'a, T, F>(f: &'a F, timeout: i32) -> Result<&'a T, Error>
where
    F: Future<Output = Result<T, Error>>,
{
    let sf = SignalFuture::<T, Error>::new();
    let sf1 = sf.clone();
    wasm_bindgen_futures::spawn_local(async move {
        sleep(timeout).await;
        sf1.reject(errors::timeout!()());
    });
    let sf2 = sf.clone();
    wasm_bindgen_futures::spawn_local(async move {
        match f.await {
            Ok(v) => {
                sf2.resolve(v);
            }
            Err(v) => {
                sf2.reject(v);
            }
        }
    });
    sf.await
} */
