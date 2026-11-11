//! The answer to a permissions call, named as `permission_handler` names it.

/// What the platform says about a permission right now.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Status {
    /// The app may use the permission.
    Granted,
    /// The app may not; asking again can still show the system's prompt.
    Denied,
    /// The app may not, and only the system's settings can change that.
    PermanentlyDenied,
    /// The app may use part of it, such as iOS limited photo access.
    Limited,
    /// A policy the user cannot lift, such as MDM or parental controls, holds the permission.
    Restricted,
}

/// The wire name of `status`: the string TSX receives.
pub fn status_wire_name(status: Status) -> &'static str {
    match status {
        Status::Granted => "granted",
        Status::Denied => "denied",
        Status::PermanentlyDenied => "permanentlyDenied",
        Status::Limited => "limited",
        Status::Restricted => "restricted",
    }
}

/// The status a wire name names, or `None` when TSX sent an unknown one.
pub fn status_from_wire(name: &str) -> Option<Status> {
    match name {
        "granted" => Some(Status::Granted),
        "denied" => Some(Status::Denied),
        "permanentlyDenied" => Some(Status::PermanentlyDenied),
        "limited" => Some(Status::Limited),
        "restricted" => Some(Status::Restricted),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EVERY_STATUS: [Status; 5] = [
        Status::Granted,
        Status::Denied,
        Status::PermanentlyDenied,
        Status::Limited,
        Status::Restricted,
    ];

    #[test]
    fn status_wire_names_round_trip() {
        let names: Vec<&str> = EVERY_STATUS.map(status_wire_name).into_iter().collect();
        assert_eq!(
            names,
            [
                "granted",
                "denied",
                "permanentlyDenied",
                "limited",
                "restricted"
            ]
        );
        for status in EVERY_STATUS {
            assert_eq!(status_from_wire(status_wire_name(status)), Some(status));
        }
        assert_eq!(status_from_wire("Granted"), None);
        assert_eq!(status_from_wire(""), None);
    }
}
