//! The platform bridges. Every bridge compiles on every host — its rules are pure mapping logic —
//! so its tests run everywhere; `default_backend` picks the one this build runs on.

mod android;
mod apple;
mod linux;
mod unsupported;
mod web;
mod windows;

use std::sync::Arc;

use crate::backend::PermissionBackend;

/// The backend of the target this build runs on.
///
/// The `cfg!` chain keeps every bridge compiled and referenced on every host, so no bridge goes
/// untested on the machines that run CI.
pub(crate) fn default_backend() -> Arc<dyn PermissionBackend> {
    if cfg!(target_vendor = "apple") {
        apple::default_backend()
    } else if cfg!(target_os = "android") {
        android::default_backend()
    } else if cfg!(target_os = "windows") {
        windows::default_backend()
    } else if cfg!(target_os = "linux") {
        linux::default_backend()
    } else if cfg!(target_arch = "wasm32") {
        web::default_backend()
    } else {
        unsupported::default_backend()
    }
}
