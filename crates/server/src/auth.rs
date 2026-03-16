// TODO: This module handles connection-time authentication (CONNECT message verification).
//       Future work: PasswordAuthenticator, JWT-based auth, etc.
//       Authorization (subject-level permissions) is handled separately in permission.rs.

use crate::parser::pb;

#[allow(dead_code)]
pub enum AuthOutcome {
    Accepted,
    Rejected { reason: String },
}

/// Validates credentials presented in the CONNECT message.
pub trait Authenticator: Send + Sync + 'static {
    fn authenticate(&self, connect: &pb::Connect) -> AuthOutcome;
}

/// Accepts all connections without credential verification.
pub struct NoAuthAuthenticator;

impl Authenticator for NoAuthAuthenticator {
    fn authenticate(&self, _connect: &pb::Connect) -> AuthOutcome {
        AuthOutcome::Accepted
    }
}
