use bytes::Bytes;

use crate::{
    Command,
    error::{DecodeError, EncodeError},
    header::FixedHeader,
    message::{
        Message, connect::Connect, info::Info, msg::Msg, publish::Pub, sub::Sub, unsub::Unsub,
    },
    wire::{CommandCodec, wire_frame},
};

/// Sends protocol messages to clients over an established connection.
pub struct ServerCodec;

impl ServerCodec {
    /// Serializes a server→client [`Message`] into a wire frame ready for
    /// transmission.
    pub fn encode(&self, message: &Message) -> Result<Bytes, EncodeError> {
        match message {
            Message::Info(info) => Ok(wire_frame(info)),
            Message::Msg(msg) => Ok(wire_frame(msg)),
            _ => Err(EncodeError::WrongDirection),
        }
    }

    /// Parses a client→server wire frame into a [`Message`].
    pub fn decode(&self, mut src: Bytes) -> Result<Message, DecodeError> {
        let header = FixedHeader::decode(&mut src)?;

        match header.command {
            Command::CONNECT => {
                let connect = Connect::decode(header.flags, &mut src)?;
                Ok(Message::Connect(connect))
            }
            Command::PUB => {
                let message = Pub::decode(header.flags, &mut src)?;
                Ok(Message::Pub(message))
            }
            Command::SUB => {
                let message = Sub::decode(header.flags, &mut src)?;
                Ok(Message::Sub(message))
            }
            Command::UNSUB => {
                let message = Unsub::decode(header.flags, &mut src)?;
                Ok(Message::Unsub(message))
            }
            other => Err(DecodeError::UnsupportedCommand(other)),
        }
    }
}

/// Receives protocol messages from the server over an established connection.
pub struct ClientCodec;

impl ClientCodec {
    /// Parses a server→client wire frame into a [`Message`].
    pub fn decode(&self, mut src: Bytes) -> Result<Message, DecodeError> {
        let header = FixedHeader::decode(&mut src)?;

        match header.command {
            Command::INFO => {
                let info = Info::decode(header.flags, &mut src)?;
                Ok(Message::Info(info))
            }
            Command::MSG => {
                let msg = Msg::decode(header.flags, &mut src)?;
                Ok(Message::Msg(msg))
            }
            other => Err(DecodeError::UnsupportedCommand(other)),
        }
    }

    /// Serializes a client→server [`Message`] into a wire frame ready for
    /// transmission.
    pub fn encode(&self, message: &Message) -> Result<Bytes, EncodeError> {
        match message {
            Message::Connect(connect) => Ok(wire_frame(connect)),
            Message::Pub(message) => Ok(wire_frame(message)),
            Message::Sub(message) => Ok(wire_frame(message)),
            Message::Unsub(message) => Ok(wire_frame(message)),
            _ => Err(EncodeError::WrongDirection),
        }
    }
}

#[cfg(test)]
mod tests {
    use bytes::{Bytes, BytesMut};
    use wire::WireEncode;

    use super::*;
    use crate::message::{connect::Auth, msg::Msg, publish::Pub, sub::Sub, unsub::Unsub};

    fn make_topic(raw: &[u8]) -> topic::Topic {
        let mut buf = BytesMut::new();
        buf.put_length_prefixed_u16(raw);
        topic::Topic::decode(&mut buf.freeze()).unwrap()
    }

    fn make_topic_filter(raw: &[u8]) -> topic::TopicFilter {
        let mut buf = BytesMut::new();
        buf.put_length_prefixed_u16(raw);
        topic::TopicFilter::decode(&mut buf.freeze()).unwrap()
    }

    #[test]
    fn server_encode_client_decode_info_roundtrip() {
        let original = Info {
            version: 1,
            max_payload: 1_048_576,
            server_id: Bytes::from_static(b"srv-1"),
            server_name: Bytes::from_static(b"ocypode"),
            auth_required: true,
            headers: false,
        };

        let wire = ServerCodec.encode(&Message::Info(original)).unwrap();
        let message = ClientCodec.decode(wire).unwrap();

        let Message::Info(decoded) = message else { panic!("expected Info") };
        assert_eq!(decoded.version, 1);
        assert_eq!(decoded.max_payload, 1_048_576);
        assert_eq!(decoded.server_id, Bytes::from_static(b"srv-1"));
        assert_eq!(decoded.server_name, Bytes::from_static(b"ocypode"));
        assert!(decoded.auth_required);
        assert!(!decoded.headers);
    }

    #[test]
    fn client_encode_server_decode_connect_no_auth_roundtrip() {
        let original = Connect { version: 1, verbose: true, auth: None };

        let wire = ClientCodec.encode(&Message::Connect(original)).unwrap();
        let message = ServerCodec.decode(wire).unwrap();

        let Message::Connect(decoded) = message else { panic!("expected Connect") };
        assert_eq!(decoded.version, 1);
        assert!(decoded.verbose);
        assert!(decoded.auth.is_none());
    }

    #[test]
    fn client_encode_server_decode_connect_password_auth_roundtrip() {
        let original = Connect {
            version: 1,
            verbose: false,
            auth: Some(Auth::Password {
                username: Bytes::from_static(b"alice"),
                password: Bytes::from_static(b"hunter2"),
            }),
        };

        let wire = ClientCodec.encode(&Message::Connect(original)).unwrap();
        let message = ServerCodec.decode(wire).unwrap();

        let Message::Connect(decoded) = message else { panic!("expected Connect") };
        let Some(Auth::Password { username, password }) = decoded.auth else {
            panic!("expected password auth");
        };
        assert_eq!(username, Bytes::from_static(b"alice"));
        assert_eq!(password, Bytes::from_static(b"hunter2"));
    }

    #[test]
    fn client_encode_server_decode_connect_jwt_auth_roundtrip() {
        let token = Bytes::from_static(b"header.payload.sig");
        let original =
            Connect { version: 1, verbose: false, auth: Some(Auth::Jwt { token: token.clone() }) };

        let wire = ClientCodec.encode(&Message::Connect(original)).unwrap();
        let message = ServerCodec.decode(wire).unwrap();

        let Message::Connect(decoded) = message else { panic!("expected Connect") };
        let Some(Auth::Jwt { token: decoded_token }) = decoded.auth else {
            panic!("expected JWT auth");
        };
        assert_eq!(decoded_token, token);
    }

    #[test]
    fn server_encode_rejects_connect() {
        let connect = Connect { version: 1, verbose: false, auth: None };
        assert!(matches!(
            ServerCodec.encode(&Message::Connect(connect)),
            Err(EncodeError::WrongDirection)
        ));
    }

    #[test]
    fn client_encode_rejects_info() {
        let info = Info {
            version: 1,
            max_payload: 0,
            server_id: Bytes::new(),
            server_name: Bytes::new(),
            auth_required: false,
            headers: false,
        };
        assert!(matches!(
            ClientCodec.encode(&Message::Info(info)),
            Err(EncodeError::WrongDirection)
        ));
    }

    #[test]
    fn client_encode_server_decode_pub_roundtrip() {
        let original = Pub {
            topic: make_topic(b"events"),
            reply_to: None,
            header: None,
            payload: Bytes::from_static(b"data"),
        };
        let wire = ClientCodec.encode(&Message::Pub(original)).unwrap();
        let message = ServerCodec.decode(wire).unwrap();

        let Message::Pub(decoded) = message else { panic!("expected Pub") };
        assert_eq!(decoded.topic.as_bytes(), &Bytes::from_static(b"events"));
        assert!(decoded.reply_to.is_none());
        assert!(decoded.header.is_none());
        assert_eq!(decoded.payload, Bytes::from_static(b"data"));
    }

    #[test]
    fn client_encode_server_decode_sub_roundtrip() {
        let original = Sub {
            topic: make_topic_filter(b"events/data"),
            subscription_id: Bytes::from_static(b"sub-1"),
            queue_group: None,
        };
        let wire = ClientCodec.encode(&Message::Sub(original)).unwrap();
        let message = ServerCodec.decode(wire).unwrap();

        let Message::Sub(decoded) = message else { panic!("expected Sub") };
        assert_eq!(decoded.topic.as_bytes(), &Bytes::from_static(b"events/data"));
        assert_eq!(decoded.subscription_id, Bytes::from_static(b"sub-1"));
        assert!(decoded.queue_group.is_none());
    }

    #[test]
    fn client_encode_server_decode_unsub_roundtrip() {
        let original = Unsub { subscription_id: Bytes::from_static(b"sub-1") };
        let wire = ClientCodec.encode(&Message::Unsub(original)).unwrap();
        let message = ServerCodec.decode(wire).unwrap();

        let Message::Unsub(decoded) = message else { panic!("expected Unsub") };
        assert_eq!(decoded.subscription_id, Bytes::from_static(b"sub-1"));
    }

    #[test]
    fn server_encode_client_decode_msg_roundtrip() {
        let original = Msg {
            topic: make_topic(b"events/data"),
            subscription_id: Bytes::from_static(b"sub-1"),
            reply_to: None,
            header: None,
            payload: Bytes::from_static(b"payload"),
        };
        let wire = ServerCodec.encode(&Message::Msg(original)).unwrap();
        let message = ClientCodec.decode(wire).unwrap();

        let Message::Msg(decoded) = message else { panic!("expected Msg") };
        assert_eq!(decoded.topic.as_bytes(), &Bytes::from_static(b"events/data"));
        assert_eq!(decoded.subscription_id, Bytes::from_static(b"sub-1"));
        assert!(decoded.reply_to.is_none());
        assert!(decoded.header.is_none());
        assert_eq!(decoded.payload, Bytes::from_static(b"payload"));
    }
}
