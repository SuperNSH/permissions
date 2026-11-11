//! `Permissions` through the fake backend: one request per kind, one resolution per call,
//! cancellation and deadlines.

use std::{
    future::Future,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use nsh_permissions::{
    BackendFuture, ErrorKind, FakeBackend, Permission, PermissionBackend, Permissions,
    PermissionsError, Status, Timeouts,
};

/// Runs `future` on a current-thread runtime inside a `LocalSet`: the facade's futures are not
/// `Send` (the seam's futures are not), so its tests spawn locally.
fn run<F: Future>(future: F) -> F::Output {
    let local = tokio::task::LocalSet::new();
    tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap()
        .block_on(local.run_until(future))
}

/// Yields until `ready` holds, so a spawned call has reached its backend.
async fn settle(ready: impl Fn() -> bool) {
    for _ in 0..1000 {
        if ready() {
            return;
        }
        tokio::task::yield_now().await;
    }
    panic!("the call never reached its backend");
}

/// The deadline tests wait real milliseconds instead of pausing tokio's clock: the facade's
/// timer is nsh-core's, whose off-engine fallback reads real time, and a paused clock would
/// desynchronize the two.
/// Starts a request on its own task, and waits until its prompt is up.
async fn start_request(
    permissions: &Permissions,
    fake: &FakeBackend,
) -> tokio::task::JoinHandle<Result<Status, PermissionsError>> {
    let call = permissions.clone();
    let handle = tokio::task::spawn_local(async move { call.request(Permission::Camera).await });
    settle(|| fake.request_pending(Permission::Camera)).await;
    handle
}

fn spawn_request(
    permissions: &Permissions,
) -> tokio::task::JoinHandle<Result<Status, PermissionsError>> {
    let call = permissions.clone();
    tokio::task::spawn_local(async move { call.request(Permission::Camera).await })
}

const ALL_KINDS: [(ErrorKind, &str); 8] = [
    (
        ErrorKind::UnsupportedPlatform,
        "ERR_PERMISSIONS_UNSUPPORTED_PLATFORM",
    ),
    (
        ErrorKind::InvalidArgument,
        "ERR_PERMISSIONS_INVALID_ARGUMENT",
    ),
    (
        ErrorKind::BridgeUnavailable,
        "ERR_PERMISSIONS_BRIDGE_UNAVAILABLE",
    ),
    (
        ErrorKind::ServiceDisabled,
        "ERR_PERMISSIONS_SERVICE_DISABLED",
    ),
    (ErrorKind::InProgress, "ERR_PERMISSIONS_IN_PROGRESS"),
    (ErrorKind::Cancelled, "ERR_PERMISSIONS_CANCELLED"),
    (ErrorKind::Timeout, "ERR_PERMISSIONS_TIMEOUT"),
    (ErrorKind::Backend, "ERR_PERMISSIONS_BACKEND"),
];

#[test]
fn every_kind_reaches_the_caller_with_its_code() {
    let fake = FakeBackend::new();
    let permissions = Permissions::with_backend(Arc::new(fake.clone()));
    run(async {
        for (kind, code) in ALL_KINDS {
            fake.set_request_answer(Permission::Camera, Err(PermissionsError::new(kind)));
            let error = permissions.request(Permission::Camera).await.unwrap_err();
            assert_eq!(error.code(), code, "{kind:?}");
            assert_eq!(error.kind(), kind);
        }
    });
}

#[test]
fn a_platform_status_is_kept_with_its_error() {
    let fake = FakeBackend::new();
    let permissions = Permissions::with_backend(Arc::new(fake.clone()));
    run(async {
        fake.set_request_answer(
            Permission::Camera,
            Err(PermissionsError::new(ErrorKind::Backend).with_status(-7)),
        );
        let error = permissions.request(Permission::Camera).await.unwrap_err();
        assert_eq!(error.status(), Some(-7));
    });
}

#[test]
fn second_request_while_one_runs_is_in_progress() {
    let fake = FakeBackend::new();
    fake.hold_requests_for_the_test();
    let permissions = Permissions::with_backend(Arc::new(fake.clone()));
    run(async {
        let first = start_request(&permissions, &fake).await;
        let second = permissions.request(Permission::Camera).await.unwrap_err();
        assert_eq!(second.kind(), ErrorKind::InProgress);
        assert_eq!(
            second.code(),
            "ERR_PERMISSIONS_IN_PROGRESS",
            "the second call resolves immediately"
        );
        assert_eq!(fake.requests_started(), 1);
        assert!(fake.finish_request(Permission::Camera, Ok(Status::Granted)));
        assert_eq!(first.await.unwrap(), Ok(Status::Granted));
    });
}

#[test]
fn a_call_resolves_exactly_once() {
    let fake = FakeBackend::new();
    fake.answer_requests_twice();
    fake.set_request_answer(Permission::Camera, Ok(Status::Granted));
    let permissions = Permissions::with_backend(Arc::new(fake.clone()));
    run(async {
        assert_eq!(
            permissions.request(Permission::Camera).await.unwrap(),
            Status::Granted
        );
        assert_eq!(fake.requests_started(), 1);
        // The double answer left no phantom state: the gate freed once, so the next call runs.
        assert_eq!(
            permissions.request(Permission::Camera).await.unwrap(),
            Status::Granted
        );
        assert_eq!(fake.requests_started(), 2);
    });
}

#[test]
fn cancel_resolves_with_cancelled() {
    let fake = FakeBackend::new();
    fake.hold_requests_for_the_test();
    let permissions = Permissions::with_backend(Arc::new(fake.clone()));
    run(async {
        let call = start_request(&permissions, &fake).await;
        assert!(permissions.cancel(Permission::Camera));
        let error = call.await.unwrap().unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Cancelled);
        assert_eq!(error.code(), "ERR_PERMISSIONS_CANCELLED");
        // Nothing runs any more: a second cancel reports false, and the held request is spent.
        assert!(!permissions.cancel(Permission::Camera));
        assert!(!fake.finish_request(Permission::Camera, Ok(Status::Granted)));
    });
}

#[test]
fn deadline_resolves_with_timeout() {
    let fake = FakeBackend::new();
    fake.never_answer_requests();
    let permissions = Permissions::with_backend(Arc::new(fake.clone())).with_timeouts(Timeouts {
        request: Duration::from_millis(50),
        ..Timeouts::default()
    });
    run(async {
        let call = spawn_request(&permissions);
        settle(|| fake.requests_started() == 1).await;
        let error = call.await.unwrap().unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Timeout);
        assert_eq!(error.code(), "ERR_PERMISSIONS_TIMEOUT");
        // The timed-out call freed the gate: the next one starts and is answered at once.
        fake.answer_requests_at_once();
        fake.set_request_answer(Permission::Camera, Ok(Status::Granted));
        let next = spawn_request(&permissions);
        settle(|| fake.requests_started() == 2).await;
        assert_eq!(next.await.unwrap(), Ok(Status::Granted));
    });
}

/// A backend that never answers, to prove the deadlines end the call. The flag says a call arrived.
#[derive(Clone, Default)]
struct SilentBackend {
    polled: Arc<AtomicBool>,
}

impl PermissionBackend for SilentBackend {
    fn check(&self, _kind: Permission) -> BackendFuture<'_, Result<Status, PermissionsError>> {
        let polled = Arc::clone(&self.polled);
        Box::pin(async move {
            polled.store(true, Ordering::SeqCst);
            std::future::pending().await
        })
    }

    fn request(&self, _kind: Permission) -> BackendFuture<'_, Result<Status, PermissionsError>> {
        let polled = Arc::clone(&self.polled);
        Box::pin(async move {
            polled.store(true, Ordering::SeqCst);
            std::future::pending().await
        })
    }

    fn open_settings(&self, _kind: Permission) -> BackendFuture<'_, Result<(), PermissionsError>> {
        let polled = Arc::clone(&self.polled);
        Box::pin(async move {
            polled.store(true, Ordering::SeqCst);
            std::future::pending().await
        })
    }
}

#[test]
fn a_check_resolves_within_its_query_deadline() {
    let backend = SilentBackend::default();
    let permissions =
        Permissions::with_backend(Arc::new(backend.clone())).with_timeouts(Timeouts {
            query: Duration::from_millis(50),
            ..Timeouts::default()
        });
    run(async {
        let call = {
            let permissions = permissions.clone();
            tokio::task::spawn_local(async move { permissions.check(Permission::Camera).await })
        };
        settle(|| backend.polled.load(Ordering::SeqCst)).await;
        let error = call.await.unwrap().unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Timeout);
    });
}

#[test]
fn the_scripted_answers_pass_through() {
    let fake = FakeBackend::new();
    fake.set_check(Permission::Microphone, Ok(Status::Denied));
    fake.set_request_answer(Permission::Microphone, Ok(Status::PermanentlyDenied));
    fake.set_open_settings(
        Permission::Microphone,
        Err(PermissionsError::new(ErrorKind::UnsupportedPlatform)),
    );
    let permissions = Permissions::with_backend(Arc::new(fake.clone()));
    run(async {
        assert_eq!(
            permissions.check(Permission::Microphone).await.unwrap(),
            Status::Denied
        );
        assert_eq!(
            permissions.request(Permission::Microphone).await.unwrap(),
            Status::PermanentlyDenied
        );
        let error = permissions
            .open_settings(Permission::Microphone)
            .await
            .unwrap_err();
        assert_eq!(error.kind(), ErrorKind::UnsupportedPlatform);
    });
}

#[test]
fn a_request_that_fails_to_start_frees_the_gate() {
    let fake = FakeBackend::new();
    fake.fail_next_request(PermissionsError::new(ErrorKind::BridgeUnavailable));
    let permissions = Permissions::with_backend(Arc::new(fake.clone()));
    run(async {
        let error = permissions.request(Permission::Camera).await.unwrap_err();
        assert_eq!(error.kind(), ErrorKind::BridgeUnavailable);
        assert_eq!(fake.requests_started(), 0, "no prompt was started");
        assert_eq!(
            permissions.request(Permission::Camera).await.unwrap(),
            Status::Granted
        );
    });
}

#[test]
fn a_dropped_request_frees_the_gate() {
    let fake = FakeBackend::new();
    fake.hold_requests_for_the_test();
    let permissions = Permissions::with_backend(Arc::new(fake.clone()));
    run(async {
        let call = start_request(&permissions, &fake).await;
        call.abort();
        let _ = call.await;
        assert!(!fake.request_pending(Permission::Camera));
        // The dropped call freed the gate: the next one runs and is answered once.
        let next = spawn_request(&permissions);
        settle(|| fake.request_pending(Permission::Camera)).await;
        assert!(fake.finish_request(Permission::Camera, Ok(Status::Limited)));
        assert_eq!(next.await.unwrap(), Ok(Status::Limited));
    });
}
