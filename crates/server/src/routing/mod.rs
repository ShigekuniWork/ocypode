use std::{collections::HashSet, sync::Arc};

use bytes::Bytes;
use dashmap::DashMap;

use crate::subscriber::Subscriber;
use crate::topic::Topic;

type Subscribers = DashMap<String, Subscriber>;
type SubscribingTopics = DashMap<String, HashSet<Topic>>;

struct Node {
    #[allow(dead_code)]
    has_wildcard: bool,
    subscribe_map: Subscribers,
    // children are Arc<Node> so we can clone and hold them across iterations without borrowing guards
    children: DashMap<Bytes, Arc<Node>>,
}

impl Node {
    fn new(has_wildcard: bool) -> Self {
        Node { has_wildcard, subscribe_map: Subscribers::new(), children: DashMap::new() }
    }

    fn get_or_create_child(&self, segment: Bytes) -> Arc<Node> {
        self.children.entry(segment).or_insert_with(|| Arc::new(Node::new(false))).value().clone()
    }

    fn get_child(&self, segment: &Bytes) -> Option<Arc<Node>> {
        self.children.get(segment).map(|e| e.value().clone())
    }
}

pub struct Router {
    root: Arc<Node>,
    subscribing_topics: SubscribingTopics,
}

impl Router {
    pub fn new() -> Self {
        Router { root: Arc::new(Node::new(false)), subscribing_topics: SubscribingTopics::new() }
    }

    fn get_or_create_node(&self, topic: &Topic) -> Arc<Node> {
        let mut current = Arc::clone(&self.root);
        for segment in topic.segments() {
            current = current.get_or_create_child(segment);
        }
        current
    }

    fn find_node(&self, topic: &Topic) -> Option<Arc<Node>> {
        let mut current = Arc::clone(&self.root);
        for segment in topic.segments() {
            current = current.get_child(&segment)?;
        }
        Some(current)
    }

    pub fn subscribe(&self, topic: Topic, subscriber: Subscriber) {
        let id = subscriber.id().to_string();

        if let Some(topics_ref) = self.subscribing_topics.get(&id)
            && topics_ref.contains(&topic)
        {
            return;
        }

        let node = self.get_or_create_node(&topic);
        node.subscribe_map.insert(id.clone(), subscriber);
        self.subscribing_topics.entry(id).or_default().insert(topic);
    }

    pub fn un_subscribe(&self, id: &str, topic: &Topic) {
        let mut remove_entry = false;
        if let Some(mut topics_ref) = self.subscribing_topics.get_mut(id) {
            topics_ref.remove(topic);
            remove_entry = topics_ref.is_empty();
        }
        if remove_entry {
            self.subscribing_topics.remove(id);
        }

        if let Some(node) = self.find_node(topic) {
            node.subscribe_map.remove(id);
        }
    }

    pub fn publish(&self, topic: &Topic, payload: Bytes) {
        let Some(node) = self.find_node(topic) else {
            return;
        };

        for entry in node.subscribe_map.iter() {
            entry.value().send(payload.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use tokio::sync::mpsc;

    use super::*;

    fn setup() -> Router {
        Router::new()
    }

    fn make_subscriber(id: &str) -> (Subscriber, mpsc::Receiver<Bytes>) {
        let (tx, rx) = mpsc::channel(16);
        (Subscriber::new(id.to_string(), tx), rx)
    }

    fn topic(s: &'static str) -> Topic {
        Topic::new(Bytes::from_static(s.as_bytes()))
    }

    fn payload(s: &'static str) -> Bytes {
        Bytes::from_static(s.as_bytes())
    }

    #[tokio::test]
    async fn publish_delivers_to_subscriber() {
        let router = setup();
        let t = topic("a/b/c");
        let (sub, mut rx) = make_subscriber("alice");

        router.subscribe(t.clone(), sub);
        router.publish(&t, payload("hello"));

        assert_eq!(rx.recv().await.unwrap(), payload("hello"));
    }

    #[tokio::test]
    async fn publish_to_unknown_topic_does_nothing() {
        let router = setup();
        router.publish(&topic("unknown/topic"), payload("data"));
    }

    #[tokio::test]
    async fn unsubscribe_stops_delivery() {
        let router = setup();
        let t = topic("a/b");
        let (sub, mut rx) = make_subscriber("alice");

        router.subscribe(t.clone(), sub);
        router.un_subscribe("alice", &t);
        router.publish(&t, payload("after unsubscribe"));

        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn publish_delivers_to_all_subscribers() {
        let router = setup();
        let t = topic("a/b");
        let (sub1, mut rx1) = make_subscriber("alice");
        let (sub2, mut rx2) = make_subscriber("bob");

        router.subscribe(t.clone(), sub1);
        router.subscribe(t.clone(), sub2);
        router.publish(&t, payload("broadcast"));

        assert_eq!(rx1.recv().await.unwrap(), payload("broadcast"));
        assert_eq!(rx2.recv().await.unwrap(), payload("broadcast"));
    }

    #[tokio::test]
    async fn duplicate_subscribe_delivers_only_once() {
        let router = setup();
        let t = topic("a/b");
        let (sub1, mut rx) = make_subscriber("alice");
        let (sub2, _) = make_subscriber("alice");

        router.subscribe(t.clone(), sub1);
        router.subscribe(t.clone(), sub2);
        router.publish(&t, payload("once"));

        assert_eq!(rx.recv().await.unwrap(), payload("once"));
        assert!(rx.try_recv().is_err());
    }
}
