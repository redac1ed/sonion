use crate::net::{FetchError, FetchedPage, NetStack, SharedNet};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};

pub enum NetCommand {
    Fetch {
        url: String, 
        reply: oneshot::Sender<Result<FetchedPage, Arc<FetchError>>>
    },
    Shutdown
}

#[derive(Clone)]
pub struct NetHandle {
    tx: mpsc::UnboundedSender<NetCommand>
}

impl NetHandle {
    pub fn spawn(stack: NetStack) -> Self {
        let (tx, mut rx) = mpsc::unbounded_channel::<NetCommand>();
        let stack: SharedNet = Arc::new(stack);
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().expect("tokio runtime");
            rt.block_on(async move {
                while let Some(cmd) = rx.recv().await {
                    match cmd {
                        NetCommand::Fetch {url, reply} => {
                            let stack = stack.clone();
                            tokio::spawn(async move {
                                let result = stack.fetch_page(&url).await.map_err(Arc::new);
                                let _ = reply.send(result);
                            });
                        }
                        NetCommand::Shutdown => break
                    }
                }
            });
        });
        Self {tx}
    }
    pub fn fetch_blocking(&self, url: &str) -> Result<FetchedPage, Arc<FetchError>> {
        let (tx, rx) = oneshot::channel();
        if self.tx
            .send(NetCommand::Fetch {
                url: url.to_string(), 
                reply: tx
            }).is_err() {
                return Err(Arc::new(FetchError::BadUrl("net worker gone".into())));
        }
        match rx.blocking_recv() {
            Ok(r) => r,
            Err(_) => Err(Arc::new(FetchError::BadUrl("net worker dropped".into())))
        }
    }
    pub fn shutdown(&self) {
        let _ = self.tx.send(NetCommand::Shutdown);
    }
}