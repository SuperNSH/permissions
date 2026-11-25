//! The platform bridges. Every bridge compiles on every host — its rules are pure mapping logic —
//! so its tests run everywhere; `default_backend` picks the one this build runs on.

mod android;
mod apple;
mod linux;
mod notifications;
mod unsupported;
mod web;
mod windows;

use std::sync::Arc;

use crate::backend::PermissionBackend;

/// The backend of the target this build runs on.
///
/// The `cfg!` chain keeps every bridge compiled and referenced on every host, so no bridge goes
/// untested on the machines that run CI. Notifications are the one kind NSH 0.7.1 answers itself,
/// so Apple, Android and the web serve them through `notifications::via_nsh`.
pub(crate) fn default_backend() -> Arc<dyn PermissionBackend> {
    if cfg!(target_vendor = "apple") {
        notifications::via_nsh(apple::default_backend())
    } else if cfg!(target_os = "android") {
        notifications::via_nsh(android::default_backend())
    } else if cfg!(target_os = "windows") {
        windows::default_backend()
    } else if cfg!(target_os = "linux") {
        linux::default_backend()
    } else if cfg!(target_arch = "wasm32") {
        notifications::via_nsh(web::default_backend())
    } else {
        unsupported::default_backend()
    }
}
