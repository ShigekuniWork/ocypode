use bytes::Bytes;
use tokio::sync::mpsc;

#[derive(Clone)]
pub struct Subscriber {
    id: String,
    tx: mpsc::Sender<Bytes>,
}

impl Subscriber {
    pub fn new(id: String, tx: mpsc::Sender<Bytes>) -> Self {
        Subscriber { id, tx }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn send(&self, payload: Bytes) {
        let _ = self.tx.try_send(payload);
    }
}
