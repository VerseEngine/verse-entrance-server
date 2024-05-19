use std::future::Future;

cfg_if::cfg_if! {
    if #[cfg(target_family = "wasm")] {
        pub fn spawn<F>(future: F)
        where
            F: Future<Output = ()> + 'static,
        {
            wasm_bindgen_futures::spawn_local(future);
        }
    } else {
        pub fn spawn<T>(future: T) -> tokio::task::JoinHandle<T::Output>
        where
            T: Future + 'static,
            T::Output: 'static,
        {
            tokio::task::spawn_local(future)
        }
    }
}
