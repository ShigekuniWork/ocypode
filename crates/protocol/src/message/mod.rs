pub mod connect;
pub mod info;
pub mod publish;
pub mod sub;
pub mod unsub;

pub use connect::Connect;
pub use info::Info;
pub use publish::Pub;
pub use sub::Sub;
pub use unsub::Unsub;

/// All protocol messages that can be sent or received.
pub enum Message {
    Info(Info),
    Connect(Connect),
    Pub(Pub),
    Sub(Sub),
    Unsub(Unsub),
}
