use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::broadcast;

pub struct Stop {
    shutdown: Arc<AtomicBool>,
    sender: broadcast::Sender<()>,
    receiver: broadcast::Receiver<()>,
}

impl Stop {
    pub fn new() -> Self {
        let (sender, receiver) = broadcast::channel(1);
        Self {
            shutdown: Arc::new(AtomicBool::new(false)),
            sender,
            receiver,
        }
    }

    pub fn trigger(&self) {
        let _ = self.sender.send(());
    }

    pub async fn recv(&mut self) {
        if self.shutdown.load(Ordering::SeqCst) {
            return;
        }

        let _ = self.receiver.recv().await;
        self.shutdown.store(true, Ordering::SeqCst);
    }

    pub fn is_stop(&self) -> bool {
        self.shutdown.load(Ordering::SeqCst)
    }
}

impl Clone for Stop {
    fn clone(&self) -> Self {
        let sender = self.sender.clone();
        let receiver = self.sender.subscribe();
        Self {
            shutdown: self.shutdown.clone(),
            sender,
            receiver,
        }
    }
}
