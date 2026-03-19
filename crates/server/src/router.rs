use std::collections::HashMap;

use bytes::Bytes;
use tokio::sync::mpsc::Sender;

use crate::{
    client::ClientId,
    topic::{Topic, TopicFilter, WILDCARD_MULTI, WILDCARD_SINGLE},
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct SubscriptionKey {
    pub(crate) client_id: ClientId,
    pub(crate) subscription_id: u32,
}

impl SubscriptionKey {
    fn new(client_id: ClientId, subscription_id: u32) -> Self {
        Self { client_id, subscription_id }
    }
}

type SubscriptionMap = HashMap<SubscriptionKey, Sender<Bytes>>;

#[allow(dead_code)]
struct Node {
    level: Bytes,
    subscription_map: SubscriptionMap,
    queue_group_map: HashMap<Bytes, SubscriptionMap>,
    children: Option<Vec<Node>>,
    has_wildcard_single: bool,
    has_wildcard_multi: bool,
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
        node.subscription_map.insert(SubscriptionKey::new(client_id, subscription_id), tx);
    }

    pub(crate) fn search(&self, topic: &Topic) -> SubscriptionResponse {
        let segments: Vec<&[u8]> = topic.segments().collect();
        let mut subscription_list = Vec::new();
        let mut queue_group_list = Vec::new();

        // Stack of (node, remaining_segments).
        let mut stack: Vec<(&Node, &[&[u8]])> = vec![(&self.root, &segments)];

        while let Some((node, remaining)) = stack.pop() {
            // `#` matches zero or more levels, so once a `#` child exists it absorbs
            // all remaining segments. This covers both the multi-level case　and
            // the zero-level case.
            if node.has_wildcard_multi
                && let Some(multi_child) = node
                    .children
                    .as_ref()
                    .and_then(|c| c.iter().find(|n| n.level.as_ref() == WILDCARD_MULTI))
            {
                collect_node(multi_child, &mut subscription_list, &mut queue_group_list);
            }

            let [segment, rest @ ..] = remaining else {
                collect_node(node, &mut subscription_list, &mut queue_group_list);
                continue;
            };

            let Some(children) = &node.children else { continue };

            for child in children {
                if child.level.as_ref() == *segment || child.level.as_ref() == WILDCARD_SINGLE {
                    stack.push((child, rest));
                }
            }
        }

        SubscriptionResponse { subscription_list, queue_group_list }
    }

    pub(crate) fn delete(&mut self, _client_id: ClientId, _subscription_id: u32) {
        todo!()
    }
}

fn collect_node(
    node: &Node,
    subscription_list: &mut Vec<(ClientId, Subscription)>,
    queue_group_list: &mut Vec<Vec<(ClientId, Subscription)>>,
) {
    for (key, tx) in &node.subscription_map {
        subscription_list.push((
            key.client_id,
            Subscription { subscription_id: key.subscription_id, tx: tx.clone() },
        ));
    }
    for group in node.queue_group_map.values() {
        queue_group_list.push(
            group
                .iter()
                .map(|(key, tx)| {
                    (
                        key.client_id,
                        Subscription { subscription_id: key.subscription_id, tx: tx.clone() },
                    )
                })
                .collect(),
        );
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
    async fn insert_leaf_node_contains_subscription() {
        let mut router = Router::new();
        let client_id = ClientId::new();
        router.insert(dummy_tx(), client_id, 7, make_filter("a/b"));
        let leaf = &router.root.children.as_ref().unwrap()[0].children.as_ref().unwrap()[0];
        assert!(leaf.subscription_map.contains_key(&SubscriptionKey::new(client_id, 7)));
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

    fn make_topic(s: &str) -> Topic {
        Topic::new(BytesMut::from(s)).unwrap()
    }

    #[tokio::test]
    async fn search_exact_match_returns_subscriber() {
        let mut router = Router::new();
        let client_id = ClientId::new();
        router.insert(dummy_tx(), client_id, 1, make_filter("a/b"));
        let result = router.search(&make_topic("a/b"));
        assert_eq!(result.subscription_list.len(), 1);
        assert_eq!(result.subscription_list[0].0, client_id);
    }

    #[tokio::test]
    async fn search_no_match_returns_empty() {
        let mut router = Router::new();
        router.insert(dummy_tx(), ClientId::new(), 1, make_filter("a/b"));
        let result = router.search(&make_topic("a/c"));
        assert!(result.subscription_list.is_empty());
    }

    #[tokio::test]
    async fn search_single_wildcard_matches_one_segment() {
        let mut router = Router::new();
        let client_id = ClientId::new();
        router.insert(dummy_tx(), client_id, 1, make_filter("a/+/c"));
        let result = router.search(&make_topic("a/b/c"));
        assert_eq!(result.subscription_list.len(), 1);
        assert_eq!(result.subscription_list[0].0, client_id);
    }

    #[tokio::test]
    async fn search_single_wildcard_does_not_match_wrong_depth() {
        let mut router = Router::new();
        router.insert(dummy_tx(), ClientId::new(), 1, make_filter("a/+/c"));
        let result = router.search(&make_topic("a/c"));
        assert!(result.subscription_list.is_empty());
    }

    #[tokio::test]
    async fn search_multi_wildcard_matches_remaining_segments() {
        let mut router = Router::new();
        let client_id = ClientId::new();
        router.insert(dummy_tx(), client_id, 1, make_filter("a/#"));
        let result = router.search(&make_topic("a/b/c"));
        assert_eq!(result.subscription_list.len(), 1);
        assert_eq!(result.subscription_list[0].0, client_id);
    }

    #[tokio::test]
    async fn search_multi_wildcard_matches_zero_remaining_segments() {
        let mut router = Router::new();
        let client_id = ClientId::new();
        router.insert(dummy_tx(), client_id, 1, make_filter("a/#"));
        let result = router.search(&make_topic("a"));
        assert_eq!(result.subscription_list.len(), 1);
        assert_eq!(result.subscription_list[0].0, client_id);
    }

    #[tokio::test]
    async fn search_root_multi_wildcard_matches_any_topic() {
        let mut router = Router::new();
        let client_id = ClientId::new();
        router.insert(dummy_tx(), client_id, 1, make_filter("#"));
        let result = router.search(&make_topic("a/b/c"));
        assert_eq!(result.subscription_list.len(), 1);
        assert_eq!(result.subscription_list[0].0, client_id);
    }

    #[tokio::test]
    async fn search_returns_all_matching_subscribers() {
        let mut router = Router::new();
        router.insert(dummy_tx(), ClientId::new(), 1, make_filter("a/b"));
        router.insert(dummy_tx(), ClientId::new(), 2, make_filter("a/+"));
        router.insert(dummy_tx(), ClientId::new(), 3, make_filter("a/#"));
        let result = router.search(&make_topic("a/b"));
        assert_eq!(result.subscription_list.len(), 3);
    }

    #[tokio::test]
    async fn search_non_matching_sibling_not_returned() {
        let mut router = Router::new();
        router.insert(dummy_tx(), ClientId::new(), 1, make_filter("x/y"));
        let client_id = ClientId::new();
        router.insert(dummy_tx(), client_id, 2, make_filter("a/b"));
        let result = router.search(&make_topic("a/b"));
        assert_eq!(result.subscription_list.len(), 1);
        assert_eq!(result.subscription_list[0].0, client_id);
    }
}
