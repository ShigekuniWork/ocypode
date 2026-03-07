// TODO: Temporary: client codec decode variants are present to avoid dead_code warnings.
// This helps keep the binary crate free of spurious warnings while the client-side
// integration is still in progress. Remove or reconcile these once a full client
// implementation is wired in.
#![allow(dead_code)]
// TODO: Temporary crate-level allow(dead_code).
// Remove this attribute once the client implementation is wired and
// the currently-unused client codec items are actually referenced.
// This suppresses dead_code warnings during incremental development.
use bytes::{Buf, BufMut, Bytes, BytesMut};
use thiserror::Error;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const COMMAND_SHIFT: u8 = 4;
const COMMAND_MASK: u8 = 0xF0;
const FLAGS_MASK: u8 = 0x0F;

const VARINT_CONTINUATION_BIT: u8 = 0x80;
const VARINT_PAYLOAD_MASK: u8 = 0x7F;
const VARINT_PAYLOAD_BITS: u32 = 7;
const VARINT_MAX_BYTES: usize = 4;
const VARINT_MAX_VALUE: u32 = 0x0FFF_FFFF;

const PROTOCOL_VERSION: u8 = 0;

const DEFAULT_MAX_PAYLOAD: u32 = 1_048_576;
const MAX_TOPIC_LENGTH: u16 = 256;

// ---------------------------------------------------------------------------
// Command
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Command {
    Info = 0x1,
    Connect = 0x2,
    Pub = 0x3,
    Sub = 0x4,
    Unsub = 0x5,
    Msg = 0x6,
    Ping = 0x7,
    Pong = 0x8,
    Ok = 0x9,
    Err = 0xA,
}

impl Command {
    fn from_u8(value: u8) -> Result<Self, DecodeError> {
        match value {
            0x1 => Ok(Self::Info),
            0x2 => Ok(Self::Connect),
            0x3 => Ok(Self::Pub),
            0x4 => Ok(Self::Sub),
            0x5 => Ok(Self::Unsub),
            0x6 => Ok(Self::Msg),
            0x7 => Ok(Self::Ping),
            0x8 => Ok(Self::Pong),
            0x9 => Ok(Self::Ok),
            0xA => Ok(Self::Err),
            0x0 => Err(DecodeError::ReservedCommand),
            _ => Err(DecodeError::UnknownCommand(value)),
        }
    }
}

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum ErrorCode {
    UnknownProtocol = 0x0001,
    AuthFailed = 0x0002,
    AuthRequired = 0x0003,
    InvalidTopic = 0x0004,
    PayloadTooLarge = 0x0005,
    ParseError = 0x0006,
    UnknownSubscription = 0x0007,
}

impl ErrorCode {
    fn from_u16(value: u16) -> Result<Self, DecodeError> {
        match value {
            0x0001 => Ok(Self::UnknownProtocol),
            0x0002 => Ok(Self::AuthFailed),
            0x0003 => Ok(Self::AuthRequired),
            0x0004 => Ok(Self::InvalidTopic),
            0x0005 => Ok(Self::PayloadTooLarge),
            0x0006 => Ok(Self::ParseError),
            0x0007 => Ok(Self::UnknownSubscription),
            _ => Err(DecodeError::UnknownErrorCode(value)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DecodeError {
    #[error("incomplete message")]
    Incomplete,
    #[error("reserved command 0x0")]
    ReservedCommand,
    #[error("unknown command 0x{0:X}")]
    UnknownCommand(u8),
    #[error("unknown error code 0x{0:04X}")]
    UnknownErrorCode(u16),
    #[error("varint exceeds maximum length")]
    VarintTooLong,
    #[error("varint value overflow")]
    VarintOverflow,
    #[error("invalid UTF-8 string")]
    InvalidUtf8,
    #[error("topic length {0} exceeds maximum")]
    TopicTooLong(u16),
    #[error("unexpected trailing bytes")]
    TrailingBytes,
    #[error("invalid flags 0x{flags:X} for {command:?}")]
    InvalidFlags { command: Command, flags: u8 },
}

// ---------------------------------------------------------------------------
// Flags
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ConnectFlags {
    pub verbose: bool,
    pub has_auth: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CapabilityFlags {
    pub auth_required: bool,
    pub headers: bool,
}

// ---------------------------------------------------------------------------
// Auth
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Auth {
    Password { username: String, password: String },
    Jwt { token: String },
}

const AUTH_TYPE_PASSWORD: u8 = 1;
const AUTH_TYPE_JWT: u8 = 2;

// ---------------------------------------------------------------------------
// Message
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    Info {
        version: u8,
        max_payload: u32,
        server_id: String,
        server_name: String,
        capability_flags: CapabilityFlags,
    },
    Connect {
        version: u8,
        verbose: bool,
        auth: Option<Auth>,
    },
    Pub {
        // TODO: replace with a dedicated type once the topic format is finalised
        topic: BytesMut,
        header: Option<Bytes>,
        payload: Bytes,
    },
    Sub {
        // TODO: replace with a dedicated type once the topic format is finalised
        topic: BytesMut,
        subscription_id: Bytes,
        queue_group: Option<String>,
    },
    Unsub {
        subscription_id: Bytes,
    },
    Msg {
        // TODO: replace with a dedicated type once the topic format is finalised
        topic: BytesMut,
        subscription_id: Bytes,
        header: Option<Bytes>,
        payload: Bytes,
    },
    Ping,
    Pong,
    Ok,
    Err {
        error_code: ErrorCode,
    },
}

// ---------------------------------------------------------------------------
// Varint helpers
// ---------------------------------------------------------------------------

fn encode_varint(value: u32, buf: &mut BytesMut) {
    let mut v = value;
    loop {
        let mut byte = (v as u8) & VARINT_PAYLOAD_MASK;
        v >>= VARINT_PAYLOAD_BITS;
        if v > 0 {
            byte |= VARINT_CONTINUATION_BIT;
        }
        buf.put_u8(byte);
        if v == 0 {
            break;
        }
    }
}

fn decode_varint(buf: &mut BytesMut) -> Result<u32, DecodeError> {
    let mut value: u32 = 0;
    let mut shift: u32 = 0;

    for i in 0..VARINT_MAX_BYTES {
        if !buf.has_remaining() {
            return Err(DecodeError::Incomplete);
        }
        let byte = buf.get_u8();
        let payload = (byte & VARINT_PAYLOAD_MASK) as u32;

        if shift + VARINT_PAYLOAD_BITS > 32 && payload > (u32::MAX >> shift) {
            return Err(DecodeError::VarintOverflow);
        }

        value |= payload << shift;
        shift += VARINT_PAYLOAD_BITS;

        if byte & VARINT_CONTINUATION_BIT == 0 {
            return Ok(value);
        }

        if i == VARINT_MAX_BYTES - 1 {
            return Err(DecodeError::VarintTooLong);
        }
    }

    Err(DecodeError::VarintTooLong)
}

fn varint_encoded_size(value: u32) -> usize {
    match value {
        0..=127 => 1,
        128..=16_383 => 2,
        16_384..=2_097_151 => 3,
        _ => 4,
    }
}

// ---------------------------------------------------------------------------
// Primitive read/write helpers
// ---------------------------------------------------------------------------

fn require_remaining(buf: &BytesMut, needed: usize) -> Result<(), DecodeError> {
    if buf.remaining() < needed { Err(DecodeError::Incomplete) } else { Ok(()) }
}

fn read_u8(buf: &mut BytesMut) -> Result<u8, DecodeError> {
    require_remaining(buf, 1)?;
    Ok(buf.get_u8())
}

fn read_u16_le(buf: &mut BytesMut) -> Result<u16, DecodeError> {
    require_remaining(buf, 2)?;
    Ok(buf.get_u16_le())
}

fn read_u32_le(buf: &mut BytesMut) -> Result<u32, DecodeError> {
    require_remaining(buf, 4)?;
    Ok(buf.get_u32_le())
}

fn read_bytes(buf: &mut BytesMut, len: usize) -> Result<BytesMut, DecodeError> {
    require_remaining(buf, len)?;
    Ok(buf.split_to(len))
}

fn read_string(buf: &mut BytesMut, len: usize) -> Result<String, DecodeError> {
    let bytes = read_bytes(buf, len)?;
    String::from_utf8(bytes.to_vec()).map_err(|_| DecodeError::InvalidUtf8)
}

fn read_length_prefixed_string_u8(buf: &mut BytesMut) -> Result<String, DecodeError> {
    let len = read_u8(buf)? as usize;
    read_string(buf, len)
}

// TODO: replace with a dedicated type once the topic format is finalised
fn read_topic(buf: &mut BytesMut) -> Result<BytesMut, DecodeError> {
    let topic_length = read_u16_le(buf)?;
    if topic_length > MAX_TOPIC_LENGTH {
        return Err(DecodeError::TopicTooLong(topic_length));
    }
    read_bytes(buf, topic_length as usize)
}

fn read_length_prefixed_bytes_u16_le(buf: &mut BytesMut) -> Result<Bytes, DecodeError> {
    let len = read_u16_le(buf)? as usize;
    Ok(read_bytes(buf, len)?.freeze())
}

fn read_varint_prefixed_bytes(buf: &mut BytesMut) -> Result<Bytes, DecodeError> {
    let len = decode_varint(buf)? as usize;
    Ok(read_bytes(buf, len)?.freeze())
}

// TODO: replace with a dedicated type once the topic format is finalised
fn write_topic(buf: &mut BytesMut, topic: &[u8]) {
    buf.put_u16_le(topic.len() as u16);
    buf.put_slice(topic);
}

fn write_length_prefixed_bytes_u16_le(buf: &mut BytesMut, data: &[u8]) {
    buf.put_u16_le(data.len() as u16);
    buf.put_slice(data);
}

fn write_varint_prefixed_bytes(buf: &mut BytesMut, data: &[u8]) {
    encode_varint(data.len() as u32, buf);
    buf.put_slice(data);
}

fn write_length_prefixed_string_u8(buf: &mut BytesMut, s: &str) {
    buf.put_u8(s.len() as u8);
    buf.put_slice(s.as_bytes());
}

// ---------------------------------------------------------------------------
// Fixed header encoding/decoding
// ---------------------------------------------------------------------------

fn encode_fixed_header(command: Command, flags: u8, remaining_length: u32) -> BytesMut {
    let mut buf = BytesMut::with_capacity(1 + varint_encoded_size(remaining_length));
    let first_byte = ((command as u8) << COMMAND_SHIFT) | (flags & FLAGS_MASK);
    buf.put_u8(first_byte);
    encode_varint(remaining_length, &mut buf);
    buf
}

struct FixedHeader {
    command: Command,
    flags: u8,
    remaining_length: u32,
}

fn decode_fixed_header(buf: &mut BytesMut) -> Result<FixedHeader, DecodeError> {
    let first_byte = read_u8(buf)?;
    let command_nibble = (first_byte & COMMAND_MASK) >> COMMAND_SHIFT;
    let flags = first_byte & FLAGS_MASK;
    let command = Command::from_u8(command_nibble)?;
    let remaining_length = decode_varint(buf)?;
    Ok(FixedHeader { command, flags, remaining_length })
}

// ---------------------------------------------------------------------------
// Flags bit constants
// ---------------------------------------------------------------------------

const CONNECT_FLAG_VERBOSE: u8 = 0b0001;
const CONNECT_FLAG_HAS_AUTH: u8 = 0b0010;
const CONNECT_RESERVED_MASK: u8 = 0b1100;

const PUB_FLAG_HAS_HEADER: u8 = 0b0001;
const PUB_RESERVED_MASK: u8 = 0b1110;

const SUB_FLAG_HAS_QUEUE_GROUP: u8 = 0b0001;
const SUB_RESERVED_MASK: u8 = 0b1110;

const MSG_FLAG_HAS_HEADER: u8 = 0b0001;
const MSG_RESERVED_MASK: u8 = 0b1110;

const NO_FLAGS: u8 = 0;

const CAPABILITY_FLAG_AUTH_REQUIRED: u8 = 0b0000_0001;
const CAPABILITY_FLAG_HEADERS: u8 = 0b0000_0010;

// ---------------------------------------------------------------------------
// Encoding
// ---------------------------------------------------------------------------

impl Message {
    pub fn encode(&self) -> BytesMut {
        match self {
            Self::Info { version, max_payload, server_id, server_name, capability_flags } => {
                let mut body = BytesMut::new();
                body.put_u8(*version);
                body.put_u32_le(*max_payload);
                write_length_prefixed_string_u8(&mut body, server_id);
                write_length_prefixed_string_u8(&mut body, server_name);

                let mut cap: u8 = 0;
                if capability_flags.auth_required {
                    cap |= CAPABILITY_FLAG_AUTH_REQUIRED;
                }
                if capability_flags.headers {
                    cap |= CAPABILITY_FLAG_HEADERS;
                }
                body.put_u8(cap);

                let mut out = encode_fixed_header(Command::Info, NO_FLAGS, body.len() as u32);
                out.unsplit(body);
                out
            }

            Self::Connect { version, verbose, auth } => {
                let mut flags: u8 = 0;
                if *verbose {
                    flags |= CONNECT_FLAG_VERBOSE;
                }
                if auth.is_some() {
                    flags |= CONNECT_FLAG_HAS_AUTH;
                }

                let mut body = BytesMut::new();
                body.put_u8(*version);

                if let Some(auth) = auth {
                    match auth {
                        Auth::Password { username, password } => {
                            body.put_u8(AUTH_TYPE_PASSWORD);
                            let auth_len = 1 + username.len() + 1 + password.len();
                            encode_varint(auth_len as u32, &mut body);
                            write_length_prefixed_string_u8(&mut body, username);
                            write_length_prefixed_string_u8(&mut body, password);
                        }
                        Auth::Jwt { token } => {
                            body.put_u8(AUTH_TYPE_JWT);
                            let auth_len = 2 + token.len();
                            encode_varint(auth_len as u32, &mut body);
                            body.put_u16_le(token.len() as u16);
                            body.put_slice(token.as_bytes());
                        }
                    }
                }

                let mut out = encode_fixed_header(Command::Connect, flags, body.len() as u32);
                out.unsplit(body);
                out
            }

            Self::Pub { topic, header, payload } => {
                let mut flags: u8 = 0;
                if header.is_some() {
                    flags |= PUB_FLAG_HAS_HEADER;
                }

                let mut body = BytesMut::new();
                write_topic(&mut body, topic);
                if let Some(hdr) = header {
                    write_length_prefixed_bytes_u16_le(&mut body, hdr);
                }
                write_varint_prefixed_bytes(&mut body, payload);

                let mut out = encode_fixed_header(Command::Pub, flags, body.len() as u32);
                out.unsplit(body);
                out
            }

            Self::Sub { topic, subscription_id, queue_group } => {
                let mut flags: u8 = 0;
                if queue_group.is_some() {
                    flags |= SUB_FLAG_HAS_QUEUE_GROUP;
                }

                let mut body = BytesMut::new();
                write_topic(&mut body, topic);
                write_length_prefixed_bytes_u16_le(&mut body, subscription_id);
                if let Some(qg) = queue_group {
                    write_length_prefixed_string_u8(&mut body, qg);
                }

                let mut out = encode_fixed_header(Command::Sub, flags, body.len() as u32);
                out.unsplit(body);
                out
            }

            Self::Unsub { subscription_id } => {
                let mut body = BytesMut::new();
                write_length_prefixed_bytes_u16_le(&mut body, subscription_id);

                let mut out = encode_fixed_header(Command::Unsub, NO_FLAGS, body.len() as u32);
                out.unsplit(body);
                out
            }

            Self::Msg { topic, subscription_id, header, payload } => {
                let mut flags: u8 = 0;
                if header.is_some() {
                    flags |= MSG_FLAG_HAS_HEADER;
                }

                let mut body = BytesMut::new();
                write_topic(&mut body, topic);
                write_length_prefixed_bytes_u16_le(&mut body, subscription_id);
                if let Some(hdr) = header {
                    write_length_prefixed_bytes_u16_le(&mut body, hdr);
                }
                write_varint_prefixed_bytes(&mut body, payload);

                let mut out = encode_fixed_header(Command::Msg, flags, body.len() as u32);
                out.unsplit(body);
                out
            }

            Self::Ping => encode_fixed_header(Command::Ping, NO_FLAGS, 0),
            Self::Pong => encode_fixed_header(Command::Pong, NO_FLAGS, 0),
            Self::Ok => encode_fixed_header(Command::Ok, NO_FLAGS, 0),

            Self::Err { error_code } => {
                let mut body = BytesMut::with_capacity(2);
                body.put_u16_le(*error_code as u16);

                let mut out = encode_fixed_header(Command::Err, NO_FLAGS, body.len() as u32);
                out.unsplit(body);
                out
            }
        }
    }

    pub fn decode(buf: &mut BytesMut) -> Result<Self, DecodeError> {
        let header = decode_fixed_header(buf)?;
        let mut body = read_bytes(buf, header.remaining_length as usize)?;

        let message = match header.command {
            Command::Info => {
                validate_no_flags(header.command, header.flags)?;
                let version = read_u8(&mut body)?;
                let max_payload = read_u32_le(&mut body)?;
                let server_id = read_length_prefixed_string_u8(&mut body)?;
                let server_name = read_length_prefixed_string_u8(&mut body)?;
                let cap_byte = read_u8(&mut body)?;
                let capability_flags = CapabilityFlags {
                    auth_required: cap_byte & CAPABILITY_FLAG_AUTH_REQUIRED != 0,
                    headers: cap_byte & CAPABILITY_FLAG_HEADERS != 0,
                };
                Self::Info { version, max_payload, server_id, server_name, capability_flags }
            }

            Command::Connect => {
                validate_reserved_flags(header.command, header.flags, CONNECT_RESERVED_MASK)?;
                let flags = ConnectFlags {
                    verbose: header.flags & CONNECT_FLAG_VERBOSE != 0,
                    has_auth: header.flags & CONNECT_FLAG_HAS_AUTH != 0,
                };

                let version = read_u8(&mut body)?;
                let auth = if flags.has_auth {
                    let auth_type = read_u8(&mut body)?;
                    let auth_payload = read_varint_prefixed_bytes(&mut body)?;
                    let mut auth_buf = BytesMut::from(auth_payload.as_ref());
                    Some(decode_auth(auth_type, &mut auth_buf)?)
                } else {
                    None
                };

                Self::Connect { version, verbose: flags.verbose, auth }
            }

            Command::Pub => {
                validate_reserved_flags(header.command, header.flags, PUB_RESERVED_MASK)?;
                let has_header = header.flags & PUB_FLAG_HAS_HEADER != 0;

                let topic = read_topic(&mut body)?;
                let hdr = if has_header {
                    Some(read_length_prefixed_bytes_u16_le(&mut body)?)
                } else {
                    None
                };
                let payload = read_varint_prefixed_bytes(&mut body)?;

                Self::Pub { topic, header: hdr, payload }
            }

            Command::Sub => {
                validate_reserved_flags(header.command, header.flags, SUB_RESERVED_MASK)?;
                let has_queue_group = header.flags & SUB_FLAG_HAS_QUEUE_GROUP != 0;

                let topic = read_topic(&mut body)?;
                let subscription_id = read_length_prefixed_bytes_u16_le(&mut body)?;
                let queue_group = if has_queue_group {
                    Some(read_length_prefixed_string_u8(&mut body)?)
                } else {
                    None
                };

                Self::Sub { topic, subscription_id, queue_group }
            }

            Command::Unsub => {
                validate_no_flags(header.command, header.flags)?;
                let subscription_id = read_length_prefixed_bytes_u16_le(&mut body)?;
                Self::Unsub { subscription_id }
            }

            Command::Msg => {
                validate_reserved_flags(header.command, header.flags, MSG_RESERVED_MASK)?;
                let has_header = header.flags & MSG_FLAG_HAS_HEADER != 0;

                let topic = read_topic(&mut body)?;
                let subscription_id = read_length_prefixed_bytes_u16_le(&mut body)?;
                let hdr = if has_header {
                    Some(read_length_prefixed_bytes_u16_le(&mut body)?)
                } else {
                    None
                };
                let payload = read_varint_prefixed_bytes(&mut body)?;

                Self::Msg { topic, subscription_id, header: hdr, payload }
            }

            Command::Ping => {
                validate_no_flags(header.command, header.flags)?;
                Self::Ping
            }
            Command::Pong => {
                validate_no_flags(header.command, header.flags)?;
                Self::Pong
            }
            Command::Ok => {
                validate_no_flags(header.command, header.flags)?;
                Self::Ok
            }
            Command::Err => {
                validate_no_flags(header.command, header.flags)?;
                let code = read_u16_le(&mut body)?;
                Self::Err { error_code: ErrorCode::from_u16(code)? }
            }
        };

        if body.has_remaining() {
            return Err(DecodeError::TrailingBytes);
        }

        Ok(message)
    }
}

// ---------------------------------------------------------------------------
// Validation helpers
// ---------------------------------------------------------------------------

fn validate_no_flags(command: Command, flags: u8) -> Result<(), DecodeError> {
    if flags != 0 { Err(DecodeError::InvalidFlags { command, flags }) } else { Ok(()) }
}

fn validate_reserved_flags(
    command: Command,
    flags: u8,
    reserved_mask: u8,
) -> Result<(), DecodeError> {
    if flags & reserved_mask != 0 {
        Err(DecodeError::InvalidFlags { command, flags })
    } else {
        Ok(())
    }
}

impl Message {
    fn command_code(&self) -> u8 {
        match self {
            Self::Info { .. } => Command::Info as u8,
            Self::Connect { .. } => Command::Connect as u8,
            Self::Pub { .. } => Command::Pub as u8,
            Self::Sub { .. } => Command::Sub as u8,
            Self::Unsub { .. } => Command::Unsub as u8,
            Self::Msg { .. } => Command::Msg as u8,
            Self::Ping => Command::Ping as u8,
            Self::Pong => Command::Pong as u8,
            Self::Ok => Command::Ok as u8,
            Self::Err { .. } => Command::Err as u8,
        }
    }
}

fn decode_auth(auth_type: u8, buf: &mut BytesMut) -> Result<Auth, DecodeError> {
    match auth_type {
        AUTH_TYPE_PASSWORD => {
            let username = read_length_prefixed_string_u8(buf)?;
            let password = read_length_prefixed_string_u8(buf)?;
            Ok(Auth::Password { username, password })
        }
        AUTH_TYPE_JWT => {
            let token_length = read_u16_le(buf)? as usize;
            let token = read_string(buf, token_length)?;
            Ok(Auth::Jwt { token })
        }
        _ => Err(DecodeError::UnknownCommand(auth_type)),
    }
}

// ---------------------------------------------------------------------------
// Codec
// ---------------------------------------------------------------------------

/// Messages the server is allowed to receive from a client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerInbound {
    Connect { version: u8, verbose: bool, auth: Option<Auth> },
    Pub { topic: BytesMut, header: Option<Bytes>, payload: Bytes },
    Sub { topic: BytesMut, subscription_id: Bytes, queue_group: Option<String> },
    Unsub { subscription_id: Bytes },
    Ping,
    Pong,
}

/// Messages the server is allowed to send to a client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerOutbound {
    Info {
        version: u8,
        max_payload: u32,
        server_id: String,
        server_name: String,
        capability_flags: CapabilityFlags,
    },
    Msg {
        topic: BytesMut,
        subscription_id: Bytes,
        header: Option<Bytes>,
        payload: Bytes,
    },
    Ping,
    Pong,
    Ok,
    Err {
        error_code: ErrorCode,
    },
}

/// Messages the client is allowed to send to the server.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientOutbound {
    Connect { version: u8, verbose: bool, auth: Option<Auth> },
    Pub { topic: BytesMut, header: Option<Bytes>, payload: Bytes },
    Sub { topic: BytesMut, subscription_id: Bytes, queue_group: Option<String> },
    Unsub { subscription_id: Bytes },
    Ping,
    Pong,
}

/// Messages the client is allowed to receive from the server.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientInbound {
    Info {
        version: u8,
        max_payload: u32,
        server_id: String,
        server_name: String,
        capability_flags: CapabilityFlags,
    },
    Msg {
        topic: BytesMut,
        subscription_id: Bytes,
        header: Option<Bytes>,
        payload: Bytes,
    },
    Ping,
    Pong,
    Ok,
    Err {
        error_code: ErrorCode,
    },
}

pub trait ProtocolCodec {
    type Inbound;
    type Outbound;

    fn encode(msg: &Self::Outbound) -> BytesMut;
    fn decode(buf: &mut BytesMut) -> Result<Self::Inbound, DecodeError>;
}

#[allow(dead_code)]
pub struct ServerCodec;
#[allow(dead_code)]
pub struct ClientCodec;

impl ProtocolCodec for ServerCodec {
    type Inbound = ServerInbound;
    type Outbound = ServerOutbound;

    fn encode(msg: &ServerOutbound) -> BytesMut {
        let message = match msg {
            ServerOutbound::Info {
                version,
                max_payload,
                server_id,
                server_name,
                capability_flags,
            } => Message::Info {
                version: *version,
                max_payload: *max_payload,
                server_id: server_id.clone(),
                server_name: server_name.clone(),
                capability_flags: *capability_flags,
            },
            ServerOutbound::Msg { topic, subscription_id, header, payload } => Message::Msg {
                topic: topic.clone(),
                subscription_id: subscription_id.clone(),
                header: header.clone(),
                payload: payload.clone(),
            },
            ServerOutbound::Ping => Message::Ping,
            ServerOutbound::Pong => Message::Pong,
            ServerOutbound::Ok => Message::Ok,
            ServerOutbound::Err { error_code } => Message::Err { error_code: *error_code },
        };
        message.encode()
    }

    fn decode(buf: &mut BytesMut) -> Result<ServerInbound, DecodeError> {
        let message = Message::decode(buf)?;
        match message {
            Message::Connect { version, verbose, auth } => {
                Ok(ServerInbound::Connect { version, verbose, auth })
            }
            Message::Pub { topic, header, payload } => {
                Ok(ServerInbound::Pub { topic, header, payload })
            }
            Message::Sub { topic, subscription_id, queue_group } => {
                Ok(ServerInbound::Sub { topic, subscription_id, queue_group })
            }
            Message::Unsub { subscription_id } => Ok(ServerInbound::Unsub { subscription_id }),
            Message::Ping => Ok(ServerInbound::Ping),
            Message::Pong => Ok(ServerInbound::Pong),
            other => Err(DecodeError::UnknownCommand(other.command_code())),
        }
    }
}

impl ProtocolCodec for ClientCodec {
    type Inbound = ClientInbound;
    type Outbound = ClientOutbound;

    fn encode(msg: &ClientOutbound) -> BytesMut {
        let message = match msg {
            ClientOutbound::Connect { version, verbose, auth } => {
                Message::Connect { version: *version, verbose: *verbose, auth: auth.clone() }
            }
            ClientOutbound::Pub { topic, header, payload } => Message::Pub {
                topic: topic.clone(),
                header: header.clone(),
                payload: payload.clone(),
            },
            ClientOutbound::Sub { topic, subscription_id, queue_group } => Message::Sub {
                topic: topic.clone(),
                subscription_id: subscription_id.clone(),
                queue_group: queue_group.clone(),
            },
            ClientOutbound::Unsub { subscription_id } => {
                Message::Unsub { subscription_id: subscription_id.clone() }
            }
            ClientOutbound::Ping => Message::Ping,
            ClientOutbound::Pong => Message::Pong,
        };
        message.encode()
    }

    fn decode(buf: &mut BytesMut) -> Result<ClientInbound, DecodeError> {
        let message = Message::decode(buf)?;
        match message {
            Message::Info { version, max_payload, server_id, server_name, capability_flags } => {
                Ok(ClientInbound::Info {
                    version,
                    max_payload,
                    server_id,
                    server_name,
                    capability_flags,
                })
            }
            Message::Msg { topic, subscription_id, header, payload } => {
                Ok(ClientInbound::Msg { topic, subscription_id, header, payload })
            }
            Message::Ping => Ok(ClientInbound::Ping),
            Message::Pong => Ok(ClientInbound::Pong),
            Message::Ok => Ok(ClientInbound::Ok),
            Message::Err { error_code } => Ok(ClientInbound::Err { error_code }),
            other => Err(DecodeError::UnknownCommand(other.command_code())),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- varint --

    #[test]
    fn varint_roundtrips_boundary_values() {
        let boundary_values =
            [0u32, 1, 127, 128, 16_383, 16_384, 2_097_151, 2_097_152, VARINT_MAX_VALUE];
        for &v in &boundary_values {
            let mut buf = BytesMut::new();
            encode_varint(v, &mut buf);
            let decoded = decode_varint(&mut buf).unwrap();
            assert_eq!(v, decoded, "varint roundtrip failed for {v}");
        }
    }

    #[test]
    fn varint_uses_expected_byte_counts() {
        let cases: &[(u32, usize)] = &[
            (0, 1),
            (127, 1),
            (128, 2),
            (16_383, 2),
            (16_384, 3),
            (2_097_151, 3),
            (2_097_152, 4),
            (VARINT_MAX_VALUE, 4),
        ];
        for &(value, expected_size) in cases {
            let mut buf = BytesMut::new();
            encode_varint(value, &mut buf);
            assert_eq!(buf.len(), expected_size, "unexpected byte count for {value}");
            assert_eq!(varint_encoded_size(value), expected_size);
        }
    }

    #[test]
    fn varint_300_encodes_to_spec_example() {
        let mut buf = BytesMut::new();
        encode_varint(300, &mut buf);
        assert_eq!(&buf[..], &[0xAC, 0x02]);
    }

    #[test]
    fn varint_decode_rejects_too_many_continuation_bytes() {
        let mut buf = BytesMut::from(&[0x80, 0x80, 0x80, 0x80, 0x01][..]);
        assert_eq!(decode_varint(&mut buf), Err(DecodeError::VarintTooLong));
    }

    #[test]
    fn varint_decode_returns_incomplete_on_empty_buffer() {
        let mut buf = BytesMut::new();
        assert_eq!(decode_varint(&mut buf), Err(DecodeError::Incomplete));
    }

    // -- PING / PONG / OK (zero-length body) --

    #[test]
    fn ping_encodes_to_spec_wire_format() {
        let wire = Message::Ping.encode();
        assert_eq!(&wire[..], &[0x70, 0x00]);
    }

    #[test]
    fn pong_roundtrips() {
        let encoded = Message::Pong.encode();
        let mut buf = encoded.clone();
        let decoded = Message::decode(&mut buf).unwrap();
        assert_eq!(decoded, Message::Pong);
    }

    #[test]
    fn ok_roundtrips() {
        let encoded = Message::Ok.encode();
        let mut buf = encoded.clone();
        let decoded = Message::decode(&mut buf).unwrap();
        assert_eq!(decoded, Message::Ok);
    }

    // -- ERR --

    #[test]
    fn err_roundtrips_all_error_codes() {
        let codes = [
            ErrorCode::UnknownProtocol,
            ErrorCode::AuthFailed,
            ErrorCode::AuthRequired,
            ErrorCode::InvalidTopic,
            ErrorCode::PayloadTooLarge,
            ErrorCode::ParseError,
            ErrorCode::UnknownSubscription,
        ];
        for code in codes {
            let msg = Message::Err { error_code: code };
            let mut buf = msg.encode();
            let decoded = Message::decode(&mut buf).unwrap();
            assert_eq!(decoded, msg);
        }
    }

    #[test]
    fn err_rejects_unknown_error_code() {
        let mut buf = BytesMut::new();
        buf.put_u8((Command::Err as u8) << COMMAND_SHIFT);
        encode_varint(2, &mut buf);
        buf.put_u16_le(0x0000);
        assert_eq!(Message::decode(&mut buf), Err(DecodeError::UnknownErrorCode(0x0000)));
    }

    // -- INFO --

    #[test]
    fn info_roundtrips_with_all_capabilities() {
        let msg = Message::Info {
            version: PROTOCOL_VERSION,
            max_payload: DEFAULT_MAX_PAYLOAD,
            server_id: "server-1".into(),
            server_name: "My Server".into(),
            capability_flags: CapabilityFlags { auth_required: true, headers: true },
        };
        let mut buf = msg.encode();
        let decoded = Message::decode(&mut buf).unwrap();
        assert_eq!(decoded, msg);
    }

    #[test]
    fn info_roundtrips_with_no_capabilities() {
        let msg = Message::Info {
            version: PROTOCOL_VERSION,
            max_payload: DEFAULT_MAX_PAYLOAD,
            server_id: "abc".into(),
            server_name: "".into(),
            capability_flags: CapabilityFlags::default(),
        };
        let mut buf = msg.encode();
        let decoded = Message::decode(&mut buf).unwrap();
        assert_eq!(decoded, msg);
    }

    // -- CONNECT --

    #[test]
    fn connect_without_auth_roundtrips() {
        let msg = Message::Connect { version: PROTOCOL_VERSION, verbose: false, auth: None };
        let mut buf = msg.encode();
        let decoded = Message::decode(&mut buf).unwrap();
        assert_eq!(decoded, msg);
    }

    #[test]
    fn connect_with_verbose_and_password_auth_roundtrips() {
        let msg = Message::Connect {
            version: PROTOCOL_VERSION,
            verbose: true,
            auth: Some(Auth::Password { username: "alice".into(), password: "s3cret".into() }),
        };
        let mut buf = msg.encode();
        let decoded = Message::decode(&mut buf).unwrap();
        assert_eq!(decoded, msg);
    }

    #[test]
    fn connect_with_jwt_auth_roundtrips() {
        let msg = Message::Connect {
            version: PROTOCOL_VERSION,
            verbose: false,
            auth: Some(Auth::Jwt { token: "eyJhbGciOiJIUzI1NiJ9.test.sig".into() }),
        };
        let mut buf = msg.encode();
        let decoded = Message::decode(&mut buf).unwrap();
        assert_eq!(decoded, msg);
    }

    // -- PUB --

    #[test]
    fn pub_without_header_encodes_to_spec_wire_format() {
        let msg = Message::Pub {
            topic: BytesMut::from("chat"),
            header: None,
            payload: Bytes::from("Hello"),
        };
        let wire = msg.encode();
        // remaining_length = topic_length(2) + topic(4) + payload_size_varint(1) + payload(5) = 12 = 0x0C
        // Note: the spec appendix states 0x0B, but that is an off-by-one error in the example.
        let expected: &[u8] =
            &[0x30, 0x0C, 0x04, 0x00, b'c', b'h', b'a', b't', 0x05, b'H', b'e', b'l', b'l', b'o'];
        assert_eq!(&wire[..], expected);
    }

    #[test]
    fn pub_without_header_roundtrips() {
        let msg = Message::Pub {
            topic: BytesMut::from("events.user.created"),
            header: None,
            payload: Bytes::from("payload-data"),
        };
        let mut buf = msg.encode();
        let decoded = Message::decode(&mut buf).unwrap();
        assert_eq!(decoded, msg);
    }

    #[test]
    fn pub_with_header_roundtrips() {
        let msg = Message::Pub {
            topic: BytesMut::from("topic"),
            header: Some(Bytes::from_static(b"\x01\x02\x03")),
            payload: Bytes::from("body"),
        };
        let mut buf = msg.encode();
        let decoded = Message::decode(&mut buf).unwrap();
        assert_eq!(decoded, msg);
    }

    #[test]
    fn pub_with_empty_payload_roundtrips() {
        let msg = Message::Pub { topic: BytesMut::from("t"), header: None, payload: Bytes::new() };
        let mut buf = msg.encode();
        let decoded = Message::decode(&mut buf).unwrap();
        assert_eq!(decoded, msg);
    }

    // -- SUB --

    #[test]
    fn sub_without_queue_group_roundtrips() {
        let msg = Message::Sub {
            topic: BytesMut::from("news.*"),
            subscription_id: Bytes::from("sub-1"),
            queue_group: None,
        };
        let mut buf = msg.encode();
        let decoded = Message::decode(&mut buf).unwrap();
        assert_eq!(decoded, msg);
    }

    #[test]
    fn sub_with_queue_group_roundtrips() {
        let msg = Message::Sub {
            topic: BytesMut::from("orders.>"),
            subscription_id: Bytes::from("sub-2"),
            queue_group: Some("workers".into()),
        };
        let mut buf = msg.encode();
        let decoded = Message::decode(&mut buf).unwrap();
        assert_eq!(decoded, msg);
    }

    // -- UNSUB --

    #[test]
    fn unsub_roundtrips() {
        let msg = Message::Unsub { subscription_id: Bytes::from("sub-1") };
        let mut buf = msg.encode();
        let decoded = Message::decode(&mut buf).unwrap();
        assert_eq!(decoded, msg);
    }

    // -- MSG --

    #[test]
    fn msg_without_header_roundtrips() {
        let msg = Message::Msg {
            topic: BytesMut::from("chat"),
            subscription_id: Bytes::from("sub-99"),
            header: None,
            payload: Bytes::from("hello world"),
        };
        let mut buf = msg.encode();
        let decoded = Message::decode(&mut buf).unwrap();
        assert_eq!(decoded, msg);
    }

    #[test]
    fn msg_with_header_roundtrips() {
        let msg = Message::Msg {
            topic: BytesMut::from("events"),
            subscription_id: Bytes::from("s1"),
            header: Some(Bytes::from_static(b"hdr")),
            payload: Bytes::from("data"),
        };
        let mut buf = msg.encode();
        let decoded = Message::decode(&mut buf).unwrap();
        assert_eq!(decoded, msg);
    }

    // -- Error cases --

    #[test]
    fn decode_rejects_reserved_command_zero() {
        let mut buf = BytesMut::from(&[0x00, 0x00][..]);
        assert_eq!(Message::decode(&mut buf), Err(DecodeError::ReservedCommand));
    }

    #[test]
    fn decode_rejects_unknown_command() {
        let mut buf = BytesMut::from(&[0xF0, 0x00][..]);
        assert_eq!(Message::decode(&mut buf), Err(DecodeError::UnknownCommand(0xF)));
    }

    #[test]
    fn decode_rejects_trailing_bytes() {
        // Encode a valid Ping, then append a spurious extra byte.
        // Message::decode must consume exactly remaining_length bytes and then
        // reject the leftover byte rather than silently ignoring it.
        let mut buf = BytesMut::new();
        buf.put_u8((Command::Ping as u8) << COMMAND_SHIFT);
        encode_varint(1, &mut buf);
        buf.put_u8(0x00);
        assert_eq!(Message::decode(&mut buf), Err(DecodeError::TrailingBytes));
    }

    #[test]
    fn decode_rejects_invalid_flags_on_ping() {
        let mut buf = BytesMut::new();
        buf.put_u8(((Command::Ping as u8) << COMMAND_SHIFT) | 0x01);
        encode_varint(0, &mut buf);
        assert_eq!(
            Message::decode(&mut buf),
            Err(DecodeError::InvalidFlags { command: Command::Ping, flags: 0x01 })
        );
    }

    #[test]
    fn decode_rejects_reserved_connect_flags() {
        let mut buf = BytesMut::new();
        let bad_flags = CONNECT_RESERVED_MASK;
        buf.put_u8(((Command::Connect as u8) << COMMAND_SHIFT) | bad_flags);
        encode_varint(1, &mut buf);
        buf.put_u8(PROTOCOL_VERSION);
        assert_eq!(
            Message::decode(&mut buf),
            Err(DecodeError::InvalidFlags { command: Command::Connect, flags: bad_flags })
        );
    }

    #[test]
    fn decode_returns_incomplete_on_truncated_fixed_header() {
        let mut buf = BytesMut::new();
        assert_eq!(Message::decode(&mut buf), Err(DecodeError::Incomplete));
    }

    #[test]
    fn decode_returns_incomplete_on_truncated_body() {
        let mut buf = BytesMut::new();
        buf.put_u8((Command::Err as u8) << COMMAND_SHIFT);
        encode_varint(2, &mut buf);
        buf.put_u8(0x01);
        assert_eq!(Message::decode(&mut buf), Err(DecodeError::Incomplete));
    }

    #[test]
    fn topic_exceeding_max_length_is_rejected() {
        let long_topic = vec![b'a'; (MAX_TOPIC_LENGTH as usize) + 1];

        let mut raw = BytesMut::new();
        raw.put_u8((Command::Pub as u8) << COMMAND_SHIFT);

        let mut body = BytesMut::new();
        body.put_u16_le(long_topic.len() as u16);
        body.put_slice(&long_topic);
        encode_varint(0, &mut body);

        encode_varint(body.len() as u32, &mut raw);
        raw.unsplit(body);

        assert_eq!(
            Message::decode(&mut raw),
            Err(DecodeError::TopicTooLong((MAX_TOPIC_LENGTH) + 1))
        );
    }

    // -- Multiple messages in sequence --

    #[test]
    fn decodes_multiple_messages_from_single_buffer() {
        let messages = vec![
            Message::Ping,
            Message::Pong,
            Message::Ok,
            Message::Err { error_code: ErrorCode::ParseError },
        ];

        let mut buf = BytesMut::new();
        for msg in &messages {
            buf.unsplit(msg.encode());
        }

        for expected in &messages {
            let decoded = Message::decode(&mut buf).unwrap();
            assert_eq!(&decoded, expected);
        }

        assert!(!buf.has_remaining());
    }

    // -- Large payload with multi-byte varint --

    #[test]
    fn pub_with_large_payload_roundtrips() {
        let payload = Bytes::from(vec![0xAB_u8; 20_000]);
        let msg = Message::Pub { topic: BytesMut::from("big"), header: None, payload };
        let mut buf = msg.encode();
        let decoded = Message::decode(&mut buf).unwrap();
        assert_eq!(decoded, msg);
    }
}
