// TODO: This module will sit between frame dispatch and routing.
//       Each inbound PUB/SUB command will be validated against the client's permission set
//       before being forwarded to the router. Cedar-based policy evaluation is planned.

/// Checks whether a client is authorized for publish or subscribe operations.
// TODO: Implement with Cedar policy engine for fine-grained, attribute-based access control.
#[allow(dead_code)]
pub trait PermissionChecker: Send + Sync + 'static {
    // TODO: fn check_publish(&self, subject: &str, client_id: u64) -> bool;
    // TODO: fn check_subscribe(&self, subject: &str, client_id: u64) -> bool;
}
