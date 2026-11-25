//! Notifications through NSH's own permission calls, the one kind NSH 0.7.1 serves.
//!
//! `notification_permission` and `request_notification_permission` (`nsh-core`'s ambient calls,
//! `element/ambient.rs`) answer on iOS, macOS, Android and the web. NSH has no other permission
//! call, so every other kind goes to the platform bridge underneath.

use std::sync::Arc;

use nsh_core::notifications::NotificationPermission;
use nsh_core::prelude::{notification_permission, request_notification_permission};

use crate::{
    backend::{BackendFuture, PermissionBackend},
    error::{ErrorKind, PermissionsError},
    permission::Permission,
    status::Status,
};

/// Where the notification permission comes from. NSH in production, a script in tests.
trait NotificationSource: Send + Sync {
    /// The permission now, without asking.
    fn permission(&self) -> BackendFuture<'_, NotificationPermission>;

    /// Asks the user once and answers with the permission.
    fn request(&self) -> BackendFuture<'_, NotificationPermission>;
}

/// NSH's ambient calls; they run only on the engine's thread, inside one of its tasks.
struct Nsh;

impl NotificationSource for Nsh {
    fn permission(&self) -> BackendFuture<'_, NotificationPermission> {
        Box::pin(notification_permission())
    }

    fn request(&self) -> BackendFuture<'_, NotificationPermission> {
        Box::pin(request_notification_permission())
    }
}

/// Maps NSH's answer to a status. A permission not asked yet reads denied, as on every bridge
/// here; a refusal is final because NSH says only the system's settings change it.
fn map(permission: NotificationPermission) -> Result<Status, PermissionsError> {
    match permission {
        NotificationPermission::Granted => Ok(Status::Granted),
        NotificationPermission::Default => Ok(Status::Denied),
        NotificationPermission::Denied => Ok(Status::PermanentlyDenied),
        NotificationPermission::Unavailable => {
            Err(PermissionsError::new(ErrorKind::UnsupportedPlatform))
        }
    }
}

/// Serves `Permission::Notifications` from `source` and every other call from `inner`.
struct ViaNotifications<S> {
    inner: Arc<dyn PermissionBackend>,
    source: S,
}

impl<S: NotificationSource> PermissionBackend for ViaNotifications<S> {
    fn check(&self, kind: Permission) -> BackendFuture<'_, Result<Status, PermissionsError>> {
        if kind != Permission::Notifications {
            return self.inner.check(kind);
        }
        Box::pin(async move { map(self.source.permission().await) })
    }

    fn request(&self, kind: Permission) -> BackendFuture<'_, Result<Status, PermissionsError>> {
        if kind != Permission::Notifications {
            return self.inner.request(kind);
        }
        Box::pin(async move { map(self.source.request().await) })
    }

    fn open_settings(&self, kind: Permission) -> BackendFuture<'_, Result<(), PermissionsError>> {
        self.inner.open_settings(kind)
    }
}

/// `inner`, with notifications answered by NSH.
pub(crate) fn via_nsh(inner: Arc<dyn PermissionBackend>) -> Arc<dyn PermissionBackend> {
    Arc::new(ViaNotifications { inner, source: Nsh })
}

#[cfg(test)]
mod tests {
    use std::sync::{Mutex, PoisonError};

    use super::*;
    use crate::fake::FakeBackend;

    /// A source whose answers the test sets, counting how often each call ran.
    struct Script {
        now: Mutex<NotificationPermission>,
        after_prompt: NotificationPermission,
        calls: Mutex<(usize, usize)>,
    }

    impl Script {
        fn new(now: NotificationPermission, after_prompt: NotificationPermission) -> Self {
            Self {
                now: Mutex::new(now),
                after_prompt,
                calls: Mutex::new((0, 0)),
            }
        }

        fn calls(&self) -> (usize, usize) {
            *self.calls.lock().unwrap_or_else(PoisonError::into_inner)
        }
    }

    impl NotificationSource for Arc<Script> {
        fn permission(&self) -> BackendFuture<'_, NotificationPermission> {
            self.calls.lock().unwrap_or_else(PoisonError::into_inner).0 += 1;
            let now = *self.now.lock().unwrap_or_else(PoisonError::into_inner);
            Box::pin(std::future::ready(now))
        }

        fn request(&self) -> BackendFuture<'_, NotificationPermission> {
            self.calls.lock().unwrap_or_else(PoisonError::into_inner).1 += 1;
            *self.now.lock().unwrap_or_else(PoisonError::into_inner) = self.after_prompt;
            Box::pin(std::future::ready(self.after_prompt))
        }
    }

    fn build(script: &Arc<Script>) -> (ViaNotifications<Arc<Script>>, Arc<FakeBackend>) {
        let fake = Arc::new(FakeBackend::new());
        fake.set_check(Permission::Camera, Ok(Status::Limited));
        let bridge = ViaNotifications {
            inner: fake.clone(),
            source: script.clone(),
        };
        (bridge, fake)
    }

    #[test]
    fn nsh_answers_map_to_the_shared_statuses() {
        assert_eq!(map(NotificationPermission::Granted), Ok(Status::Granted));
        assert_eq!(map(NotificationPermission::Default), Ok(Status::Denied));
        assert_eq!(
            map(NotificationPermission::Denied),
            Ok(Status::PermanentlyDenied)
        );
        assert_eq!(
            map(NotificationPermission::Unavailable).unwrap_err().code(),
            "ERR_PERMISSIONS_UNSUPPORTED_PLATFORM"
        );
    }

    #[tokio::test]
    async fn notifications_check_asks_nsh_and_never_prompts() {
        let script = Arc::new(Script::new(
            NotificationPermission::Default,
            NotificationPermission::Granted,
        ));
        let (bridge, fake) = build(&script);
        assert_eq!(
            bridge.check(Permission::Notifications).await,
            Ok(Status::Denied)
        );
        assert_eq!(script.calls(), (1, 0), "a check never prompts");
        assert_eq!(fake.requests_started(), 0);
    }

    #[tokio::test]
    async fn notifications_request_shows_nsh_prompt_and_reports_its_answer() {
        let script = Arc::new(Script::new(
            NotificationPermission::Default,
            NotificationPermission::Granted,
        ));
        let (bridge, _) = build(&script);
        assert_eq!(
            bridge.request(Permission::Notifications).await,
            Ok(Status::Granted)
        );
        assert_eq!(script.calls(), (0, 1));
        assert_eq!(
            bridge.check(Permission::Notifications).await,
            Ok(Status::Granted),
            "the answer sticks"
        );
    }

    #[tokio::test]
    async fn a_refused_notification_prompt_is_permanent() {
        let script = Arc::new(Script::new(
            NotificationPermission::Default,
            NotificationPermission::Denied,
        ));
        let (bridge, _) = build(&script);
        assert_eq!(
            bridge.request(Permission::Notifications).await,
            Ok(Status::PermanentlyDenied)
        );
    }

    #[tokio::test]
    async fn other_kinds_and_settings_go_to_the_platform_bridge() {
        let script = Arc::new(Script::new(
            NotificationPermission::Granted,
            NotificationPermission::Granted,
        ));
        let (bridge, fake) = build(&script);
        assert_eq!(bridge.check(Permission::Camera).await, Ok(Status::Limited));
        let _ = bridge.request(Permission::Camera).await;
        assert_eq!(fake.requests_started(), 1);
        assert!(
            bridge
                .open_settings(Permission::Notifications)
                .await
                .is_ok()
        );
        assert_eq!(
            script.calls(),
            (0, 0),
            "NSH was never asked about the camera"
        );
    }
}
