//! The errors a permissions call reports. Each has a stable code; a message says what to do.

use std::fmt;

/// What went wrong, as a stable code; see [`PermissionsError::code`].
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorKind {
    /// This platform has no such permission this release.
    UnsupportedPlatform,
    /// An unknown kind string or another bad argument.
    InvalidArgument,
    /// The host bridge is missing, as with a missing iOS Podfile macro; never read this as a denial.
    BridgeUnavailable,
    /// A global service the permission needs is off, such as location services or a privacy toggle.
    ServiceDisabled,
    /// A request for this kind is already running; one runs at a time.
    InProgress,
    /// The caller or the system cancelled the request.
    Cancelled,
    /// The deadline passed before the platform answered.
    Timeout,
    /// The platform's permission service failed; `status` holds its own code where it gives one.
    Backend,
}

/// A failed permissions call.
///
/// `Display` writes the stable code; [`PermissionsError::message`] says what to do. Neither holds
/// user data.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{}", .kind.code())]
pub struct PermissionsError {
    kind: ErrorKind,
    status: Option<i64>,
}

impl PermissionsError {
    /// An error of `kind`, with no platform status.
    pub fn new(kind: ErrorKind) -> Self {
        Self { kind, status: None }
    }

    /// The same error, carrying the platform's own status code.
    #[must_use]
    pub fn with_status(mut self, status: i64) -> Self {
        self.status = Some(status);
        self
    }

    /// What went wrong.
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    /// The platform's status code, when the platform gave one.
    pub fn status(&self) -> Option<i64> {
        self.status
    }

    /// The stable code, such as `ERR_PERMISSIONS_CANCELLED`. Apps match on this.
    pub fn code(&self) -> &'static str {
        self.kind.code()
    }

    /// What to do about it, in one sentence.
    pub fn message(&self) -> &'static str {
        self.kind.message()
    }
}

impl ErrorKind {
    /// The stable code of this kind.
    pub fn code(self) -> &'static str {
        match self {
            ErrorKind::UnsupportedPlatform => "ERR_PERMISSIONS_UNSUPPORTED_PLATFORM",
            ErrorKind::InvalidArgument => "ERR_PERMISSIONS_INVALID_ARGUMENT",
            ErrorKind::BridgeUnavailable => "ERR_PERMISSIONS_BRIDGE_UNAVAILABLE",
            ErrorKind::ServiceDisabled => "ERR_PERMISSIONS_SERVICE_DISABLED",
            ErrorKind::InProgress => "ERR_PERMISSIONS_IN_PROGRESS",
            ErrorKind::Cancelled => "ERR_PERMISSIONS_CANCELLED",
            ErrorKind::Timeout => "ERR_PERMISSIONS_TIMEOUT",
            ErrorKind::Backend => "ERR_PERMISSIONS_BACKEND",
        }
    }

    /// What to do about it, in one sentence.
    pub fn message(self) -> &'static str {
        match self {
            ErrorKind::UnsupportedPlatform => "This platform has no such permission.",
            ErrorKind::InvalidArgument => "An unknown kind string or another bad argument.",
            ErrorKind::BridgeUnavailable => {
                "The host bridge is missing; this is a setup failure, never a silent denial."
            }
            ErrorKind::ServiceDisabled => {
                "A global service the permission needs is off, such as location services."
            }
            ErrorKind::InProgress => {
                "A request for this kind is already running; one runs at a time."
            }
            ErrorKind::Cancelled => "The caller or the system cancelled the request.",
            ErrorKind::Timeout => "The deadline passed before the platform answered.",
            ErrorKind::Backend => "The platform's permission service failed.",
        }
    }
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EVERY_KIND: [(ErrorKind, &str); 8] = [
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
    fn every_kind_has_its_code_and_a_message() {
        for (kind, code) in EVERY_KIND {
            let error = PermissionsError::new(kind);
            assert_eq!(error.code(), code);
            assert_eq!(error.to_string(), code, "Display writes the code");
            assert!(!error.kind().message().is_empty());
        }
    }
}
