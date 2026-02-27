//! Ocypode Protocol
//!
//! This crate defines the Ocypode wire protocol primitives shared by the
//! client and server.
//!
//! Command identifiers are aligned with the protocol specification.
//!
//! TODO: Link to a stable, versioned protocol specification once its location
//! is finalized.
//!
//! TODO: Implement codecs for CONNECT, PUB, SUB, UNSUB, MSG, PING, PONG, OK, ERR.

pub mod codec;
pub mod error;
pub mod header;
pub mod message;
pub mod wire;

pub use codec::{ClientCodec, CommandCodec, ServerCodec};
pub use error::DecodeError;
pub use message::Message;
pub use wire::{Headers, Payload, Topic};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Command {
    /// Sent to the client after a `QUIC` connection is established.
    INFO = 0x1,
    /// Sent to the server to establish a connection.
    CONNECT = 0x2,
    /// Used to publish a message to a specified topic.
    PUB = 0x3,
    /// Used to subscribe to a topic or wildcard topic.
    SUB = 0x4,
    /// Used to end a subscription.
    UNSUB = 0x5,
    /// Delivers a message to clients subscribed to a topic.
    MSG = 0x6,
    /// Used as a keep-alive message.
    PING = 0x7,
    /// Response to a keep-alive message.
    PONG = 0x8,
    /// Notifies that a message was received in `verbose` mode.
    OK = 0x9,
    /// Sent when a protocol error occurs.
    ERR = 0xA,
}

impl TryFrom<u8> for Command {
    type Error = DecodeError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x1 => Ok(Command::INFO),
            0x2 => Ok(Command::CONNECT),
            0x3 => Ok(Command::PUB),
            0x4 => Ok(Command::SUB),
            0x5 => Ok(Command::UNSUB),
            0x6 => Ok(Command::MSG),
            0x7 => Ok(Command::PING),
            0x8 => Ok(Command::PONG),
            0x9 => Ok(Command::OK),
            0xA => Ok(Command::ERR),
            other => Err(DecodeError::UnknownCommand(other)),
        }
    }
}
