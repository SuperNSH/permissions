//! Android: the manifest answers what can be asked, the rationale signal infers what Android
//! never states outright, and the runtime dialog answers the rest.

use std::{future::ready, sync::Arc};

use crate::{
    backend::{BackendFuture, PermissionBackend},
    error::{ErrorKind, PermissionsError},
    permission::{Permission, Platform, supports},
    status::Status,
};

/// Android 13: `POST_NOTIFICATIONS` exists, and its runtime dialog with it.
const SDK_WITH_RUNTIME_NOTIFICATIONS: u32 = 33;

/// The last `targetSdkVersion` an API 33 device still treats as legacy: no dialog is shown.
const LAST_LEGACY_TARGET: u32 = 32;

/// Android's own answers, as the NSH host reads them over JNI. Every call resolves exactly once,
/// with a coded error; the bridge adds the mapping rules on top.
pub trait AndroidRaw: Send + Sync {
    /// Whether the app's manifest declares the runtime permission for `kind`.
    fn declared(&self, kind: Permission) -> BackendFuture<'_, Result<bool, PermissionsError>>;

    /// `shouldShowRequestPermissionRationale` for `kind`.
    fn rationale(&self, kind: Permission) -> BackendFuture<'_, Result<bool, PermissionsError>>;

    /// Whether a policy the user cannot lift holds the permission.
    fn restricted(&self) -> BackendFuture<'_, Result<bool, PermissionsError>>;

    /// `Build.VERSION.SDK_INT` of the device.
    fn api_level(&self) -> BackendFuture<'_, Result<u32, PermissionsError>>;

    /// The app's `targetSdkVersion`.
    fn target_sdk(&self) -> BackendFuture<'_, Result<u32, PermissionsError>>;

    /// Shows the runtime dialog for `kind` and answers once, with `Granted` or `Denied`.
    fn start_dialog(&self, kind: Permission)
    -> BackendFuture<'_, Result<Status, PermissionsError>>;

    /// Opens the app's settings page for `kind`.
    fn open_settings(&self, kind: Permission) -> BackendFuture<'_, Result<(), PermissionsError>>;
}

/// The production seam, and the NSH-host integration point. NSH 0.4.0 ships no permission
/// bridge, so every call reports `BridgeUnavailable` — a coded setup failure, never a silent
/// denial. The host integration replaces these bodies.
pub struct NshAndroidRaw;

/// Every call's answer until the host bridge exists.
fn unavailable<T: 'static>() -> BackendFuture<'static, Result<T, PermissionsError>> {
    Box::pin(ready(Err(PermissionsError::new(
        ErrorKind::BridgeUnavailable,
    ))))
}

impl AndroidRaw for NshAndroidRaw {
    fn declared(&self, _kind: Permission) -> BackendFuture<'_, Result<bool, PermissionsError>> {
        unavailable()
    }

    fn rationale(&self, _kind: Permission) -> BackendFuture<'_, Result<bool, PermissionsError>> {
        unavailable()
    }

    fn restricted(&self) -> BackendFuture<'_, Result<bool, PermissionsError>> {
        unavailable()
    }

    fn api_level(&self) -> BackendFuture<'_, Result<u32, PermissionsError>> {
        unavailable()
    }

    fn target_sdk(&self) -> BackendFuture<'_, Result<u32, PermissionsError>> {
        unavailable()
    }

    fn start_dialog(
        &self,
        _kind: Permission,
    ) -> BackendFuture<'_, Result<Status, PermissionsError>> {
        unavailable()
    }

    fn open_settings(&self, _kind: Permission) -> BackendFuture<'_, Result<(), PermissionsError>> {
        unavailable()
    }
}

/// Android's mapping rules over any [`AndroidRaw`]. The facade adds the deadline and the
/// one-request-per-kind gate; this layer only maps.
pub struct AndroidBridge<R: AndroidRaw> {
    raw: R,
}

impl<R: AndroidRaw> AndroidBridge<R> {
    /// A backend answering through `raw`.
    pub fn new(raw: R) -> Self {
        Self { raw }
    }

    /// Kinds outside the frozen android row resolve `UnsupportedPlatform` before anything else.
    fn gate(kind: Permission) -> Result<(), PermissionsError> {
        if supports(Platform::Android, kind) {
            Ok(())
        } else {
            Err(PermissionsError::new(ErrorKind::UnsupportedPlatform))
        }
    }

    /// Asks the dialog and maps its answer.
    async fn dialog_status(&self, kind: Permission) -> Result<Status, PermissionsError> {
        match self.raw.start_dialog(kind).await? {
            Status::Granted => Ok(Status::Granted),
            Status::Denied => {
                // Android states no permanent denial of its own: a rationale signal gone
                // false — the dialog will not show again — is what infers it.
                if infers_denial(kind) && !self.raw.rationale(kind).await? {
                    Ok(Status::PermanentlyDenied)
                } else {
                    Ok(Status::Denied)
                }
            }
            // The seam answers Granted or Denied only; a host answering more is broken, not obeyed.
            _ => Err(PermissionsError::new(ErrorKind::Backend)),
        }
    }

    /// Whether notifications are in the legacy regime for this app: the device is below API 33,
    /// or the app still targets 32 or lower, to which an API 33 device shows no dialog. Either
    /// way the install-time grant holds and the seam has nothing to ask.
    async fn legacy_notifications(&self) -> Result<bool, PermissionsError> {
        Ok(self.raw.api_level().await? < SDK_WITH_RUNTIME_NOTIFICATIONS
            || self.raw.target_sdk().await? <= LAST_LEGACY_TARGET)
    }

    /// The status of `kind` through the seam. Check and request share it: the seam's one status
    /// source is the dialog, and no android rule distinguishes the two calls.
    async fn status(&self, kind: Permission) -> Result<Status, PermissionsError> {
        Self::gate(kind)?;
        // An undeclared kind can never be granted, and no dialog exists to show for it.
        if !self.raw.declared(kind).await? {
            return Ok(Status::PermanentlyDenied);
        }
        if self.raw.restricted().await? {
            return Ok(Status::Restricted);
        }
        if kind == Permission::Notifications && self.legacy_notifications().await? {
            return Ok(Status::Granted);
        }
        self.dialog_status(kind).await
    }
}

/// Kinds whose denial reads the rationale signal: notifications' inference, extended to the
/// two split location kinds.
fn infers_denial(kind: Permission) -> bool {
    matches!(
        kind,
        Permission::Notifications | Permission::LocationAlways | Permission::LocationWhenInUse
    )
}

impl<R: AndroidRaw> PermissionBackend for AndroidBridge<R> {
    fn check(&self, kind: Permission) -> BackendFuture<'_, Result<Status, PermissionsError>> {
        Box::pin(self.status(kind))
    }

    fn request(&self, kind: Permission) -> BackendFuture<'_, Result<Status, PermissionsError>> {
        Box::pin(self.status(kind))
    }

    fn open_settings(&self, kind: Permission) -> BackendFuture<'_, Result<(), PermissionsError>> {
        Box::pin(async move {
            Self::gate(kind)?;
            self.raw.open_settings(kind).await
        })
    }
}

/// The Android platform backend.
pub fn default_backend() -> Arc<dyn PermissionBackend> {
    Arc::new(AndroidBridge::new(NshAndroidRaw))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex, PoisonError};

    /// A scripted seam: fixed facts, one fixed dialog answer, and a transcript of dialogs started.
    struct Script {
        declared: bool,
        rationale: bool,
        restricted: bool,
        api: u32,
        target: u32,
        answer: Status,
        dialogs: Arc<Mutex<Vec<Permission>>>,
    }

    impl Script {
        /// A declared, unrestricted API 34 device targeting 34, whose dialog grants.
        fn modern_device() -> Self {
            Self {
                declared: true,
                rationale: false,
                restricted: false,
                api: SDK_WITH_RUNTIME_NOTIFICATIONS + 1,
                target: SDK_WITH_RUNTIME_NOTIFICATIONS + 1,
                answer: Status::Granted,
                dialogs: Arc::new(Mutex::new(Vec::new())),
            }
        }
    }

    /// A raw answer that is already known.
    fn done<T: Send + 'static>(value: T) -> BackendFuture<'static, Result<T, PermissionsError>> {
        Box::pin(ready(Ok(value)))
    }

    impl AndroidRaw for Script {
        fn declared(&self, _kind: Permission) -> BackendFuture<'_, Result<bool, PermissionsError>> {
            done(self.declared)
        }

        fn rationale(
            &self,
            _kind: Permission,
        ) -> BackendFuture<'_, Result<bool, PermissionsError>> {
            done(self.rationale)
        }

        fn restricted(&self) -> BackendFuture<'_, Result<bool, PermissionsError>> {
            done(self.restricted)
        }

        fn api_level(&self) -> BackendFuture<'_, Result<u32, PermissionsError>> {
            done(self.api)
        }

        fn target_sdk(&self) -> BackendFuture<'_, Result<u32, PermissionsError>> {
            done(self.target)
        }

        fn start_dialog(
            &self,
            kind: Permission,
        ) -> BackendFuture<'_, Result<Status, PermissionsError>> {
            self.dialogs
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(kind);
            done(self.answer)
        }

        fn open_settings(
            &self,
            _kind: Permission,
        ) -> BackendFuture<'_, Result<(), PermissionsError>> {
            done(())
        }
    }

    /// A bridge over a script the test changes, with the dialog transcript to assert on.
    fn bridged(
        change: impl FnOnce(&mut Script),
    ) -> (AndroidBridge<Script>, Arc<Mutex<Vec<Permission>>>) {
        let mut script = Script::modern_device();
        change(&mut script);
        let dialogs = Arc::clone(&script.dialogs);
        (AndroidBridge::new(script), dialogs)
    }

    /// How many dialogs the script has started.
    fn started(dialogs: &Arc<Mutex<Vec<Permission>>>) -> usize {
        dialogs.lock().unwrap_or_else(PoisonError::into_inner).len()
    }

    #[tokio::test]
    async fn android_notification_denial_is_inferred_from_rationale() {
        let (backend, dialogs) = bridged(|script| {
            script.answer = Status::Denied;
            // A live rationale signal means the user refused without "don't ask again".
            script.rationale = true;
        });
        assert_eq!(
            backend.request(Permission::Notifications).await,
            Ok(Status::Denied)
        );
        assert_eq!(
            backend.check(Permission::Notifications).await,
            Ok(Status::Denied)
        );
        assert_eq!(started(&dialogs), 2);

        let (backend, _dialogs) = bridged(|script| {
            script.answer = Status::Denied;
            // A rationale signal gone false means the dialog will never show again.
            script.rationale = false;
        });
        assert_eq!(
            backend.request(Permission::Notifications).await,
            Ok(Status::PermanentlyDenied)
        );
    }

    #[tokio::test]
    async fn android_target_sdk_32_never_shows_the_dialog() {
        // A declared kind keeps the legacy install-time grant; there is no dialog to ask with.
        let (backend, dialogs) = bridged(|script| script.target = LAST_LEGACY_TARGET);
        assert_eq!(
            backend.request(Permission::Notifications).await,
            Ok(Status::Granted)
        );
        assert_eq!(
            backend.check(Permission::Notifications).await,
            Ok(Status::Granted)
        );
        assert_eq!(started(&dialogs), 0, "the dialog never shows");

        // The undeclared rule still fires first, without the dialog.
        let (backend, dialogs) = bridged(|script| {
            script.target = LAST_LEGACY_TARGET;
            script.declared = false;
        });
        assert_eq!(
            backend.request(Permission::Notifications).await,
            Ok(Status::PermanentlyDenied)
        );
        assert_eq!(started(&dialogs), 0);
    }

    #[tokio::test]
    async fn android_undeclared_kind_never_prompts() {
        let (backend, dialogs) = bridged(|script| script.declared = false);
        assert_eq!(
            backend.check(Permission::Camera).await,
            Ok(Status::PermanentlyDenied)
        );
        assert_eq!(
            backend.request(Permission::Camera).await,
            Ok(Status::PermanentlyDenied)
        );
        assert_eq!(started(&dialogs), 0);
    }

    #[tokio::test]
    async fn android_restricted_surfaces_as_restricted() {
        let (backend, dialogs) = bridged(|script| script.restricted = true);
        assert_eq!(
            backend.check(Permission::Camera).await,
            Ok(Status::Restricted)
        );
        assert_eq!(
            backend.request(Permission::Bluetooth).await,
            Ok(Status::Restricted)
        );
        assert_eq!(started(&dialogs), 0);
    }

    #[tokio::test]
    async fn android_unsupported_kinds_are_coded() {
        let (backend, _dialogs) = bridged(|_| {});
        for kind in [
            Permission::PhotosAddOnly,
            Permission::Reminders,
            Permission::AppTrackingTransparency,
        ] {
            assert_eq!(
                backend.check(kind).await.unwrap_err().code(),
                "ERR_PERMISSIONS_UNSUPPORTED_PLATFORM"
            );
            assert_eq!(
                backend.request(kind).await.unwrap_err().kind(),
                ErrorKind::UnsupportedPlatform
            );
            assert_eq!(
                backend.open_settings(kind).await.unwrap_err().kind(),
                ErrorKind::UnsupportedPlatform
            );
        }
    }

    #[tokio::test]
    async fn android_granted_dialog_maps_to_granted() {
        let (backend, dialogs) = bridged(|_| {});
        assert_eq!(backend.check(Permission::Camera).await, Ok(Status::Granted));
        assert_eq!(
            backend.request(Permission::Notifications).await,
            Ok(Status::Granted)
        );
        assert_eq!(started(&dialogs), 2);
    }

    #[tokio::test]
    async fn android_legacy_notifications_are_granted_at_install() {
        let (backend, dialogs) = bridged(|script| script.api = SDK_WITH_RUNTIME_NOTIFICATIONS - 1);
        assert_eq!(
            backend.check(Permission::Notifications).await,
            Ok(Status::Granted)
        );
        assert_eq!(
            backend.request(Permission::Notifications).await,
            Ok(Status::Granted)
        );
        assert_eq!(
            started(&dialogs),
            0,
            "no runtime notification permission exists to ask for"
        );
    }

    #[tokio::test]
    async fn android_location_denials_read_the_rationale() {
        let (backend, _dialogs) = bridged(|script| {
            script.answer = Status::Denied;
            script.rationale = true;
        });
        assert_eq!(
            backend.request(Permission::LocationAlways).await,
            Ok(Status::Denied)
        );
        assert_eq!(
            backend.request(Permission::LocationWhenInUse).await,
            Ok(Status::Denied)
        );
        assert_eq!(
            backend.request(Permission::Location).await,
            Ok(Status::Denied)
        );

        let (backend, _dialogs) = bridged(|script| {
            script.answer = Status::Denied;
            script.rationale = false;
        });
        assert_eq!(
            backend.request(Permission::LocationAlways).await,
            Ok(Status::PermanentlyDenied)
        );
        assert_eq!(
            backend.request(Permission::LocationWhenInUse).await,
            Ok(Status::PermanentlyDenied)
        );
        // Only notifications and the two split location kinds read the signal: this is the
        // branch where inference would flip the plain kind, and it must not.
        assert_eq!(
            backend.request(Permission::Location).await,
            Ok(Status::Denied)
        );
    }

    #[tokio::test]
    async fn android_camera_denial_stays_denied() {
        // Even a rationale signal gone false leaves an ordinary kind plainly denied.
        let (backend, _dialogs) = bridged(|script| {
            script.answer = Status::Denied;
            script.rationale = false;
        });
        assert_eq!(backend.check(Permission::Camera).await, Ok(Status::Denied));
        assert_eq!(
            backend.request(Permission::Calendar).await,
            Ok(Status::Denied)
        );
    }

    #[tokio::test]
    async fn android_dialog_answers_outside_the_seam_are_backend_errors() {
        let (backend, _dialogs) = bridged(|script| script.answer = Status::Restricted);
        assert_eq!(
            backend.check(Permission::Camera).await.unwrap_err().kind(),
            ErrorKind::Backend
        );
    }

    #[tokio::test]
    async fn android_open_settings_passes_through() {
        let (backend, _dialogs) = bridged(|_| {});
        assert_eq!(backend.open_settings(Permission::Camera).await, Ok(()));
    }

    #[tokio::test]
    async fn android_default_backend_reports_the_missing_host_bridge() {
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
        // Every raw fact answers the same way: none of them may read as a silent denial.
        let raw = NshAndroidRaw;
        assert_eq!(
            raw.declared(Permission::Camera).await.unwrap_err().kind(),
            ErrorKind::BridgeUnavailable
        );
        assert_eq!(
            raw.rationale(Permission::Camera).await.unwrap_err().kind(),
            ErrorKind::BridgeUnavailable
        );
        assert_eq!(
            raw.restricted().await.unwrap_err().kind(),
            ErrorKind::BridgeUnavailable
        );
        assert_eq!(
            raw.api_level().await.unwrap_err().kind(),
            ErrorKind::BridgeUnavailable
        );
        assert_eq!(
            raw.target_sdk().await.unwrap_err().kind(),
            ErrorKind::BridgeUnavailable
        );
        assert_eq!(
            raw.start_dialog(Permission::Camera)
                .await
                .unwrap_err()
                .kind(),
            ErrorKind::BridgeUnavailable
        );
    }
}
