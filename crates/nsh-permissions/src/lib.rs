//! Runtime permissions for Native Shell apps: check, request and open settings for the camera,
//! the microphone, photos, location, notifications and more.
//!
//! [`Permissions`] follows Flutter's `permission_handler`: ask for a kind, get a [`Status`], and
//! open the app's settings when the answer is permanent. Each call resolves exactly once, with a
//! [`Status`] or a [`PermissionsError`] carrying a stable code.

mod backend;
mod error;
mod fake;
mod native;
mod permission;
mod permissions;
mod platform;
mod status;

use nsh_core::prelude::*;

pub use backend::{BackendFuture, PermissionBackend};
pub use error::{ErrorKind, PermissionsError};
pub use fake::FakeBackend;
pub use native::NativePermissions;
pub use permission::{Permission, Platform, kind_from_wire, kind_wire_name, supports};
pub use permissions::{Permissions, Timeouts};
pub use status::{Status, status_from_wire, status_wire_name};

/// The crate's UI set: what TSX imports from `nsh-permissions/native`.
#[native_module("nsh-permissions/native")]
fn native(registry: &mut Registry) {
    registry.class::<NativePermissions>();
}
