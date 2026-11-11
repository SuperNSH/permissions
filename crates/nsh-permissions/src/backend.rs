//! The seam between [`crate::Permissions`] and a platform: three calls, each answering once.

use std::{future::Future, pin::Pin};

use crate::{error::PermissionsError, permission::Permission, status::Status};

/// A future a backend returns. Each call's future borrows the backend.
pub type BackendFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;

/// What a platform provides. [`crate::Permissions`] adds the rules: one request per kind, one
/// resolution per call, a deadline on everything.
pub trait PermissionBackend: Send + Sync {
    /// The status of `kind` now, without a prompt.
    fn check(&self, kind: Permission) -> BackendFuture<'_, Result<Status, PermissionsError>>;

    /// Shows the system's prompt for `kind` and answers with the resulting status.
    fn request(&self, kind: Permission) -> BackendFuture<'_, Result<Status, PermissionsError>>;

    /// Opens the app's settings page for `kind`.
    fn open_settings(&self, kind: Permission) -> BackendFuture<'_, Result<(), PermissionsError>>;
}
