# Runtime permissions: brief

Check, request and open settings for camera, microphone, photos, location, notifications and more.

## Options

- No Rust crate covers all permissions
- Swift bridge for Photos (PHPicker, limited access), notifications and location
- Mirror permission_handler's status model as a Rust enum
- Reference only: react-native-permissions

## Recommended path

Status enum (granted, denied, permanentlyDenied, limited, restricted). Check, request, then open settings when permanently denied.

## Edge cases to design for

- iOS prompts once; after denial only Settings can change it
- A missing Podfile macro makes iOS report denied silently
- Android 13+ notifications: no permanent-denial API; infer from shouldShowRequestPermissionRationale
- targetSdk 32 or lower never shows the notification dialog
- iOS 14+ limited photo access is distinct from authorized
- MDM and parental controls surface as restricted

## Research status

Research done 2026-11-09 by web search. Versions and licences are unverified until checked on the registry before pinning. Full catalogue: the research notes for the SuperNSH library set.
