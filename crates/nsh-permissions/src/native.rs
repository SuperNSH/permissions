//! The permissions as TSX sees them: `NativePermissions`, a class whose async members reject with
//! a `code` on failure. Results are plain wire strings; prompts stay on the Rust side.

use nsh_core::prelude::*;

use crate::{
    Permissions, PermissionsError,
    error::ErrorKind,
    permission::{Permission, kind_from_wire},
    status::{Status, status_wire_name},
};

/// A permissions handle. Create one and keep it; every handle shares the platform's request gates.
#[class]
pub struct NativePermissions {
    permissions: Permissions,
}

#[class]
impl NativePermissions {
    /// The platform's permissions.
    pub fn new() -> NativePermissions {
        NativePermissions {
            permissions: Permissions::new(),
        }
    }

    /// The status of `kind` (such as `"camera"`): one of the five status wire names.
    pub async fn check(this: Instance<Self>, kind: String) -> Result<String, JsError> {
        let permissions = this.with(|native| native.permissions.clone());
        let kind = to_kind(&kind)?;
        let status = permissions.check(kind).await.map_err(into_js)?;
        Ok(status_name(status))
    }

    /// Shows the system's prompt for `kind` and answers with the resulting status.
    pub async fn request(this: Instance<Self>, kind: String) -> Result<String, JsError> {
        let permissions = this.with(|native| native.permissions.clone());
        let kind = to_kind(&kind)?;
        let status = permissions.request(kind).await.map_err(into_js)?;
        Ok(status_name(status))
    }

    /// Opens the app's settings page for `kind`.
    pub async fn open_settings(this: Instance<Self>, kind: String) -> Result<(), JsError> {
        let permissions = this.with(|native| native.permissions.clone());
        let kind = to_kind(&kind)?;
        permissions.open_settings(kind).await.map_err(into_js)
    }
}

impl Default for NativePermissions {
    fn default() -> Self {
        Self::new()
    }
}

/// The error a rejection carries: its message, and the `code` apps match on.
fn into_js(error: PermissionsError) -> JsError {
    JsError::Error(error.message().to_owned()).with_code(error.code())
}

/// The kind a wire string names, refusing one TSX should never send.
fn to_kind(kind: &str) -> Result<Permission, JsError> {
    kind_from_wire(kind).ok_or_else(|| {
        JsError::Error(format!("unknown permission kind: {kind}"))
            .with_code(ErrorKind::InvalidArgument.code())
    })
}

fn status_name(status: Status) -> String {
    status_wire_name(status).to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    const EVERY_KIND: [(&str, Permission); 16] = [
        ("camera", Permission::Camera),
        ("microphone", Permission::Microphone),
        ("photos", Permission::Photos),
        ("photosAddOnly", Permission::PhotosAddOnly),
        ("location", Permission::Location),
        ("locationAlways", Permission::LocationAlways),
        ("locationWhenInUse", Permission::LocationWhenInUse),
        ("notifications", Permission::Notifications),
        ("bluetooth", Permission::Bluetooth),
        ("speech", Permission::Speech),
        ("calendar", Permission::Calendar),
        ("contacts", Permission::Contacts),
        ("reminders", Permission::Reminders),
        ("motion", Permission::Motion),
        ("mediaLibrary", Permission::MediaLibrary),
        (
            "appTrackingTransparency",
            Permission::AppTrackingTransparency,
        ),
    ];

    const EVERY_STATUS: [(&str, Status); 5] = [
        ("granted", Status::Granted),
        ("denied", Status::Denied),
        ("permanentlyDenied", Status::PermanentlyDenied),
        ("limited", Status::Limited),
        ("restricted", Status::Restricted),
    ];

    #[test]
    fn unknown_kind_string_is_refused() {
        assert_eq!(
            to_kind("location "),
            Err(
                JsError::Error("unknown permission kind: location ".to_owned())
                    .with_code(ErrorKind::InvalidArgument.code())
            )
        );
        assert_eq!(
            to_kind(""),
            Err(JsError::Error("unknown permission kind: ".to_owned())
                .with_code(ErrorKind::InvalidArgument.code()))
        );
    }

    #[test]
    fn every_kind_string_parses_to_its_kind() {
        for (name, kind) in EVERY_KIND {
            assert_eq!(to_kind(name), Ok(kind));
        }
    }

    #[test]
    fn every_status_renders_its_wire_name() {
        for (name, status) in EVERY_STATUS {
            assert_eq!(status_name(status), name);
        }
    }
}
