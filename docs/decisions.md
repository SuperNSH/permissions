# Decisions

Each entry says what was chosen and why. `docs/goals.md` holds the goals, acceptance criteria and edge cases; `docs/parity.md` lists what each API does on each platform.

## Open

None.

## Decided

### No external permissions crate

Decided 2026-11-09. The brief's research found no Rust crate that covers runtime permissions on all five platforms, so this crate owns the API itself: a `Permissions` facade over the `PermissionBackend` trait, with one bridge per platform that maps that platform's own answers through a raw seam. Flutter's `permission_handler` set the status model and react-native-permissions stays a reference for platform behaviour; neither becomes a dependency.

### Platforms in scope: apple, android, windows, linux, web

Decided 2026-11-09. The first release covers every platform an NSH app ships on, with pure-logic bridges and no OS crates: the real OS bindings are the NSH-host integration point, and until the host wires them the production raw seams resolve `Err(BridgeUnavailable)`, stated honestly. That answer is coded, never panics, never hangs, and never reports a silent `denied`. Linux supports nothing by design: there is no permission broker to talk to, so every kind resolves `ERR_PERMISSIONS_UNSUPPORTED_PLATFORM` at once.

### NSH version: nsh-core 0.5.0

Decided 2026-11-09 on 0.4.0; moved to 0.5.0 on 2026-11-12 with the sibling libraries. Every SuperNSH library pins `nsh-core = "0.5.0"`, with `nsh-engine` and `nsh-testing` 0.5.0 as dev-dependencies where it has them. This crate matches them, so the native module, the headless test driver and the TSX package all line up with the same engine.

### The Android rationale rule was amended

Decided 2026-11-09. The frozen contract's Android row was inverted against both references: after a raw denied answer on API 33+, react-native-permissions maps `shouldShowRequestPermissionRationale` true to `DENIED` and false to `BLOCKED` (`RNPermissionsModuleImpl.kt`), and permission_handler infers the same through `ActivityCompat.shouldShowRequestPermissionRationale` (`PermissionUtils.java`). The corrected rule, now the contract: rationale true resolves `denied` (the user denied without "don't ask again", so the dialog can still show) and rationale false resolves `permanentlyDenied` (the dialog will never show).

### The targetSdk-32 notifications rule was amended

Decided 2026-11-09. The frozen contract's row said a notifications request on targetSdk 32 or lower resolves `denied` without starting the dialog, which contradicted both references and the contract's own legacy-grant row: react-native-permissions keys its pre-Android-13 path on API < 33 (`methods.android.ts`, `USES_LEGACY_NOTIFICATIONS`) and answers from `areNotificationsEnabled()` (`RNPermissionsModuleImpl.kt`), and permission_handler reads the same channel state in `checkNotificationPermissionStatus()` (`PermissionManager.java`). The corrected rule, now the contract: on `target_sdk <= 32` a notifications request resolves the same answer as check (a declared kind resolves `granted`, the legacy install-time grant; an undeclared kind resolves `permanentlyDenied` through the undeclared rule) and the dialog seam is never started. The pre-13 notification-channel state is not modelled, a documented simplification.

### References verified

`docs/brief.md` names the references without URLs. The four URLs came from the task brief and were verified against the clones' git remotes, which match the canonical upstreams exactly, so no canonical-URL correction was needed: `https://github.com/Baseflow/flutter-permission-handler`, `https://github.com/zoontek/react-native-permissions`, `https://github.com/longbridge/nsh`, `https://github.com/SuperNSH/biometrics`.
