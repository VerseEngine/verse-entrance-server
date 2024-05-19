// use crate::sleep::sleep;
use crate::task;
use futures::channel::mpsc;
use futures::stream::StreamExt;
use futures::Future;
#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};
use std::cell::Cell;
use std::pin::Pin;
use std::rc::Rc;

pub type AsyncJob = Pin<Box<dyn Future<Output = ()> + 'static>>;

pub struct ThrottleJobRunner {
    sender: mpsc::UnboundedSender<AsyncJob>,
    #[allow(dead_code)]
    is_disposed: Rc<Cell<bool>>,
}
impl ThrottleJobRunner {
    pub fn new(max_num_of_parallel_processes: usize) -> Self {
        let (sender, receiver) = mpsc::unbounded::<AsyncJob>();

        let is_disposed = Rc::new(Cell::new(false));
        let mut receiver = receiver.buffered(max_num_of_parallel_processes);
        {
            let is_disposed = is_disposed.clone();
            task::spawn(async move {
                loop {
                    if receiver.next().await.is_none() {
                        break;
                    }
                }

                is_disposed.set(true);
            });
        }
        Self {
            is_disposed,
            sender,
        }
    }
    pub fn add<F>(&self, f: Pin<Box<F>>)
    where
        F: Future<Output = ()> + 'static,
    {
        if let Err(e) = self.sender.unbounded_send(f) {
            error!("{:?}{}:{}", e, file!(), line!());
        }
    }
    pub fn dispose(&self) {
        self.sender.close_channel();
    }
}

#[cfg(not(target_family = "wasm"))]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::sleep;
    use futures::channel::mpsc;
    use futures::stream::StreamExt;
    use tokio::task;

    // #[tokio::test]
    #[tokio::test(flavor = "multi_thread")]
    async fn test_1() {
        let local = task::LocalSet::new();
        local
            .run_until(async move {
                let runner = ThrottleJobRunner::new(1);
                let (mut sender, receiver) = mpsc::unbounded::<i32>();

                {
                    let mut sender = sender.clone();
                    runner.add(Box::pin(async move {
                        sender.unbounded_send(1).unwrap();
                        sleep(500).await;
                        sender.unbounded_send(2).unwrap();
                        sender.disconnect();
                    }));
                }
                {
                    let mut sender = sender.clone();
                    runner.add(Box::pin(async move {
                        sender.unbounded_send(3).unwrap();
                        sleep(100).await;
                        sender.unbounded_send(4).unwrap();
                        sender.disconnect();
                    }));
                }
                {
                    let mut sender = sender.clone();
                    runner.add(Box::pin(async move {
                        sender.unbounded_send(5).unwrap();
                        sleep(200).await;
                        sender.unbounded_send(6).unwrap();
                        sender.disconnect();
                    }));
                }
                sender.disconnect();
                let output = receiver.collect::<Vec<i32>>().await;
                assert_eq!(output, vec![1, 2, 3, 4, 5, 6]);

                runner.dispose();
                loop {
                    if runner.is_disposed.get() {
                        break;
                    }
                    sleep(10).await;
                }
                assert!(true);
            })
            .await;
    }
    #[tokio::test(flavor = "multi_thread")]
    async fn test_3() {
        let local = task::LocalSet::new();
        local
            .run_until(async move {
                let runner = ThrottleJobRunner::new(3);
                let (mut sender, receiver) = mpsc::unbounded::<i32>();

                {
                    let mut sender = sender.clone();
                    runner.add(Box::pin(async move {
                        sender.unbounded_send(1).unwrap();
                        sleep(500).await;
                        sender.unbounded_send(6).unwrap();
                        sender.disconnect();
                    }));
                }
                {
                    let mut sender = sender.clone();
                    runner.add(Box::pin(async move {
                        sender.unbounded_send(2).unwrap();
                        sleep(100).await;
                        sender.unbounded_send(4).unwrap();
                        sender.disconnect();
                    }));
                }
                {
                    let mut sender = sender.clone();
                    runner.add(Box::pin(async move {
                        sender.unbounded_send(3).unwrap();
                        sleep(200).await;
                        sender.unbounded_send(5).unwrap();
                        sender.disconnect();
                    }));
                }
                sender.disconnect();
                let output = receiver.collect::<Vec<i32>>().await;
                assert_eq!(output, vec![1, 2, 3, 4, 5, 6]);

                runner.dispose();
                loop {
                    if runner.is_disposed.get() {
                        break;
                    }
                    sleep(10).await;
                }
                assert!(true);
            })
            .await;
    }
}
