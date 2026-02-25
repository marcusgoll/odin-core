//! Secrets and session interfaces using opaque handles.

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct SecretHandle(pub String);

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct SessionHandle(pub String);

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccessContext {
    pub plugin: String,
    pub project: String,
    pub capability: String,
    pub reason: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecretRef {
    pub handle: SecretHandle,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionLease {
    pub handle: SessionHandle,
    pub expires_at_unix: u64,
    pub reauth_required: bool,
}

#[derive(Debug, Error)]
pub enum SecretError {
    #[error("unauthorized: {0}")]
    Unauthorized(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("backend failure: {0}")]
    Backend(String),
}

pub trait SecretStore: Send + Sync {
    fn resolve_secret_handle(
        &self,
        handle: &SecretHandle,
        ctx: &AccessContext,
    ) -> Result<SecretRef, SecretError>;
}

pub trait SessionVault: Send + Sync {
    fn issue_session_lease(
        &self,
        handle: &SessionHandle,
        ctx: &AccessContext,
    ) -> Result<SessionLease, SecretError>;
}

#[derive(Clone, Debug, Default)]
pub struct HandleOnlyStore;

impl SecretStore for HandleOnlyStore {
    fn resolve_secret_handle(
        &self,
        handle: &SecretHandle,
        _ctx: &AccessContext,
    ) -> Result<SecretRef, SecretError> {
        Ok(SecretRef {
            handle: handle.clone(),
        })
    }
}

impl SessionVault for HandleOnlyStore {
    fn issue_session_lease(
        &self,
        handle: &SessionHandle,
        _ctx: &AccessContext,
    ) -> Result<SessionLease, SecretError> {
        Ok(SessionLease {
            handle: handle.clone(),
            expires_at_unix: u64::MAX,
            reauth_required: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_handle_without_exposing_secret_values() {
        let store = HandleOnlyStore;
        let secret = store
            .resolve_secret_handle(
                &SecretHandle("secret://test/key".to_string()),
                &AccessContext {
                    plugin: "p".to_string(),
                    project: "proj".to_string(),
                    capability: "read".to_string(),
                    reason: "unit".to_string(),
                },
            )
            .expect("resolve");

        assert_eq!(secret.handle.0, "secret://test/key");
    }
}
