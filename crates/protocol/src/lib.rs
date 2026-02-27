//! Ocypode Protocol
//!
//! This crate defines the Ocypode wire protocol primitives shared by the
//! client and server.
//!
//! Command identifiers are aligned with the protocol specification.
//!
//! TODO: Link to a stable, versioned protocol specification once its location
//! is finalized.

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
