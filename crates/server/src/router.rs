use std::collections::HashMap;

use bytes::Bytes;
use tokio::sync::mpsc::Sender;

use crate::client::ClientId;

#[allow(dead_code)]
pub(crate) struct Subscription {
    pub(crate) subscription_id: u32,
    pub(crate) tx: Sender<Bytes>,
}

#[allow(dead_code)]
pub(crate) struct SubscriptionResponse {
    // HashMap is slower than array
    pub(crate) subscription_list: Vec<(ClientId, Subscription)>,
    pub(crate) queue_group_list: Vec<Vec<(ClientId, Subscription)>>,
}

type SubscriptionMap = HashMap<ClientId, Subscription>;

#[allow(dead_code)]
struct Node {
    level: Bytes,
    subscription_map: SubscriptionMap,
    queue_group_map: HashMap<Bytes, SubscriptionMap>,
    children: Option<Vec<Node>>,
    has_single_wildcard: bool,
    has_multi_wildcard: bool,
    is_leaf: bool,
}

impl Default for Node {
    fn default() -> Self {
        Node {
            level: Bytes::new(),
            subscription_map: SubscriptionMap::new(),
            queue_group_map: HashMap::new(),
            children: None,
            has_single_wildcard: false,
            has_multi_wildcard: false,
            is_leaf: true,
        }
    }
}

#[allow(dead_code)]
pub(crate) struct Router {
    root: Node,
}

#[allow(dead_code)]
impl Router {
    pub(crate) fn new() -> Router {
        Router { root: Node::default() }
    }

    pub(crate) fn insert(
        &mut self,
        _tx: Sender<Bytes>,
        _client_id: ClientId,
        _subscription_id: u32,
        _topic: Bytes,
    ) {
        todo!()
    }

    pub(crate) fn search(&self, _topic: &Bytes) -> SubscriptionResponse {
        todo!()
    }

    pub(crate) fn delete(&mut self, _client_id: ClientId, _subscription_id: u32) {
        todo!()
    }
}

impl Default for Router {
    fn default() -> Self {
        Self::new()
    }
}
