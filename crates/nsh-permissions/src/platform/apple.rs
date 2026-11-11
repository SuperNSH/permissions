//! The Apple bridge: the system's own answers over a raw seam, mapped to the shared statuses.

use std::{future::ready, sync::Arc};

use crate::{
    backend::{BackendFuture, PermissionBackend},
    error::{ErrorKind, PermissionsError},
    permission::Permission,
    status::Status,
};

/// What Apple's frameworks say about one permission, in their own words.
// Only the host wiring constructs these; until it exists, the scripted tests do.
#[cfg_attr(not(test), expect(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum RawStatus {
    NotDetermined,
    Granted,
    Denied,
    Limited,
    Restricted,
}

/// The seam to Apple's own answers. The bridge maps; the seam talks to the system.
/// It names only the sixteen kinds, all of which apple serves, so no call from here answers
/// `UnsupportedPlatform`; an unknown kind string is rejected upstream as `InvalidArgument`.
trait AppleRaw: Send + Sync {
    /// The system's status for `kind` now, without prompting.
    fn raw_status(
        &self,
        kind: Permission,
    ) -> BackendFuture<'_, Result<RawStatus, PermissionsError>>;

    /// Shows the system's prompt and answers with the status the user chose.
    fn prompt(&self, kind: Permission) -> BackendFuture<'_, Result<RawStatus, PermissionsError>>;

    /// Opens the app's settings page for `kind`.
    fn open_settings(&self, kind: Permission) -> BackendFuture<'_, Result<(), PermissionsError>>;
}

/// Whether `kind` can come back limited. Photo-library access is the only one (iOS 14+).
fn limited_kind(kind: Permission) -> bool {
    matches!(
        kind,
        Permission::Photos | Permission::PhotosAddOnly | Permission::MediaLibrary
    )
}

/// Maps one raw answer to a shared status. An unasked permission is not granted, so it reads denied.
fn map_status(kind: Permission, raw: RawStatus) -> Result<Status, PermissionsError> {
    match raw {
        RawStatus::NotDetermined => Ok(Status::Denied),
        RawStatus::Granted => Ok(Status::Granted),
        // A denial is final on Apple: only Settings changes it.
        RawStatus::Denied => Ok(Status::PermanentlyDenied),
        RawStatus::Limited if limited_kind(kind) => Ok(Status::Limited),
        // Apple never reports limited outside the photo library; a seam that does is failing.
        RawStatus::Limited => Err(PermissionsError::new(ErrorKind::Backend)),
        RawStatus::Restricted => Ok(Status::Restricted),
    }
}

/// The Apple backend: the raw seam's answers under the Apple mapping rules.
struct AppleBridge<R: AppleRaw> {
    raw: R,
}

impl<R: AppleRaw> AppleBridge<R> {
    fn new(raw: R) -> Self {
        Self { raw }
    }
}

impl<R: AppleRaw> PermissionBackend for AppleBridge<R> {
    fn check(&self, kind: Permission) -> BackendFuture<'_, Result<Status, PermissionsError>> {
        Box::pin(async move {
            let raw = self.raw.raw_status(kind).await?;
            map_status(kind, raw)
        })
    }

    fn request(&self, kind: Permission) -> BackendFuture<'_, Result<Status, PermissionsError>> {
        Box::pin(async move {
            let raw = self.raw.raw_status(kind).await?;
            match raw {
                // iOS asks at most once: only a not-determined kind reaches the prompt.
                RawStatus::NotDetermined => {
                    let answer = self.raw.prompt(kind).await?;
                    map_status(kind, answer)
                }
                _ => map_status(kind, raw),
            }
        })
    }

    fn open_settings(&self, kind: Permission) -> BackendFuture<'_, Result<(), PermissionsError>> {
        Box::pin(async move { self.raw.open_settings(kind).await })
    }
}

/// The NSH-host integration point: the seam the host wires to Apple's frameworks. Until it is
/// wired, every call answers `ERR_PERMISSIONS_BRIDGE_UNAVAILABLE` — coded, never a silent denial.
struct NshHostRaw;

impl AppleRaw for NshHostRaw {
    fn raw_status(
        &self,
        _kind: Permission,
    ) -> BackendFuture<'_, Result<RawStatus, PermissionsError>> {
        unavailable()
    }

    fn prompt(&self, _kind: Permission) -> BackendFuture<'_, Result<RawStatus, PermissionsError>> {
        unavailable()
    }

    fn open_settings(&self, _kind: Permission) -> BackendFuture<'_, Result<(), PermissionsError>> {
        unavailable()
    }
}

/// The one answer an unwired seam gives. Resolves at once, so no call hangs waiting for a host.
fn unavailable<T: 'static>() -> BackendFuture<'static, Result<T, PermissionsError>> {
    Box::pin(ready(Err(PermissionsError::new(
        ErrorKind::BridgeUnavailable,
    ))))
}

/// The Apple platform backend.
pub fn default_backend() -> Arc<dyn PermissionBackend> {
    Arc::new(AppleBridge::new(NshHostRaw))
}

#[cfg(test)]
mod tests {
    use std::sync::{Mutex, MutexGuard, PoisonError};

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

    /// A seam whose answers the test scripts, with iOS's memory: a prompt is asked at most once,
    /// and its answer becomes the status. Clones share one state.
    #[derive(Clone)]
    struct Script {
        state: Arc<Mutex<ScriptState>>,
    }

    struct ScriptState {
        status: RawStatus,
        answer: Option<RawStatus>,
        prompts: usize,
        settings_ok: bool,
    }

    impl Script {
        fn new(status: RawStatus) -> Self {
            Self {
                state: Arc::new(Mutex::new(ScriptState {
                    status,
                    answer: Some(RawStatus::Granted),
                    prompts: 0,
                    settings_ok: true,
                })),
            }
        }

        fn lock(&self) -> MutexGuard<'_, ScriptState> {
            self.state.lock().unwrap_or_else(PoisonError::into_inner)
        }

        /// Sets what the next prompt answers; `None` dismisses it, which cancels.
        fn set_answer(&self, answer: Option<RawStatus>) {
            self.lock().answer = answer;
        }

        /// Sets whether opening settings works.
        fn set_settings_ok(&self, ok: bool) {
            self.lock().settings_ok = ok;
        }

        /// How many times the prompt seam was touched.
        fn prompts(&self) -> usize {
            self.lock().prompts
        }
    }

    impl AppleRaw for Script {
        fn raw_status(
            &self,
            _kind: Permission,
        ) -> BackendFuture<'_, Result<RawStatus, PermissionsError>> {
            Box::pin(ready(Ok(self.lock().status)))
        }

        fn prompt(
            &self,
            _kind: Permission,
        ) -> BackendFuture<'_, Result<RawStatus, PermissionsError>> {
            let mut state = self.lock();
            state.prompts += 1;
            let answer = state.answer;
            // The system remembers the user's choice; a dismissal changes nothing.
            if let Some(chosen) = answer {
                state.status = chosen;
            }
            Box::pin(ready(
                answer.ok_or_else(|| PermissionsError::new(ErrorKind::Cancelled)),
            ))
        }

        fn open_settings(
            &self,
            _kind: Permission,
        ) -> BackendFuture<'_, Result<(), PermissionsError>> {
            let ok = self.lock().settings_ok;
            Box::pin(ready(if ok {
                Ok(())
            } else {
                Err(PermissionsError::new(ErrorKind::BridgeUnavailable))
            }))
        }
    }

    #[tokio::test]
    async fn ios_prompt_once_then_permanent_denial() {
        let script = Script::new(RawStatus::NotDetermined);
        script.set_answer(Some(RawStatus::Denied));
        let backend = AppleBridge::new(script.clone());

        assert_eq!(
            backend.request(Permission::Photos).await.unwrap(),
            Status::PermanentlyDenied
        );
        assert_eq!(script.prompts(), 1, "the prompt ran exactly once");

        assert_eq!(
            backend.check(Permission::Photos).await.unwrap(),
            Status::PermanentlyDenied
        );
        assert_eq!(
            backend.request(Permission::Photos).await.unwrap(),
            Status::PermanentlyDenied
        );
        assert_eq!(
            script.prompts(),
            1,
            "after the denial, no call touches the prompt again"
        );
    }

    #[tokio::test]
    async fn ios_missing_bridge_is_never_silent_denial() {
        let backend = default_backend();
        for kind in EVERY_KIND {
            let check = backend.check(kind).await.unwrap_err();
            assert_eq!(check.kind(), ErrorKind::BridgeUnavailable, "{kind:?}");
            assert_eq!(check.code(), "ERR_PERMISSIONS_BRIDGE_UNAVAILABLE");

            let request = backend.request(kind).await.unwrap_err();
            assert_eq!(request.kind(), ErrorKind::BridgeUnavailable, "{kind:?}");

            let settings = backend.open_settings(kind).await.unwrap_err();
            assert_eq!(settings.kind(), ErrorKind::BridgeUnavailable, "{kind:?}");
        }
    }

    #[tokio::test]
    async fn ios_limited_photos_is_distinct() {
        let script = Script::new(RawStatus::Limited);
        let backend = AppleBridge::new(script.clone());
        for kind in [
            Permission::Photos,
            Permission::PhotosAddOnly,
            Permission::MediaLibrary,
        ] {
            assert_eq!(
                backend.check(kind).await.unwrap(),
                Status::Limited,
                "{kind:?}"
            );
        }
        assert_eq!(
            backend.request(Permission::Photos).await.unwrap(),
            Status::Limited
        );
        assert_eq!(script.prompts(), 0, "limited needs no prompt");

        // A prompt can also land on limited, and it stays distinct from granted.
        let answered = Script::new(RawStatus::NotDetermined);
        answered.set_answer(Some(RawStatus::Limited));
        let backend = AppleBridge::new(answered.clone());
        assert_eq!(
            backend.request(Permission::PhotosAddOnly).await.unwrap(),
            Status::Limited
        );
        assert_eq!(answered.prompts(), 1);

        // A kind that cannot be limited is a failing seam, never a misread status.
        let backend = AppleBridge::new(Script::new(RawStatus::Limited));
        assert_eq!(
            backend.check(Permission::Camera).await.unwrap_err().kind(),
            ErrorKind::Backend
        );
    }

    #[tokio::test]
    async fn ios_restricted_surfaces_as_restricted() {
        let script = Script::new(RawStatus::Restricted);
        let backend = AppleBridge::new(script.clone());

        assert_eq!(
            backend.check(Permission::Contacts).await.unwrap(),
            Status::Restricted
        );
        // MDM holds the permission; a prompt cannot lift it, so none is shown.
        assert_eq!(
            backend.request(Permission::Contacts).await.unwrap(),
            Status::Restricted
        );
        assert_eq!(script.prompts(), 0);
    }

    #[tokio::test]
    async fn ios_not_determined_check_reads_denied_and_never_prompts() {
        let script = Script::new(RawStatus::NotDetermined);
        let backend = AppleBridge::new(script.clone());

        assert_eq!(
            backend.check(Permission::Camera).await.unwrap(),
            Status::Denied
        );
        assert_eq!(script.prompts(), 0, "check never prompts");
    }

    #[tokio::test]
    async fn ios_prompt_grant_maps_granted() {
        let script = Script::new(RawStatus::NotDetermined);
        script.set_answer(Some(RawStatus::Granted));
        let backend = AppleBridge::new(script.clone());

        assert_eq!(
            backend.request(Permission::Camera).await.unwrap(),
            Status::Granted
        );
        assert_eq!(script.prompts(), 1);
        // The system remembers the grant: a later check sees it without prompting.
        assert_eq!(
            backend.check(Permission::Camera).await.unwrap(),
            Status::Granted
        );
        assert_eq!(script.prompts(), 1);
    }

    #[tokio::test]
    async fn ios_prompt_error_propagates() {
        let script = Script::new(RawStatus::NotDetermined);
        script.set_answer(None);
        let backend = AppleBridge::new(script);

        let error = backend.request(Permission::Speech).await.unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Cancelled);
        assert_eq!(error.code(), "ERR_PERMISSIONS_CANCELLED");
    }

    #[tokio::test]
    async fn ios_open_settings_goes_through_the_seam() {
        let script = Script::new(RawStatus::Granted);
        let backend = AppleBridge::new(script.clone());

        backend.open_settings(Permission::Camera).await.unwrap();
        script.set_settings_ok(false);
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
