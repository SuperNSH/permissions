//! The Web bridge: the browser's permission answers, mapped to [`PermissionBackend`].
//!
//! `check` maps a raw `Prompt` to `Denied`: the web cannot tell "never asked" from "denied"
//! without prompting, so only `request` can lift a `Prompt`. The web has no settings app, so
//! `open_settings` has no page to open.

use std::{future::ready, sync::Arc};

use crate::{
    backend::{BackendFuture, PermissionBackend},
    error::{ErrorKind, PermissionsError},
    permission::{Permission, Platform, supports},
    status::Status,
};

/// The browser's own answer to one kind: the tri-state of `Notification.permission` and the
/// Permissions API query.
// The variants are the seam's vocabulary: this bridge only matches them, tests construct them
// now and the NSH host will later, so only the non-test build needs the exemption.
#[cfg_attr(not(test), expect(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RawState {
    /// No recorded decision: `Notification.permission === "default"`, or a query state of `prompt`.
    Prompt,
    /// The user or a policy allowed the kind.
    Granted,
    /// The user or a policy refused the kind.
    Denied,
}

/// The web's raw answers, as the NSH host reads them from the browser. Only the browser's own
/// state; every mapping rule lives in [`WebBridge`].
trait WebRaw: Send + Sync {
    /// The recorded decision for `kind`, without a prompt.
    fn query(&self, kind: Permission) -> BackendFuture<'_, Result<RawState, PermissionsError>>;

    /// Prompts; a dismissed dialog answers `Denied`, as `Notification.requestPermission` does.
    fn request(&self, kind: Permission) -> BackendFuture<'_, Result<RawState, PermissionsError>>;
}

/// The bridge over a [`WebRaw`]: the web's five kinds, mapped rule for rule.
struct WebBridge<R: WebRaw> {
    raw: R,
}

impl<R: WebRaw> WebBridge<R> {
    fn new(raw: R) -> Self {
        Self { raw }
    }
}

/// This bridge serves only the web's frozen row of the support matrix, no other kind.
fn unsupported<T>() -> Result<T, PermissionsError> {
    Err(PermissionsError::new(ErrorKind::UnsupportedPlatform))
}

impl<R: WebRaw> PermissionBackend for WebBridge<R> {
    fn check(&self, kind: Permission) -> BackendFuture<'_, Result<Status, PermissionsError>> {
        Box::pin(async move {
            if !supports(Platform::Web, kind) {
                return unsupported();
            }
            // A raw Prompt reads the same as a refusal here; only request can tell them apart.
            match self.raw.query(kind).await? {
                RawState::Granted => Ok(Status::Granted),
                RawState::Denied | RawState::Prompt => Ok(Status::Denied),
            }
        })
    }

    fn request(&self, kind: Permission) -> BackendFuture<'_, Result<Status, PermissionsError>> {
        Box::pin(async move {
            if !supports(Platform::Web, kind) {
                return unsupported();
            }
            // A decided kind answers without prompting again; only a Prompt reaches the prompt.
            match self.raw.query(kind).await? {
                RawState::Granted => Ok(Status::Granted),
                RawState::Denied => Ok(Status::Denied),
                RawState::Prompt => match self.raw.request(kind).await? {
                    RawState::Granted => Ok(Status::Granted),
                    // A dismissed dialog arrives as Denied, resolving the request exactly once.
                    RawState::Denied | RawState::Prompt => Ok(Status::Denied),
                },
            }
        })
    }

    fn open_settings(&self, _kind: Permission) -> BackendFuture<'_, Result<(), PermissionsError>> {
        // The web has no settings app; even a supported kind has no page to open.
        Box::pin(ready(unsupported()))
    }
}

/// The production seam, and this release's NSH-host integration point: the host wires the browser
/// APIs here (`navigator.permissions.query`, `Notification.requestPermission`, `getUserMedia`).
/// Until it does, every call is a coded `BridgeUnavailable` — a missing bridge is never a denial.
struct HostRaw;

impl WebRaw for HostRaw {
    fn query(&self, _kind: Permission) -> BackendFuture<'_, Result<RawState, PermissionsError>> {
        unavailable()
    }

    fn request(&self, _kind: Permission) -> BackendFuture<'_, Result<RawState, PermissionsError>> {
        unavailable()
    }
}

fn unavailable<T: 'static>() -> BackendFuture<'static, Result<T, PermissionsError>> {
    Box::pin(ready(Err(PermissionsError::new(
        ErrorKind::BridgeUnavailable,
    ))))
}

/// The Web platform backend.
pub fn default_backend() -> Arc<dyn PermissionBackend> {
    Arc::new(WebBridge::new(HostRaw))
}

#[cfg(test)]
mod tests {
    use super::*;
    use Permission::*;
    use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

    /// The web's frozen row of the support matrix.
    const WEB_KINDS: [Permission; 5] = [Camera, Microphone, Location, Notifications, Motion];

    /// Every kind the matrix leaves off the web's row, the location confusables included.
    const OFF_WEB_KINDS: [Permission; 11] = [
        Photos,
        PhotosAddOnly,
        LocationAlways,
        LocationWhenInUse,
        Bluetooth,
        Speech,
        Calendar,
        Contacts,
        Reminders,
        MediaLibrary,
        AppTrackingTransparency,
    ];

    /// A raw whose answers the test scripts, counting calls so a test can prove the bridge did
    /// not prompt. Clones share one state.
    #[derive(Clone)]
    struct Scripted {
        state: Arc<Mutex<ScriptedState>>,
    }

    struct ScriptedState {
        query: RawState,
        request: RawState,
        queries: usize,
        requests: usize,
    }

    impl Scripted {
        fn new(query: RawState, request: RawState) -> Self {
            Self {
                state: Arc::new(Mutex::new(ScriptedState {
                    query,
                    request,
                    queries: 0,
                    requests: 0,
                })),
            }
        }

        fn lock(&self) -> MutexGuard<'_, ScriptedState> {
            self.state.lock().unwrap_or_else(PoisonError::into_inner)
        }

        fn queries(&self) -> usize {
            self.lock().queries
        }

        fn requests(&self) -> usize {
            self.lock().requests
        }
    }

    impl WebRaw for Scripted {
        fn query(
            &self,
            _kind: Permission,
        ) -> BackendFuture<'_, Result<RawState, PermissionsError>> {
            let mut state = self.lock();
            state.queries += 1;
            let answer = state.query;
            Box::pin(ready(Ok(answer)))
        }

        fn request(
            &self,
            _kind: Permission,
        ) -> BackendFuture<'_, Result<RawState, PermissionsError>> {
            let mut state = self.lock();
            state.requests += 1;
            let answer = state.request;
            Box::pin(ready(Ok(answer)))
        }
    }

    #[tokio::test]
    async fn web_notification_dismissed_is_denied() {
        // The browser answers a dismissed notification dialog as denied, once.
        let raw = Scripted::new(RawState::Prompt, RawState::Denied);
        let bridge = WebBridge::new(raw.clone());
        assert_eq!(
            bridge.request(Permission::Notifications).await,
            Ok(Status::Denied)
        );
        // The dismissed dialog resolved once: one prompt, one answer.
        assert_eq!(raw.requests(), 1);
    }

    #[tokio::test]
    async fn web_unsupported_kinds_are_coded() {
        let raw = Scripted::new(RawState::Granted, RawState::Granted);
        let bridge = WebBridge::new(raw.clone());
        for kind in OFF_WEB_KINDS {
            let error = bridge.check(kind).await.unwrap_err();
            assert_eq!(error.kind(), ErrorKind::UnsupportedPlatform, "{kind:?}");
            assert_eq!(error.code(), "ERR_PERMISSIONS_UNSUPPORTED_PLATFORM");
            let error = bridge.request(kind).await.unwrap_err();
            assert_eq!(error.kind(), ErrorKind::UnsupportedPlatform, "{kind:?}");
            let error = bridge.open_settings(kind).await.unwrap_err();
            assert_eq!(error.kind(), ErrorKind::UnsupportedPlatform, "{kind:?}");
        }
        // Kinds outside the matrix never reach the browser.
        assert_eq!(raw.queries(), 0);
        assert_eq!(raw.requests(), 0);
    }

    #[tokio::test]
    async fn check_maps_the_raw_states() {
        // A raw Prompt reads the same as a refusal at check level, on every web kind.
        for kind in WEB_KINDS {
            for (raw_state, status) in [
                (RawState::Granted, Status::Granted),
                (RawState::Denied, Status::Denied),
                (RawState::Prompt, Status::Denied),
            ] {
                let bridge = WebBridge::new(Scripted::new(raw_state, RawState::Denied));
                assert_eq!(
                    bridge.check(kind).await,
                    Ok(status),
                    "{kind:?}/{raw_state:?}"
                );
            }
        }
    }

    #[tokio::test]
    async fn check_never_prompts() {
        // A passive status query must not flash the browser's dialog.
        let raw = Scripted::new(RawState::Prompt, RawState::Denied);
        let bridge = WebBridge::new(raw.clone());
        assert_eq!(bridge.check(Permission::Camera).await, Ok(Status::Denied));
        // One query answered it; the dialog never ran.
        assert_eq!(raw.queries(), 1);
        assert_eq!(raw.requests(), 0);
    }

    #[tokio::test]
    async fn request_does_not_prompt_a_decided_kind() {
        for (raw_state, status) in [
            (RawState::Granted, Status::Granted),
            (RawState::Denied, Status::Denied),
        ] {
            let raw = Scripted::new(raw_state, RawState::Granted);
            let bridge = WebBridge::new(raw.clone());
            assert_eq!(
                bridge.request(Permission::Location).await,
                Ok(status),
                "{raw_state:?}"
            );
            assert_eq!(raw.requests(), 0, "{raw_state:?}");
        }
    }

    #[tokio::test]
    async fn request_maps_the_users_decision() {
        for (answer, status) in [
            (RawState::Granted, Status::Granted),
            (RawState::Denied, Status::Denied),
        ] {
            let raw = Scripted::new(RawState::Prompt, answer);
            let bridge = WebBridge::new(raw.clone());
            assert_eq!(
                bridge.request(Permission::Microphone).await,
                Ok(status),
                "{answer:?}"
            );
            assert_eq!(raw.requests(), 1, "{answer:?}");
        }
    }

    #[tokio::test]
    async fn web_undecided_request_answer_is_denied() {
        // A browser still reporting no decision after prompting reads as not granted.
        let raw = Scripted::new(RawState::Prompt, RawState::Prompt);
        let bridge = WebBridge::new(raw.clone());
        assert_eq!(bridge.request(Permission::Camera).await, Ok(Status::Denied));
        // The undecided answer resolved once: one prompt, one answer.
        assert_eq!(raw.requests(), 1);
    }

    #[tokio::test]
    async fn open_settings_has_no_page_on_the_web() {
        let bridge = WebBridge::new(Scripted::new(RawState::Granted, RawState::Granted));
        for kind in WEB_KINDS {
            assert_eq!(
                bridge.open_settings(kind).await.unwrap_err().kind(),
                ErrorKind::UnsupportedPlatform,
                "{kind:?}"
            );
        }
    }

    #[tokio::test]
    async fn the_host_bridge_reports_itself_missing() {
        // Until the host wires the browser APIs, a missing bridge is a coded setup failure.
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
    }
}
