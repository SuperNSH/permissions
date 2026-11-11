//! The backend for a target with no bridge: every call reports the missing bridge, never a denial.

use std::{future::ready, sync::Arc};

use crate::{
    backend::{BackendFuture, PermissionBackend},
    error::{ErrorKind, PermissionsError},
    permission::Permission,
    status::Status,
};

/// Answers `ERR_PERMISSIONS_BRIDGE_UNAVAILABLE` for every call: honest, coded, never hanging.
struct Unsupported;

impl PermissionBackend for Unsupported {
    fn check(&self, _kind: Permission) -> BackendFuture<'_, Result<Status, PermissionsError>> {
        unavailable()
    }

    fn request(&self, _kind: Permission) -> BackendFuture<'_, Result<Status, PermissionsError>> {
        unavailable()
    }

    fn open_settings(&self, _kind: Permission) -> BackendFuture<'_, Result<(), PermissionsError>> {
        unavailable()
    }
}

fn unavailable<T: 'static>() -> BackendFuture<'static, Result<T, PermissionsError>> {
    Box::pin(ready(Err(PermissionsError::new(
        ErrorKind::BridgeUnavailable,
    ))))
}

/// The backend of a target with no bridge of its own.
pub(super) fn default_backend() -> Arc<dyn PermissionBackend> {
    Arc::new(Unsupported)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn every_call_answers_bridge_unavailable() {
        let backend = default_backend();
        assert_eq!(
            backend.check(Permission::Camera).await.unwrap_err().kind(),
            ErrorKind::BridgeUnavailable
        );
        assert_eq!(
            backend
                .request(Permission::Camera)
                .await
                .unwrap_err()
                .kind(),
            ErrorKind::BridgeUnavailable
        );
        assert_eq!(
            backend
                .open_settings(Permission::Camera)
                .await
                .unwrap_err()
                .kind(),
            ErrorKind::BridgeUnavailable
        );
    }
}
