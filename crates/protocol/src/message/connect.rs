use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::{
    Command,
    error::DecodeError,
    wire::{CommandCodec, WireDecode, WireEncode},
};

const VERBOSE_BIT: u8 = 0x01;
const HAS_AUTH_BIT: u8 = 0x02;

const AUTH_TYPE_PASSWORD: u8 = 1;
const AUTH_TYPE_JWT: u8 = 2;

/// Authentication credentials carried in a [`Connect`] message.
pub enum Auth {
    Password { username: Bytes, password: Bytes },
    Jwt { token: Bytes },
}

/// Sent by the client after receiving an INFO message to establish a session.
pub struct Connect {
    pub version: u8,
    /// When `true`, the server sends an `OK` response for each message received.
    pub verbose: bool,
    pub auth: Option<Auth>,
}

impl CommandCodec for Connect {
    fn command() -> Command {
        Command::CONNECT
    }

    fn flags(&self) -> u8 {
        let mut flags = 0u8;
        if self.verbose {
            flags |= VERBOSE_BIT;
        }
        if self.auth.is_some() {
            flags |= HAS_AUTH_BIT;
        }
        flags
    }

    fn encode(&self, dst: &mut BytesMut) {
        dst.put_u8(self.version);

        if let Some(auth) = &self.auth {
            let mut auth_payload = BytesMut::new();
            match auth {
                Auth::Password { username, password } => {
                    dst.put_u8(AUTH_TYPE_PASSWORD);
                    auth_payload.put_length_prefixed_u8(username.as_ref());
                    auth_payload.put_length_prefixed_u8(password.as_ref());
                }
                Auth::Jwt { token } => {
                    dst.put_u8(AUTH_TYPE_JWT);
                    auth_payload.put_length_prefixed_u16(token.as_ref());
                }
            }
            dst.put_varint(auth_payload.len() as u32);
            dst.put(auth_payload);
        }
    }

    fn decode(flags: u8, src: &mut Bytes) -> Result<Self, DecodeError> {
        let verbose = flags & VERBOSE_BIT != 0;
        let has_auth = flags & HAS_AUTH_BIT != 0;

        let version = src.read_u8()?;

        let auth = if has_auth {
            let auth_type = src.read_u8()?;
            let auth_payload_length = src.read_varint()? as usize;
            if src.remaining() < auth_payload_length {
                return Err(DecodeError::BufferTooShort {
                    expected: auth_payload_length,
                    actual: src.remaining(),
                });
            }
            let mut auth_payload = src.copy_to_bytes(auth_payload_length);
            match auth_type {
                AUTH_TYPE_PASSWORD => {
                    let username = auth_payload.read_length_prefixed_u8()?;
                    let password = auth_payload.read_length_prefixed_u8()?;
                    Some(Auth::Password { username, password })
                }
                AUTH_TYPE_JWT => {
                    let token = auth_payload.read_length_prefixed_u16()?;
                    Some(Auth::Jwt { token })
                }
                other => return Err(DecodeError::UnknownAuthType(other)),
            }
        } else {
            None
        };

        Ok(Self { version, verbose, auth })
    }
}

#[cfg(test)]
mod tests {
    use bytes::BytesMut;

    use super::*;

    fn encode_connect(connect: &Connect) -> Bytes {
        let mut buf = BytesMut::new();
        connect.encode(&mut buf);
        buf.freeze()
    }

    #[test]
    fn encode_decode_no_auth_roundtrip() {
        let original = Connect { version: 1, verbose: false, auth: None };
        let mut bytes = encode_connect(&original);
        let decoded = Connect::decode(original.flags(), &mut bytes).unwrap();

        assert_eq!(decoded.version, 1);
        assert!(!decoded.verbose);
        assert!(decoded.auth.is_none());
    }

    #[test]
    fn verbose_flag_set() {
        let connect = Connect { version: 1, verbose: true, auth: None };
        assert_eq!(connect.flags() & VERBOSE_BIT, VERBOSE_BIT);
    }

    #[test]
    fn verbose_flag_unset() {
        let connect = Connect { version: 1, verbose: false, auth: None };
        assert_eq!(connect.flags() & VERBOSE_BIT, 0);
    }

    #[test]
    fn has_auth_flag_set_when_auth_present() {
        let connect = Connect {
            version: 1,
            verbose: false,
            auth: Some(Auth::Password {
                username: Bytes::from_static(b"user"),
                password: Bytes::from_static(b"pass"),
            }),
        };
        assert_eq!(connect.flags() & HAS_AUTH_BIT, HAS_AUTH_BIT);
    }

    #[test]
    fn has_auth_flag_unset_when_no_auth() {
        let connect = Connect { version: 1, verbose: false, auth: None };
        assert_eq!(connect.flags() & HAS_AUTH_BIT, 0);
    }

    #[test]
    fn encode_decode_password_auth_roundtrip() {
        let original = Connect {
            version: 2,
            verbose: true,
            auth: Some(Auth::Password {
                username: Bytes::from_static(b"alice"),
                password: Bytes::from_static(b"s3cr3t"),
            }),
        };
        let mut bytes = encode_connect(&original);
        let decoded = Connect::decode(original.flags(), &mut bytes).unwrap();

        assert_eq!(decoded.version, 2);
        assert!(decoded.verbose);
        let Some(Auth::Password { username, password }) = decoded.auth else {
            panic!("expected password auth");
        };
        assert_eq!(username, Bytes::from_static(b"alice"));
        assert_eq!(password, Bytes::from_static(b"s3cr3t"));
    }

    #[test]
    fn encode_decode_jwt_auth_roundtrip() {
        let token = Bytes::from_static(b"eyJhbGciOiJIUzI1NiJ9.payload.sig");
        let original =
            Connect { version: 1, verbose: false, auth: Some(Auth::Jwt { token: token.clone() }) };
        let mut bytes = encode_connect(&original);
        let decoded = Connect::decode(original.flags(), &mut bytes).unwrap();

        assert_eq!(decoded.version, 1);
        let Some(Auth::Jwt { token: decoded_token }) = decoded.auth else {
            panic!("expected JWT auth");
        };
        assert_eq!(decoded_token, token);
    }
}
