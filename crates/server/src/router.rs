// TODO: This module routes inbound PUB messages to matching subscribers.
//       A trie-based subject matching algorithm (sublist pattern) is planned for O(depth) fan-out.
//       Subscribers register their outbound channel sender so the router can deliver directly.

/// Routes published messages to all matching subscribers.
// TODO: Implement with a sublist trie for efficient wildcard subject matching.
#[allow(dead_code)]
pub trait Router: Send + Sync + 'static {
    // TODO: fn publish(&self, subject: &str, payload: bytes::Bytes, sender_client_id: u64);
    // TODO: fn subscribe(&self, subject: &str, client_id: u64, sender: tokio::sync::mpsc::Sender<crate::parser::OutboundMessage>);
    // TODO: fn unsubscribe(&self, subject: &str, client_id: u64);
}
