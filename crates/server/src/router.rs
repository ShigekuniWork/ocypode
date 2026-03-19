use std::collections::HashMap;

use bytes::Bytes;
use tokio::sync::mpsc::Sender;

use crate::{
    client::ClientId,
    topic::{TopicFilter, WILDCARD_MULTI, WILDCARD_SINGLE},
};

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
    has_wildcard_single: bool,
    has_wildcard_multi: bool,
    is_leaf: bool,
}

impl Default for Node {
    fn default() -> Self {
        Node {
            level: Bytes::new(),
            subscription_map: SubscriptionMap::new(),
            queue_group_map: HashMap::new(),
            children: None,
            has_wildcard_single: false,
            has_wildcard_multi: false,
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
        tx: Sender<Bytes>,
        client_id: ClientId,
        subscription_id: u32,
        topic: TopicFilter,
    ) {
        let mut node = &mut self.root;
        for segment in topic.segments() {
            // Wildcard flags on the parent are used during search to identify which
            // branches to explore when delivering messages to matching subscribers.
            if segment == WILDCARD_SINGLE {
                node.has_wildcard_single = true;
            } else if segment == WILDCARD_MULTI {
                node.has_wildcard_multi = true;
            }
            node.is_leaf = false;
            let children = node.children.get_or_insert_with(Vec::new);
            let child_idx = match children.iter().position(|n| n.level == segment) {
                Some(pos) => pos,
                None => {
                    children
                        .push(Node { level: Bytes::copy_from_slice(segment), ..Node::default() });
                    children.len() - 1
                }
            };
            node = &mut children[child_idx];
        }
        node.subscription_map.insert(client_id, Subscription { subscription_id, tx });
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

#[cfg(test)]
mod tests {
    use bytes::BytesMut;
    use tokio::sync::mpsc::Sender;

    use super::*;
    use crate::client::ClientId;

    fn make_filter(s: &str) -> TopicFilter {
        TopicFilter::new(BytesMut::from(s)).unwrap()
    }

    fn dummy_tx() -> Sender<Bytes> {
        tokio::sync::mpsc::channel(1).0
    }

    #[tokio::test]
    async fn insert_single_segment_creates_child() {
        let mut router = Router::new();
        router.insert(dummy_tx(), ClientId::new(), 1, make_filter("a"));
        assert_eq!(router.root.children.as_ref().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn insert_multi_segment_creates_nested_children() {
        let mut router = Router::new();
        router.insert(dummy_tx(), ClientId::new(), 1, make_filter("a/b/c"));
        let level1 = &router.root.children.as_ref().unwrap()[0];
        let level2 = &level1.children.as_ref().unwrap()[0];
        let level3 = &level2.children.as_ref().unwrap()[0];
        assert_eq!(level1.level.as_ref(), b"a");
        assert_eq!(level2.level.as_ref(), b"b");
        assert_eq!(level3.level.as_ref(), b"c");
    }

    #[tokio::test]
    async fn insert_marks_intermediate_nodes_as_non_leaf() {
        let mut router = Router::new();
        router.insert(dummy_tx(), ClientId::new(), 1, make_filter("a/b/c"));
        let level1 = &router.root.children.as_ref().unwrap()[0];
        let level2 = &level1.children.as_ref().unwrap()[0];
        assert!(!level1.is_leaf);
        assert!(!level2.is_leaf);
    }

    #[tokio::test]
    async fn insert_leaf_node_contains_subscription() {
        let mut router = Router::new();
        let client_id = ClientId::new();
        router.insert(dummy_tx(), client_id, 7, make_filter("a/b"));
        let leaf = &router.root.children.as_ref().unwrap()[0].children.as_ref().unwrap()[0];
        assert!(leaf.subscription_map.contains_key(&client_id));
        assert_eq!(leaf.subscription_map[&client_id].subscription_id, 7);
    }

    #[tokio::test]
    async fn insert_wildcard_single_wildcard_sets_flag_on_parent() {
        let mut router = Router::new();
        router.insert(dummy_tx(), ClientId::new(), 1, make_filter("a/+/c"));
        let level1 = &router.root.children.as_ref().unwrap()[0];
        assert!(level1.has_wildcard_single);
    }

    #[tokio::test]
    async fn insert_wildcard_multi_sets_flag_on_parent() {
        let mut router = Router::new();
        router.insert(dummy_tx(), ClientId::new(), 1, make_filter("a/#"));
        let level1 = &router.root.children.as_ref().unwrap()[0];
        assert!(level1.has_wildcard_multi);
    }

    #[tokio::test]
    async fn insert_two_subscribers_same_topic() {
        let mut router = Router::new();
        router.insert(dummy_tx(), ClientId::new(), 1, make_filter("a/b"));
        router.insert(dummy_tx(), ClientId::new(), 2, make_filter("a/b"));
        let leaf = &router.root.children.as_ref().unwrap()[0].children.as_ref().unwrap()[0];
        assert_eq!(leaf.subscription_map.len(), 2);
    }

    #[tokio::test]
    async fn insert_shares_common_prefix_nodes() {
        let mut router = Router::new();
        router.insert(dummy_tx(), ClientId::new(), 1, make_filter("a/b/c"));
        router.insert(dummy_tx(), ClientId::new(), 2, make_filter("a/b/d"));
        let level1 = &router.root.children.as_ref().unwrap()[0];
        let level2 = &level1.children.as_ref().unwrap()[0];
        assert_eq!(router.root.children.as_ref().unwrap().len(), 1);
        assert_eq!(level1.children.as_ref().unwrap().len(), 1);
        assert_eq!(level2.children.as_ref().unwrap().len(), 2);
    }
}
