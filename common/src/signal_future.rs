use super::sleep;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

cfg_if::cfg_if! {
    if #[cfg(target_family = "wasm")] {
        use std::rc::Rc;
        use std::cell::RefCell;
        pub struct SignalFuture<T, E> {
            inner: Rc<RefCell<Inner<T, E>>>,
        }
    } else {
        use std::sync::Arc;
        use atomic_refcell::AtomicRefCell;
        pub struct SignalFuture<T, E> {
            inner: Arc<AtomicRefCell<Inner<T, E>>>,
        }
    }
}
impl<T, E> SignalFuture<T, E> {
    pub fn new() -> Self {
        let inner = Inner::<T, E> {
            result: None,
            task: None,
        };
        cfg_if::cfg_if! {
            if #[cfg(target_family = "wasm")] {
                SignalFuture::<T, E> {
                    inner: Rc::new(RefCell::new(inner)),
                }
            }else{
                SignalFuture::<T, E> {
                    inner: Arc::new(AtomicRefCell::new(inner)),
                }
            }
        }
    }
    pub fn resolve(&self, v: T) {
        self.finish(Ok(v));
    }
    pub fn reject(&self, v: E) {
        self.finish(Err(v));
    }
    pub fn finish(&self, v: Result<T, E>) {
        if let Some(task) = self.set_result(v) {
            task.wake()
        }
    }
    cfg_if::cfg_if! {
        if #[cfg(target_family = "wasm")] {
            fn set_result(&self, v: Result<T, E>) -> Option<Waker> {
                let mut inner = self.inner.borrow_mut();
                inner.result = Some(v);
                inner.task.take()
            }
            fn set_task(&self, v: Option<Waker>) {
                let mut inner = self.inner.borrow_mut();
                inner.task = v;
            }
            fn take_result(&self) -> Option<Result<T, E>> {
                let mut inner = self.inner.borrow_mut();
                inner.result.take()
            }
        } else {
            fn set_result(&self, v: Result<T, E>) -> Option<Waker> {
                #[allow(clippy::never_loop)]
                loop {
                    let Ok(mut inner) = self.inner.try_borrow_mut() else {
                        continue;
                    };
                    inner.result = Some(v);
                    return inner.task.take();
                }
            }
            fn set_task(&self, v: Option<Waker>) {
                #[allow(clippy::never_loop)]
                loop {
                    let Ok(mut inner) = self.inner.try_borrow_mut() else {
                        continue;
                    };
                    inner.task = v;
                    return;
                }
            }
            fn take_result(&self) -> Option<Result<T, E>> {
                #[allow(clippy::never_loop)]
                loop {
                    let Ok(mut inner) = self.inner.try_borrow_mut() else {
                        continue;
                    };
                    return inner.result.take();
                }
            }
        }
    }
}
cfg_if::cfg_if! {
    if #[cfg(target_family = "wasm")] {
        impl<T> SignalFuture<T, anyhow::Error>
        where
            T: Send + 'static,
        {
            /// Usage: sf.set_timeout(RPC_TIMEOUT, Box::new(|| errors::timeout!()()));
            pub fn set_timeout(&self, ms: u32, f: Box<dyn FnOnce() -> anyhow::Error + Send>) {
                let _self = self.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    sleep(ms).await;
                    _self.reject(f());
                });
            }
        }
    } else {
        impl<T> SignalFuture<T, anyhow::Error>
        where
            T: Send + Sync + 'static,
        {
            pub fn set_timeout(&self, ms: u32, f: Box<dyn FnOnce() -> anyhow::Error + Send>) {
                let _self = self.clone();
                tokio::task::spawn(async move {
                    sleep(ms).await;
                    _self.reject(f());
                });
            }
        }
    }
}

impl<T, E> Default for SignalFuture<T, E> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, E> Future for SignalFuture<T, E> {
    type Output = Result<T, E>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        if let Some(val) = self.take_result() {
            return Poll::Ready(val);
        }
        self.set_task(Some(cx.waker().clone()));
        Poll::Pending
    }
}

impl<T, E> Clone for SignalFuture<T, E> {
    fn clone(&self) -> Self {
        SignalFuture::<T, E> {
            inner: self.inner.clone(),
        }
    }
}

struct Inner<T, E> {
    result: Option<Result<T, E>>,
    task: Option<Waker>,
}

#[cfg(not(target_family = "wasm"))]
#[cfg(test)]
mod tests {
    use super::super::sleep;
    use super::*;

    #[tokio::test]
    async fn test_resolve() {
        /* let sf = Rc::new(RefCell::new(SignalFuture::<bool, bool>::new()));
        let sf1 = Rc::clone(&sf); */
        let sf = SignalFuture::<bool, bool>::new();
        let sf1 = sf.clone();
        tokio::task::spawn(async move {
            sleep(1000).await;
            sf1.resolve(true);
        });
        let res = sf.await;
        assert!(res.is_ok());
    }
    #[tokio::test]
    async fn test_reject() {
        let sf = SignalFuture::<bool, bool>::new();
        let sf1 = sf.clone();
        tokio::task::spawn(async move {
            sf1.reject(true);
        });
        let res = sf.await;
        assert!(res.is_err());
    }
    #[tokio::test]
    async fn test_resolve_no_await() {
        let sf = SignalFuture::<bool, bool>::new();
        let sf1 = sf.clone();
        tokio::task::spawn(async move {
            sf1.resolve(true);
        });
        sleep(1000).await;
        assert_eq!(true, true);
    }
    #[tokio::test]
    async fn test_reject_no_await() {
        let sf = SignalFuture::<bool, bool>::new();
        let sf1 = sf.clone();
        tokio::task::spawn(async move {
            sf1.reject(true);
        });
        sleep(1000).await;
        assert_eq!(true, true);
    }
    #[tokio::test]
    async fn test_timeout() {
        let sf = SignalFuture::<bool, anyhow::Error>::default();
        sf.set_timeout(1000, Box::new(|| anyhow::anyhow!("timeout error")));
        sleep(2000).await;
        let res = sf.await;
        assert!(res.is_err());
        assert_eq!(res.err().unwrap().to_string(), "timeout error".to_string());
    }
}
