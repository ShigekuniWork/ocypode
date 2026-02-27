pub mod connect;
pub mod info;

pub use connect::Connect;
pub use info::Info;

/// All protocol messages that can be sent or received.
pub enum Message {
    Info(Info),
    Connect(Connect),
}
