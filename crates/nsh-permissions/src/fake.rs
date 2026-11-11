//! A backend with scripted answers. Tests use it, and so can an app that runs without a host.

use std::{
    collections::{HashMap, VecDeque},
    future::pending,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};

use tokio::sync::oneshot;

use crate::{
    backend::{BackendFuture, PermissionBackend},
    error::{ErrorKind, PermissionsError},
    permission::Permission,
    status::Status,
};

/// How the fake answers requests: from the script, or one of the staged platform behaviours.
#[derive(Clone, Copy, PartialEq, Eq)]
enum RequestMode {
    /// Answers at once from the per-kind script.
    Scripted,
    /// Never answers; the facade's deadline ends the call.
    Never,
    /// Delivers its answer twice, as a misbehaving host would; only the first lands.
    Twice,
    /// Holds the request until the test finishes it, as a prompt the user has not answered.
    WaitForTest,
}

enum Setup {
    Ready(Result<Status, PermissionsError>),
    Never,
    Twice(Result<Status, PermissionsError>),
    Waiting(oneshot::Receiver<Result<Status, PermissionsError>>),
}

struct FakeState {
    mode: RequestMode,
    fail_next: Option<PermissionsError>,
    checks: HashMap<Permission, Result<Status, PermissionsError>>,
    requests: HashMap<Permission, VecDeque<Result<Status, PermissionsError>>>,
    last_answer: HashMap<Permission, Result<Status, PermissionsError>>,
    settings: HashMap<Permission, Result<(), PermissionsError>>,
    waiting: HashMap<Permission, oneshot::Sender<Result<Status, PermissionsError>>>,
    started: usize,
}

/// A backend whose answers the test scripts, and whose requests the test can hold and finish.
///
/// Clones share one state. A held request is forgotten when its call's future is dropped, as a
/// dropped call never sees the answer.
#[derive(Clone)]
pub struct FakeBackend {
    state: Arc<Mutex<FakeState>>,
}

impl Default for FakeBackend {
    /// A backend that grants everything it is asked.
    fn default() -> Self {
        Self::new()
    }
}

impl FakeBackend {
    /// A backend that answers every check with `Granted` and grants every request.
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(FakeState {
                mode: RequestMode::Scripted,
                fail_next: None,
                checks: HashMap::new(),
                requests: HashMap::new(),
                last_answer: HashMap::new(),
                settings: HashMap::new(),
                waiting: HashMap::new(),
                started: 0,
            })),
        }
    }

    fn lock(&self) -> MutexGuard<'_, FakeState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Sets what `check` answers for `kind`.
    pub fn set_check(&self, kind: Permission, answer: Result<Status, PermissionsError>) {
        self.lock().checks.insert(kind, answer);
    }

    /// Queues the next answer a `request` for `kind` returns; once the queue runs dry, the last
    /// answer repeats.
    pub fn set_request_answer(&self, kind: Permission, answer: Result<Status, PermissionsError>) {
        self.lock()
            .requests
            .entry(kind)
            .or_default()
            .push_back(answer);
    }

    /// Sets what `open_settings` answers for `kind`.
    pub fn set_open_settings(&self, kind: Permission, answer: Result<(), PermissionsError>) {
        self.lock().settings.insert(kind, answer);
    }

    /// Makes every request wait for the test to finish it.
    pub fn hold_requests_for_the_test(&self) {
        self.lock().mode = RequestMode::WaitForTest;
    }

    /// Returns to answering each request at once from the script.
    pub fn answer_requests_at_once(&self) {
        self.lock().mode = RequestMode::Scripted;
    }

    /// Makes every request never answer, as a platform that hangs.
    pub fn never_answer_requests(&self) {
        self.lock().mode = RequestMode::Never;
    }

    /// Makes every request deliver its answer twice, as a misbehaving host.
    pub fn answer_requests_twice(&self) {
        self.lock().mode = RequestMode::Twice;
    }

    /// Makes the next request fail before it starts, as a platform that cannot show a prompt.
    pub fn fail_next_request(&self, error: PermissionsError) {
        self.lock().fail_next = Some(error);
    }

    /// Whether a held request for `kind` waits for the test.
    pub fn request_pending(&self, kind: Permission) -> bool {
        self.lock().waiting.contains_key(&kind)
    }

    /// Answers the held request for `kind`. Returns `false` when none waits.
    pub fn finish_request(
        &self,
        kind: Permission,
        answer: Result<Status, PermissionsError>,
    ) -> bool {
        match self.lock().waiting.remove(&kind) {
            Some(sender) => sender.send(answer).is_ok(),
            None => false,
        }
    }

    /// How many requests were started; one that failed to start does not count.
    pub fn requests_started(&self) -> usize {
        self.lock().started
    }
}

impl PermissionBackend for FakeBackend {
    fn check(&self, kind: Permission) -> BackendFuture<'_, Result<Status, PermissionsError>> {
        let state = Arc::clone(&self.state);
        Box::pin(async move {
            state
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .checks
                .get(&kind)
                .cloned()
                .unwrap_or(Ok(Status::Granted))
        })
    }

    fn request(&self, kind: Permission) -> BackendFuture<'_, Result<Status, PermissionsError>> {
        let state = Arc::clone(&self.state);
        Box::pin(async move {
            let setup = {
                let mut state = state.lock().unwrap_or_else(PoisonError::into_inner);
                if let Some(error) = state.fail_next.take() {
                    return Err(error);
                }
                state.started += 1;
                let answer = next_answer(&mut state, kind);
                match state.mode {
                    RequestMode::Scripted => Setup::Ready(answer),
                    RequestMode::Never => Setup::Never,
                    RequestMode::Twice => Setup::Twice(answer),
                    RequestMode::WaitForTest => {
                        let (sender, receiver) = oneshot::channel();
                        state.waiting.insert(kind, sender);
                        Setup::Waiting(receiver)
                    }
                }
            };
            match setup {
                Setup::Ready(answer) => answer,
                Setup::Never => pending().await,
                Setup::Twice(answer) => {
                    // One slot, two deliveries: the second finds it taken, so the caller sees one answer.
                    let (sender, receiver) = oneshot::channel();
                    let slot = Arc::new(Mutex::new(Some(sender)));
                    for _ in 0..2 {
                        if let Some(sender) =
                            slot.lock().unwrap_or_else(PoisonError::into_inner).take()
                        {
                            let _ = sender.send(answer.clone());
                        }
                    }
                    receiver
                        .await
                        .unwrap_or_else(|_| Err(PermissionsError::new(ErrorKind::Backend)))
                }
                Setup::Waiting(receiver) => {
                    // `_guard` still binds: it forgets the request when this scope goes.
                    let _guard = WaitGuard {
                        state: Arc::clone(&state),
                        kind,
                    };
                    receiver
                        .await
                        .unwrap_or_else(|_| Err(PermissionsError::new(ErrorKind::Backend)))
                    // The guard drops here, forgetting the request if it was never answered.
                }
            }
        })
    }

    fn open_settings(&self, kind: Permission) -> BackendFuture<'_, Result<(), PermissionsError>> {
        let state = Arc::clone(&self.state);
        Box::pin(async move {
            state
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .settings
                .get(&kind)
                .cloned()
                .unwrap_or(Ok(()))
        })
    }
}

/// The next scripted answer for `kind`, repeating the last one; an unscripted kind grants.
fn next_answer(state: &mut FakeState, kind: Permission) -> Result<Status, PermissionsError> {
    if let Some(answer) = state.requests.get_mut(&kind).and_then(VecDeque::pop_front) {
        state.last_answer.insert(kind, answer.clone());
        return answer;
    }
    state
        .last_answer
        .get(&kind)
        .cloned()
        .unwrap_or(Ok(Status::Granted))
}

/// Forgets the held request when its future goes, so a dropped call leaves no stale waiter.
struct WaitGuard {
    state: Arc<Mutex<FakeState>>,
    kind: Permission,
}

impl Drop for WaitGuard {
    fn drop(&mut self) {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .waiting
            .remove(&self.kind);
    }
}
