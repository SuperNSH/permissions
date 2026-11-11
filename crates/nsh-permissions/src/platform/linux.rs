//! The Linux bridge: no raw seam exists this release.
//!
//! Desktop Linux has no standard per-app runtime permission model — xdg-desktop-portal serves
//! sandboxes that opt in, and a flatpak answer would not hold for a plain binary — so every
//! kind is coded `ERR_PERMISSIONS_UNSUPPORTED_PLATFORM` instead of guessed at.

use std::{future::ready, sync::Arc};

use crate::{
    backend::{BackendFuture, PermissionBackend},
    error::{ErrorKind, PermissionsError},
    permission::Permission,
    status::Status,
};

/// The Linux backend: the frozen support matrix serves no kind here.
struct LinuxBridge;

impl PermissionBackend for LinuxBridge {
    fn check(&self, _kind: Permission) -> BackendFuture<'_, Result<Status, PermissionsError>> {
        unsupported()
    }

    fn request(&self, _kind: Permission) -> BackendFuture<'_, Result<Status, PermissionsError>> {
        unsupported()
    }

    fn open_settings(&self, _kind: Permission) -> BackendFuture<'_, Result<(), PermissionsError>> {
        unsupported()
    }
}

/// Every kind's answer, ready on first poll so no call can hang. A denial here would lie;
/// only the coded error tells the truth.
fn unsupported<T: 'static>() -> BackendFuture<'static, Result<T, PermissionsError>> {
    Box::pin(ready(Err(PermissionsError::new(
        ErrorKind::UnsupportedPlatform,
    ))))
}

/// The Linux platform backend.
pub fn default_backend() -> Arc<dyn PermissionBackend> {
    Arc::new(LinuxBridge)
}

#[cfg(test)]
mod tests {
    use super::*;

    const EVERY_KIND: [Permission; 16] = [
        Permission::Camera,
        Permission::Microphone,
        Permission::Photos,
        Permission::PhotosAddOnly,
        Permission::Location,
        Permission::LocationAlways,
        Permission::LocationWhenInUse,
        Permission::Notifications,
        Permission::Bluetooth,
        Permission::Speech,
        Permission::Calendar,
        Permission::Contacts,
        Permission::Reminders,
        Permission::Motion,
        Permission::MediaLibrary,
        Permission::AppTrackingTransparency,
    ];

    #[tokio::test]
    async fn linux_every_kind_is_coded_unsupported() {
        let backend = default_backend();
        for kind in EVERY_KIND {
            assert_eq!(
                backend.check(kind).await.unwrap_err().code(),
                "ERR_PERMISSIONS_UNSUPPORTED_PLATFORM",
                "check {kind:?}"
            );
            assert_eq!(
                backend.request(kind).await.unwrap_err().code(),
                "ERR_PERMISSIONS_UNSUPPORTED_PLATFORM",
                "request {kind:?}"
            );
            assert_eq!(
                backend.open_settings(kind).await.unwrap_err().code(),
                "ERR_PERMISSIONS_UNSUPPORTED_PLATFORM",
                "open_settings {kind:?}"
            );
        }
    }
}
