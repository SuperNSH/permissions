//! The Windows bridge: the privacy settings' answers, mapped to [`PermissionBackend`].
//!
//! Windows governs these kinds with per-app privacy toggles, not per-call consent dialogs, so
//! `request` resolves the same answer as `check`: only the settings app can change it.

use std::{future::ready, sync::Arc};

use crate::{
    backend::{BackendFuture, PermissionBackend},
    error::{ErrorKind, PermissionsError},
    permission::{Permission, Platform, supports},
    status::Status,
};

/// What the Windows privacy settings say about one kind, in their own words. There are two
/// answers — the app may use it or may not; no prompt, no partial grant, comes from the system.
// Only the host wiring constructs these; until it exists, the scripted tests do.
#[cfg_attr(not(test), expect(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum RawStatus {
    Granted,
    Denied,
}

/// Windows' own answers for one kind, as the NSH host reads them from the privacy settings.
/// Only the system's own state; every mapping rule lives in [`WindowsBridge`].
trait WindowsRaw: Send + Sync {
    /// Whether the global privacy toggle for `kind` is on.
    fn privacy_enabled(
        &self,
        kind: Permission,
    ) -> BackendFuture<'_, Result<bool, PermissionsError>>;

    /// Whether a policy the user cannot lift, such as MDM, holds `kind`.
    fn policy_locked(&self, kind: Permission) -> BackendFuture<'_, Result<bool, PermissionsError>>;

    /// The setting's own answer for `kind`: granted or denied.
    fn raw_status(
        &self,
        kind: Permission,
    ) -> BackendFuture<'_, Result<RawStatus, PermissionsError>>;
}

/// The bridge over a [`WindowsRaw`]: the windows row of the matrix, mapped rule for rule.
struct WindowsBridge<R: WindowsRaw> {
    raw: R,
}

impl<R: WindowsRaw> WindowsBridge<R> {
    fn new(raw: R) -> Self {
        Self { raw }
    }

    /// The one answer both `check` and `request` give; the rules in their frozen order.
    async fn answer(&self, kind: Permission) -> Result<Status, PermissionsError> {
        if !supports(Platform::Windows, kind) {
            return Err(PermissionsError::new(ErrorKind::UnsupportedPlatform));
        }
        // A toggle turned off disables the service for every app, before any policy is read.
        if !self.raw.privacy_enabled(kind).await? {
            return Err(PermissionsError::new(ErrorKind::ServiceDisabled));
        }
        if self.raw.policy_locked(kind).await? {
            return Ok(Status::Restricted);
        }
        match self.raw.raw_status(kind).await? {
            RawStatus::Granted => Ok(Status::Granted),
            RawStatus::Denied => Ok(Status::Denied),
        }
    }
}

impl<R: WindowsRaw> PermissionBackend for WindowsBridge<R> {
    fn check(&self, kind: Permission) -> BackendFuture<'_, Result<Status, PermissionsError>> {
        Box::pin(async move { self.answer(kind).await })
    }

    fn request(&self, kind: Permission) -> BackendFuture<'_, Result<Status, PermissionsError>> {
        // No per-call consent dialog exists for these kinds; request asks what check asks.
        Box::pin(async move { self.answer(kind).await })
    }

    fn open_settings(&self, kind: Permission) -> BackendFuture<'_, Result<(), PermissionsError>> {
        Box::pin(ready(if supports(Platform::Windows, kind) {
            // Windows has a privacy settings page, but only the NSH host can open it.
            Err(PermissionsError::new(ErrorKind::BridgeUnavailable))
        } else {
            Err(PermissionsError::new(ErrorKind::UnsupportedPlatform))
        }))
    }
}

/// The production seam, and this release's NSH-host integration point: the host wires the Windows
/// privacy settings here. Until it does, every call is a coded `BridgeUnavailable` — a missing
/// bridge is never a denial.
struct HostRaw;

impl WindowsRaw for HostRaw {
    fn privacy_enabled(
        &self,
        _kind: Permission,
    ) -> BackendFuture<'_, Result<bool, PermissionsError>> {
        unavailable()
    }

    fn policy_locked(
        &self,
        _kind: Permission,
    ) -> BackendFuture<'_, Result<bool, PermissionsError>> {
        unavailable()
    }

    fn raw_status(
        &self,
        _kind: Permission,
    ) -> BackendFuture<'_, Result<RawStatus, PermissionsError>> {
        unavailable()
    }
}

fn unavailable<T: 'static>() -> BackendFuture<'static, Result<T, PermissionsError>> {
    Box::pin(ready(Err(PermissionsError::new(
        ErrorKind::BridgeUnavailable,
    ))))
}

/// The Windows platform backend.
pub fn default_backend() -> Arc<dyn PermissionBackend> {
    Arc::new(WindowsBridge::new(HostRaw))
}

#[cfg(test)]
mod tests {
    use super::*;
    use Permission::*;
    use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

    /// The Windows row of the support matrix.
    const WINDOWS_KINDS: [Permission; 3] = [Camera, Microphone, Location];

    /// A raw whose answers the test scripts, counting calls so a test can prove which rules ran.
    /// Clones share one state.
    #[derive(Clone)]
    struct Scripted {
        state: Arc<Mutex<ScriptedState>>,
    }

    struct ScriptedState {
        privacy_enabled: Result<bool, PermissionsError>,
        policy_locked: Result<bool, PermissionsError>,
        status: Result<RawStatus, PermissionsError>,
        privacy_calls: usize,
        policy_calls: usize,
        status_calls: usize,
    }

    impl Scripted {
        fn new(
            privacy_enabled: Result<bool, PermissionsError>,
            policy_locked: Result<bool, PermissionsError>,
            status: Result<RawStatus, PermissionsError>,
        ) -> Self {
            Self {
                state: Arc::new(Mutex::new(ScriptedState {
                    privacy_enabled,
                    policy_locked,
                    status,
                    privacy_calls: 0,
                    policy_calls: 0,
                    status_calls: 0,
                })),
            }
        }

        fn lock(&self) -> MutexGuard<'_, ScriptedState> {
            self.state.lock().unwrap_or_else(PoisonError::into_inner)
        }

        fn privacy_calls(&self) -> usize {
            self.lock().privacy_calls
        }

        fn policy_calls(&self) -> usize {
            self.lock().policy_calls
        }

        fn status_calls(&self) -> usize {
            self.lock().status_calls
        }
    }

    impl WindowsRaw for Scripted {
        fn privacy_enabled(
            &self,
            _kind: Permission,
        ) -> BackendFuture<'_, Result<bool, PermissionsError>> {
            let mut state = self.lock();
            state.privacy_calls += 1;
            let answer = state.privacy_enabled.clone();
            Box::pin(ready(answer))
        }

        fn policy_locked(
            &self,
            _kind: Permission,
        ) -> BackendFuture<'_, Result<bool, PermissionsError>> {
            let mut state = self.lock();
            state.policy_calls += 1;
            let answer = state.policy_locked.clone();
            Box::pin(ready(answer))
        }

        fn raw_status(
            &self,
            _kind: Permission,
        ) -> BackendFuture<'_, Result<RawStatus, PermissionsError>> {
            let mut state = self.lock();
            state.status_calls += 1;
            let answer = state.status.clone();
            Box::pin(ready(answer))
        }
    }

    #[tokio::test]
    async fn windows_privacy_disabled_reports_service_disabled() {
        // The privacy toggle off disables the service for every app, on check and on request,
        // before any policy or setting is read.
        let raw = Scripted::new(Ok(false), Ok(true), Ok(RawStatus::Granted));
        let bridge = WindowsBridge::new(raw.clone());
        for kind in WINDOWS_KINDS {
            assert_eq!(
                bridge.check(kind).await.unwrap_err().kind(),
                ErrorKind::ServiceDisabled,
                "{kind:?}"
            );
            assert_eq!(
                bridge.request(kind).await.unwrap_err().kind(),
                ErrorKind::ServiceDisabled,
                "{kind:?}"
            );
        }
        assert_eq!(raw.policy_calls(), 0);
        assert_eq!(raw.status_calls(), 0);
    }

    #[tokio::test]
    async fn windows_unsupported_kinds_are_coded() {
        let raw = Scripted::new(Ok(true), Ok(false), Ok(RawStatus::Granted));
        let bridge = WindowsBridge::new(raw.clone());
        for kind in [
            Photos,
            PhotosAddOnly,
            LocationAlways,
            LocationWhenInUse,
            Notifications,
            Bluetooth,
            Speech,
            Calendar,
            Contacts,
            Reminders,
            Motion,
            MediaLibrary,
            AppTrackingTransparency,
        ] {
            let error = bridge.check(kind).await.unwrap_err();
            assert_eq!(error.kind(), ErrorKind::UnsupportedPlatform, "{kind:?}");
            assert_eq!(error.code(), "ERR_PERMISSIONS_UNSUPPORTED_PLATFORM");
            let error = bridge.request(kind).await.unwrap_err();
            assert_eq!(error.kind(), ErrorKind::UnsupportedPlatform, "{kind:?}");
        }
        // Kinds outside the matrix never reach the system.
        assert_eq!(raw.privacy_calls(), 0);
        assert_eq!(raw.policy_calls(), 0);
        assert_eq!(raw.status_calls(), 0);
    }

    #[tokio::test]
    async fn policy_locked_reports_restricted() {
        // A policy the user cannot lift reads as restricted, never as a plain denial.
        let raw = Scripted::new(Ok(true), Ok(true), Ok(RawStatus::Denied));
        let bridge = WindowsBridge::new(raw.clone());
        for kind in WINDOWS_KINDS {
            assert_eq!(bridge.check(kind).await, Ok(Status::Restricted), "{kind:?}");
        }
        assert_eq!(raw.status_calls(), 0);
    }

    #[tokio::test]
    async fn the_raw_answer_passes_through() {
        // The seam's two-answer vocabulary maps one for one onto the caller's Status; nothing
        // else can cross, because nothing else fits in RawStatus.
        for kind in WINDOWS_KINDS {
            for (raw, status) in [
                (RawStatus::Granted, Status::Granted),
                (RawStatus::Denied, Status::Denied),
            ] {
                let bridge = WindowsBridge::new(Scripted::new(Ok(true), Ok(false), Ok(raw)));
                assert_eq!(bridge.check(kind).await, Ok(status), "{kind:?}/{raw:?}");
            }
        }
    }

    #[tokio::test]
    async fn request_answers_what_check_answers() {
        // Windows shows no per-call consent dialog for these kinds: an adaptation of the
        // check/request split, so both calls give the same answer for the same setting.
        for (privacy, locked, status) in [
            (Ok(true), Ok(false), Ok(RawStatus::Granted)),
            (Ok(true), Ok(false), Ok(RawStatus::Denied)),
            (Ok(true), Ok(true), Ok(RawStatus::Denied)),
            (Ok(false), Ok(false), Ok(RawStatus::Granted)),
        ] {
            let bridge = WindowsBridge::new(Scripted::new(privacy, locked, status));
            assert_eq!(bridge.check(Location).await, bridge.request(Location).await);
        }
    }

    #[tokio::test]
    async fn open_settings_needs_the_host_bridge() {
        // Only the NSH host can open the Windows privacy page; until it is wired, say so.
        let bridge = WindowsBridge::new(Scripted::new(Ok(true), Ok(false), Ok(RawStatus::Granted)));
        for kind in WINDOWS_KINDS {
            assert_eq!(
                bridge.open_settings(kind).await.unwrap_err().kind(),
                ErrorKind::BridgeUnavailable,
                "{kind:?}"
            );
        }
        assert_eq!(
            bridge.open_settings(Photos).await.unwrap_err().kind(),
            ErrorKind::UnsupportedPlatform
        );
    }

    #[tokio::test]
    async fn the_host_bridge_reports_itself_missing() {
        // Until the host wires the privacy settings, a missing bridge is a coded setup failure.
        let backend = default_backend();
        assert_eq!(
            backend.check(Permission::Camera).await.unwrap_err().kind(),
            ErrorKind::BridgeUnavailable
        );
        assert_eq!(
            backend
                .request(Permission::Microphone)
                .await
                .unwrap_err()
                .kind(),
            ErrorKind::BridgeUnavailable
        );
        assert_eq!(
            backend
                .open_settings(Permission::Location)
                .await
                .unwrap_err()
                .kind(),
            ErrorKind::BridgeUnavailable
        );
    }

    #[tokio::test]
    async fn seam_errors_propagate_with_their_status() {
        // A failing system query is a coded Backend error, and its own status survives the bridge.
        let error = PermissionsError::new(ErrorKind::Backend).with_status(-2147024894);
        let bridge = WindowsBridge::new(Scripted::new(
            Err(error.clone()),
            Ok(false),
            Ok(RawStatus::Granted),
        ));
        let answered = bridge.check(Permission::Camera).await.unwrap_err();
        assert_eq!(answered.kind(), ErrorKind::Backend);
        assert_eq!(answered.status(), Some(-2147024894));
    }
}
