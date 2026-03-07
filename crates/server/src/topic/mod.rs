use bytes::Bytes;

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Topic(Bytes);

impl Topic {
    pub fn new(bytes: Bytes) -> Self {
        Topic(bytes)
    }

    pub fn segments(&self) -> impl Iterator<Item = Bytes> + '_ {
        let raw = &self.0;
        let mut start = 0usize;
        let len = raw.len();
        let mut done = false;

        std::iter::from_fn(move || {
            if done {
                return None;
            }
            loop {
                let slash_pos = raw[start..].iter().position(|&b| b == b'/').map(|p| start + p);

                let end = slash_pos.unwrap_or(len);
                let segment = raw.slice(start..end);

                match slash_pos {
                    Some(pos) => start = pos + 1,
                    None => done = true,
                }

                if !segment.is_empty() {
                    return Some(segment);
                }

                if done {
                    return None;
                }
            }
        })
    }
}

impl From<Bytes> for Topic {
    fn from(b: Bytes) -> Self {
        Topic(b)
    }
}

impl From<&'static [u8]> for Topic {
    fn from(b: &'static [u8]) -> Self {
        Topic(Bytes::from_static(b))
    }
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;

    use super::*;

    fn topic(s: &'static str) -> Topic {
        Topic::new(Bytes::from_static(s.as_bytes()))
    }

    #[test]
    fn segments_splits_simple_path() {
        let t = topic("a/b/c");
        let segs: Vec<_> = t.segments().collect();
        assert_eq!(segs, vec![b"a".as_ref(), b"b", b"c"]);
    }

    #[test]
    fn segments_ignores_leading_and_trailing_slashes() {
        let t = topic("/a/b/");
        let segs: Vec<_> = t.segments().collect();
        assert_eq!(segs, vec![b"a".as_ref(), b"b"]);
    }

    #[test]
    fn segments_ignores_consecutive_slashes() {
        let t = topic("a//b");
        let segs: Vec<_> = t.segments().collect();
        assert_eq!(segs, vec![b"a".as_ref(), b"b"]);
    }

    #[test]
    fn segments_single_component() {
        let t = topic("single");
        let segs: Vec<_> = t.segments().collect();
        assert_eq!(segs, vec![b"single".as_ref()]);
    }

    #[test]
    fn segments_empty_topic_yields_nothing() {
        let t = topic("");
        assert_eq!(t.segments().count(), 0);
    }
}
