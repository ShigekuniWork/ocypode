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
//! TODO: Implement codecs for PING, PONG, OK, ERR.

pub mod codec;
pub mod error;
pub mod header;
pub mod message;
pub mod wire;

pub use codec::{ClientCodec, ServerCodec};
pub use error::{DecodeError, EncodeError};
pub use message::Message;
pub use wire::{CommandCodec, Headers, Payload};

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
        if (Command::INFO as u8..=Command::ERR as u8).contains(&value) {
            // SAFETY: Command is #[repr(u8)] with contiguous discriminants INFO..=ERR,
            // and value has been verified to be within that range.
            Ok(unsafe { std::mem::transmute::<u8, Command>(value) })
        } else {
            Err(DecodeError::UnknownCommand(value))
        }
    }
}
