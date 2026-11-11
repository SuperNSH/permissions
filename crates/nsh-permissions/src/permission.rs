//! The permission kinds, their wire names, and the frozen platform support matrix.

/// A permission an app can ask for, named as `permission_handler` names it.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Permission {
    Camera,
    Microphone,
    Photos,
    PhotosAddOnly,
    Location,
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
}

/// A platform this release knows the support of.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Platform {
    Apple,
    Android,
    Windows,
    Linux,
    Web,
}

/// Whether `platform` serves `kind` this release. The table is frozen; the TS side asserts the same one.
pub fn supports(platform: Platform, kind: Permission) -> bool {
    match platform {
        Platform::Apple => true,
        Platform::Android => !matches!(
            kind,
            Permission::PhotosAddOnly | Permission::Reminders | Permission::AppTrackingTransparency
        ),
        Platform::Windows => matches!(
            kind,
            Permission::Camera | Permission::Microphone | Permission::Location
        ),
        Platform::Linux => false,
        Platform::Web => matches!(
            kind,
            Permission::Camera
                | Permission::Microphone
                | Permission::Location
                | Permission::Notifications
                | Permission::Motion
        ),
    }
}

/// The wire name of `kind`: the string TSX sends and receives.
pub fn kind_wire_name(kind: Permission) -> &'static str {
    match kind {
        Permission::Camera => "camera",
        Permission::Microphone => "microphone",
        Permission::Photos => "photos",
        Permission::PhotosAddOnly => "photosAddOnly",
        Permission::Location => "location",
        Permission::LocationAlways => "locationAlways",
        Permission::LocationWhenInUse => "locationWhenInUse",
        Permission::Notifications => "notifications",
        Permission::Bluetooth => "bluetooth",
        Permission::Speech => "speech",
        Permission::Calendar => "calendar",
        Permission::Contacts => "contacts",
        Permission::Reminders => "reminders",
        Permission::Motion => "motion",
        Permission::MediaLibrary => "mediaLibrary",
        Permission::AppTrackingTransparency => "appTrackingTransparency",
    }
}

/// The kind a wire name names, or `None` when TSX sent an unknown one.
pub fn kind_from_wire(name: &str) -> Option<Permission> {
    match name {
        "camera" => Some(Permission::Camera),
        "microphone" => Some(Permission::Microphone),
        "photos" => Some(Permission::Photos),
        "photosAddOnly" => Some(Permission::PhotosAddOnly),
        "location" => Some(Permission::Location),
        "locationAlways" => Some(Permission::LocationAlways),
        "locationWhenInUse" => Some(Permission::LocationWhenInUse),
        "notifications" => Some(Permission::Notifications),
        "bluetooth" => Some(Permission::Bluetooth),
        "speech" => Some(Permission::Speech),
        "calendar" => Some(Permission::Calendar),
        "contacts" => Some(Permission::Contacts),
        "reminders" => Some(Permission::Reminders),
        "motion" => Some(Permission::Motion),
        "mediaLibrary" => Some(Permission::MediaLibrary),
        "appTrackingTransparency" => Some(Permission::AppTrackingTransparency),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Permission::*;

    const EVERY_KIND: [Permission; 16] = [
        Camera,
        Microphone,
        Photos,
        PhotosAddOnly,
        Location,
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
    ];

    #[test]
    fn support_matrix_matches_the_spec() {
        let matrix: [(Platform, &[Permission]); 5] = [
            (
                Platform::Apple,
                &[
                    Camera,
                    Microphone,
                    Photos,
                    PhotosAddOnly,
                    Location,
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
                ],
            ),
            (
                Platform::Android,
                &[
                    Camera,
                    Microphone,
                    Photos,
                    Location,
                    LocationAlways,
                    LocationWhenInUse,
                    Notifications,
                    Bluetooth,
                    Speech,
                    Calendar,
                    Contacts,
                    Motion,
                    MediaLibrary,
                ],
            ),
            (Platform::Windows, &[Camera, Microphone, Location]),
            (Platform::Linux, &[]),
            (
                Platform::Web,
                &[Camera, Microphone, Location, Notifications, Motion],
            ),
        ];
        for (platform, supported) in matrix {
            for kind in EVERY_KIND {
                assert_eq!(
                    supports(platform, kind),
                    supported.contains(&kind),
                    "{platform:?}/{kind:?}"
                );
            }
        }
    }

    #[test]
    fn kind_wire_names_round_trip() {
        let names: Vec<&str> = EVERY_KIND.map(kind_wire_name).into_iter().collect();
        assert_eq!(
            names,
            [
                "camera",
                "microphone",
                "photos",
                "photosAddOnly",
                "location",
                "locationAlways",
                "locationWhenInUse",
                "notifications",
                "bluetooth",
                "speech",
                "calendar",
                "contacts",
                "reminders",
                "motion",
                "mediaLibrary",
                "appTrackingTransparency",
            ]
        );
        for kind in EVERY_KIND {
            assert_eq!(kind_from_wire(kind_wire_name(kind)), Some(kind));
        }
        assert_eq!(kind_from_wire("Camera"), None);
        assert_eq!(kind_from_wire(""), None);
    }
}
