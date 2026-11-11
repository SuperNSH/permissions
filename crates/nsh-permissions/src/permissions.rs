//! `Permissions`: the calls an app makes — one in-flight request per kind, one resolution per call.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError},
    time::Duration,
};

use nsh_core::element::timer;
use tokio::sync::oneshot;

use crate::{
    backend::{BackendFuture, PermissionBackend},
    error::{ErrorKind, PermissionsError},
    permission::Permission,
    status::Status,
};

/// The deadlines a call runs under, on the engine's clock. Expiry resolves the call
/// `ERR_PERMISSIONS_TIMEOUT`, once, on every target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timeouts {
    /// The longest a `request` runs. A prompt can outlast this on a platform that cannot be
    /// dismissed; the call still resolves once, at its deadline.
    pub request: Duration,
    /// The longest a `check` or an `open_settings` runs.
    pub query: Duration,
}

impl Default for Timeouts {
    fn default() -> Self {
        Self {
            request: Duration::from_secs(30),
            query: Duration::from_secs(10),
        }
    }
}

/// One running request: the cancel signal its owner watches. `None` once a cancel was delivered.
struct Pending {
    cancel: Option<oneshot::Sender<()>>,
}

struct Shared {
    backend: Arc<dyn PermissionBackend>,
    timeouts: Mutex<Timeouts>,
    requests: Mutex<HashMap<Permission, Pending>>,
}

impl Shared {
    fn requests(&self) -> MutexGuard<'_, HashMap<Permission, Pending>> {
        self.requests.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// The permissions facade: check a status, request a prompt, open the app's settings.
///
/// One request per kind runs at a time; a second gets `ERR_PERMISSIONS_IN_PROGRESS`. Every call
/// resolves exactly once — with its answer, a cancel, or a timeout — never hanging. Clones share
/// one backend and one set of gates; [`Permissions::new`] returns the platform's shared handle.
#[derive(Clone)]
pub struct Permissions {
    shared: Arc<Shared>,
}

impl Permissions {
    /// The platform's permissions, with one set of request gates for the whole process.
    pub fn new() -> Self {
        static PLATFORM: OnceLock<Permissions> = OnceLock::new();
        PLATFORM
            .get_or_init(|| Self::with_backend(crate::platform::default_backend()))
            .clone()
    }

    /// Permissions through `backend`, with their own gates. Tests use this with a
    /// [`crate::FakeBackend`].
    pub fn with_backend(backend: Arc<dyn PermissionBackend>) -> Self {
        Self {
            shared: Arc::new(Shared {
                backend,
                timeouts: Mutex::new(Timeouts::default()),
                requests: Mutex::new(HashMap::new()),
            }),
        }
    }

    /// Sets the deadlines every call runs under. Every clone shares them.
    #[must_use]
    pub fn with_timeouts(self, timeouts: Timeouts) -> Self {
        *self
            .shared
            .timeouts
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = timeouts;
        self
    }

    fn timeouts(&self) -> Timeouts {
        *self
            .shared
            .timeouts
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// The status of `kind` right now, without a prompt.
    pub async fn check(&self, kind: Permission) -> Result<Status, PermissionsError> {
        let deadline = self.timeouts().query;
        self.answer_within(deadline, |backend| backend.check(kind))
            .await
    }

    /// Shows the system's prompt for `kind` and waits for the resulting status.
    ///
    /// A second request for the same kind while one runs resolves `ERR_PERMISSIONS_IN_PROGRESS`
    /// immediately. The call resolves exactly once: with the platform's answer, with
    /// `ERR_PERMISSIONS_CANCELLED` after a [`Permissions::cancel`], or with
    /// `ERR_PERMISSIONS_TIMEOUT` at its deadline. Dropping the call's future frees the gate for
    /// the next one.
    pub async fn request(&self, kind: Permission) -> Result<Status, PermissionsError> {
        let (cancel_sender, cancel_receiver) = oneshot::channel::<()>();
        {
            let mut requests = self.shared.requests();
            if requests.contains_key(&kind) {
                return Err(PermissionsError::new(ErrorKind::InProgress));
            }
            requests.insert(
                kind,
                Pending {
                    cancel: Some(cancel_sender),
                },
            );
        }
        let gate = GateFree {
            shared: &self.shared,
            kind,
        };
        let answer = tokio::select! {
            answer = self.shared.backend.request(kind) => answer,
            _ = cancel_receiver => Err(PermissionsError::new(ErrorKind::Cancelled)),
            // nsh-core's timer, not tokio's sleep: the engine's loop wakes it on every target
            // (the web's JS event loop included), so the deadline holds on wasm32 too.
            _ = timer(self.timeouts().request) => Err(PermissionsError::new(ErrorKind::Timeout)),
        };
        drop(gate);
        answer
    }

    /// Opens the app's settings page for `kind`, for a user who denied it permanently.
    pub async fn open_settings(&self, kind: Permission) -> Result<(), PermissionsError> {
        let deadline = self.timeouts().query;
        self.answer_within(deadline, |backend| backend.open_settings(kind))
            .await
    }

    /// Cancels the running request for `kind`; it resolves `ERR_PERMISSIONS_CANCELLED` exactly
    /// once. Returns `true` when a request was running.
    pub fn cancel(&self, kind: Permission) -> bool {
        let mut requests = self.shared.requests();
        match requests.get_mut(&kind) {
            Some(pending) => {
                if let Some(sender) = pending.cancel.take() {
                    let _ = sender.send(());
                }
                true
            }
            None => false,
        }
    }

    /// Runs `call` against the backend, ending it at `deadline` on every target.
    async fn answer_within<T>(
        &self,
        deadline: Duration,
        call: impl FnOnce(&dyn PermissionBackend) -> BackendFuture<'_, Result<T, PermissionsError>>,
    ) -> Result<T, PermissionsError> {
        let future = call(&*self.shared.backend);
        tokio::select! {
            answer = future => answer,
            // nsh-core's timer, not tokio's sleep: the engine's loop wakes it on every target
            // (the web's JS event loop included), so the deadline holds on wasm32 too.
            _ = timer(deadline) => Err(PermissionsError::new(ErrorKind::Timeout)),
        }
    }
}

impl Default for Permissions {
    fn default() -> Self {
        Self::new()
    }
}

/// Frees the kind's request gate when the call goes, however it ends — answered, cancelled,
/// timed out, or dropped mid-flight.
struct GateFree<'a> {
    shared: &'a Shared,
    kind: Permission,
}

impl Drop for GateFree<'_> {
    fn drop(&mut self) {
        self.shared.requests().remove(&self.kind);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_the_frozen_deadlines() {
        let timeouts = Timeouts::default();
        assert_eq!(timeouts.request, Duration::from_secs(30));
        assert_eq!(timeouts.query, Duration::from_secs(10));
    }
}
