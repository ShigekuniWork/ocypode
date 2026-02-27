pub mod info;

pub use info::Info;

/// All protocol messages that can be sent or received.
pub enum Message {
    Info(Info),
}
