// TODO: This module drives the per-connection handshake state machine.
//       The type-state pattern enforces correct ordering at compile time:
//       frames cannot be dispatched until authentication succeeds.

use thiserror::Error;

use crate::{
    auth::{AuthOutcome, Authenticator},
    parser::pb,
};

/// Initial state: INFO has been sent to the client, CONNECT has not yet arrived.
pub struct PendingHandshake {
    pub client_id: u64,
}

/// Terminal state: CONNECT received and authentication succeeded.
pub struct CompletedHandshake {
    pub client_id: u64,
    /// The CONNECT message received from the client; available for future dispatch logic.
    #[allow(dead_code)]
    pub connect_info: pb::Connect,
}

#[allow(dead_code)]
#[derive(Debug, Error)]
pub enum HandshakeError {
    #[error("CONNECT timeout")]
    ConnectTimeout,
    #[error("connection closed before CONNECT")]
    ConnectionClosed,
    /// Reserved for detecting frames arriving after the handshake is complete.
    #[error("unexpected frame received after handshake")]
    UnexpectedFrame,
    #[error("authentication failed: {reason}")]
    AuthenticationFailed { reason: String },
}

impl PendingHandshake {
    pub fn new(client_id: u64) -> Self {
        Self { client_id }
    }

    /// Validates the CONNECT message and transitions to the completed state.
    pub fn on_connect(
        self,
        connect: pb::Connect,
        authenticator: &dyn Authenticator,
    ) -> Result<CompletedHandshake, HandshakeError> {
        match authenticator.authenticate(&connect) {
            AuthOutcome::Accepted => {
                Ok(CompletedHandshake { client_id: self.client_id, connect_info: connect })
            }
            AuthOutcome::Rejected { reason } => {
                Err(HandshakeError::AuthenticationFailed { reason })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::NoAuthAuthenticator;

    #[test]
    fn on_connect_transitions_to_completed_with_no_auth() {
        let pending = PendingHandshake::new(42);
        let connect = pb::Connect {
            version: 1,
            verbose: false,
            auth_method: pb::AuthMethod::NoAuth as i32,
            credentials: None,
        };
        let completed = pending.on_connect(connect, &NoAuthAuthenticator).unwrap();
        assert_eq!(completed.client_id, 42);
    }
}
