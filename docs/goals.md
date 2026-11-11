# Goals: nsh-permissions

## 1. Goals

One API for runtime permissions in Native Shell apps, called from Rust and from TSX: `check`, `request` and `open_settings` for camera, microphone, photos, location, notifications and eleven more kinds, across Apple, Android, Windows, Linux and the web. The status model mirrors Flutter's `permission_handler`: `granted`, `denied`, `permanentlyDenied`, `limited`, `restricted`. The intended flow is the brief's: check, request, then open settings when the answer is `permanentlyDenied`.

Two guarantees hold on every platform:

- Every call resolves exactly once: a success, a coded failure, or a cancel. Never a hang, never a second answer.
- One request per kind runs at a time; a second concurrent request resolves `ERR_PERMISSIONS_IN_PROGRESS` at once.

Failures carry stable codes, never a silent `denied`. A missing host bridge reports itself as `ERR_PERMISSIONS_BRIDGE_UNAVAILABLE`, and a kind a platform does not have reports `ERR_PERMISSIONS_UNSUPPORTED_PLATFORM` instead of pretending a dialog could show.

## 2. Acceptance criteria

- [ ] The frozen API surface is implemented in Rust (`crates/nsh-permissions`) and TypeScript (`packages/nsh-permissions`) with identical wire names: the sixteen kind strings and the five status strings are byte-identical on both sides.
- [ ] Every behaviour row has a test under its canonical name below, in both suites where the row applies. The bridge modules compile on every host, so every bridge's tests run on every CI runner.
- [ ] The gates are green in CI, exactly the jobs `.github/workflows/ci.yml` runs: `format` with `cargo fmt --all -- --check`; `rust` on the Linux, macOS and Windows matrix with `cargo clippy --all-targets -- -D warnings` then `cargo test --all-targets`; `cross-check` for the iOS-simulator, Android and wasm targets with `cargo clippy --all-targets --target <target> -- -D warnings`; and `typescript` on `packages/nsh-permissions` with `nsh check`, `nsh build` passing with no warnings, and `nsh test`. A gate with no passing run is pending, not verified; `docs/parity.md` records the current state.
- [ ] No panic on any input: no `unwrap` or `expect` on external data, and a scripted hostile backend gets coded errors out, never a panic.
- [ ] No call hangs past its deadline: requests 30 s and queries 10 s by default (`Timeouts`), enforced by the facade.

### Canonical test names

In both suites:

| Name | Rust | TypeScript |
|---|---|---|
| `every_kind_has_its_code_and_a_message` | inline in `src/error.rs` | `tests/errors.test.ts` |
| `every_kind_reaches_the_caller_with_its_code` | `tests/permissions.rs` | `tests/permissions.test.ts` |
| `unknown_kind_string_is_refused` | inline in `src/native.rs` | `tests/permissions.test.ts` |
| `status_wire_names_round_trip` | inline in `src/status.rs` | `tests/status.test.ts` |
| `support_matrix_matches_the_spec` | inline in `src/permission.rs` | `tests/permission.test.ts` |
| `second_request_while_one_runs_is_in_progress` | `tests/permissions.rs` | `tests/permissions.test.ts` |
| `a_call_resolves_exactly_once` | `tests/permissions.rs` | `tests/memory.test.ts` |
| `cancel_resolves_with_cancelled` | `tests/permissions.rs` | `tests/memory.test.ts` |
| `deadline_resolves_with_timeout` | `tests/permissions.rs` | `tests/memory.test.ts` |
| `ios_missing_bridge_is_never_silent_denial` | inline in `src/platform/apple.rs` | `tests/permissions.test.ts` |
| `web_notification_dismissed_is_denied` | inline in `src/platform/web.rs` | `tests/web.test.ts` |
| `web_unsupported_kinds_are_coded` | inline in `src/platform/web.rs` | `tests/web.test.ts` |

Rust only: the mapping lives in the bridge, and the TypeScript suite runs the memory and web backends, not the native bridges. `ios_prompt_once_then_permanent_denial`, `ios_limited_photos_is_distinct`, `ios_restricted_surfaces_as_restricted` (`src/platform/apple.rs`); `android_notification_denial_is_inferred_from_rationale`, `android_target_sdk_32_never_shows_the_dialog`, `android_undeclared_kind_never_prompts`, `android_restricted_surfaces_as_restricted`, `android_unsupported_kinds_are_coded` (`src/platform/android.rs`); `windows_privacy_disabled_reports_service_disabled`, `windows_unsupported_kinds_are_coded` (`src/platform/windows.rs`); `linux_every_kind_is_coded_unsupported` (`src/platform/linux.rs`).

Rust paths are under `crates/nsh-permissions/`, TypeScript paths under `packages/nsh-permissions/`. The package's `App.tsx` screen, which runs `check("camera")` and shows the resulting status, lands in this same changeset with its `tests/app.test.tsx`; that coverage is CI-pending until a run executes it.

## 3. The error enum

Every failure is a `PermissionsError` with an `ErrorKind`, a stable code and a one-sentence message. `Display` writes the code; apps match on `code()`. `Backend` carries the platform's own status code in `status()` where the platform gives one.

| Kind | Code | Fires when | Example |
|---|---|---|---|
| `UnsupportedPlatform` | `ERR_PERMISSIONS_UNSUPPORTED_PLATFORM` | this platform has no such permission | `photosAddOnly` on Android, which `permission_handler` marks iOS-only; on Linux, every kind |
| `InvalidArgument` | `ERR_PERMISSIONS_INVALID_ARGUMENT` | unknown kind string or a bad argument | a caller sends `"locationalways"`; the wire mapping refuses it instead of guessing |
| `BridgeUnavailable` | `ERR_PERMISSIONS_BRIDGE_UNAVAILABLE` | the host bridge is missing | the missing-iOS-Podfile-macro class of failure: a `permission_handler` app that forgets the `PERMISSION_CAMERA` macro gets a silent `denied`; here the unreachable raw seam reports this code, never a silent denial |
| `ServiceDisabled` | `ERR_PERMISSIONS_SERVICE_DISABLED` | a global service is off | location services turned off in the OS settings, or the Windows camera privacy toggle off |
| `InProgress` | `ERR_PERMISSIONS_IN_PROGRESS` | one request per kind runs at a time | `request("camera")` is waiting on the system prompt and a second `request("camera")` arrives |
| `Cancelled` | `ERR_PERMISSIONS_CANCELLED` | the caller or the system cancelled | the app calls `cancel("camera")` while the prompt is showing; that request resolves `Cancelled` exactly once |
| `Timeout` | `ERR_PERMISSIONS_TIMEOUT` | the deadline passed; resolves once, never hangs | a backend that never answers; the facade resolves at the request deadline (30 s by default) |
| `Backend` | `ERR_PERMISSIONS_BACKEND` | the platform service failed | the OS API returns its own failure and the error keeps its status code in `status()` |

## 4. The support matrix

A `no` cell resolves `ERR_PERMISSIONS_UNSUPPORTED_PLATFORM` at once. Both suites assert this table (`support_matrix_matches_the_spec`).

| Kind | Apple | Android | Windows | Linux | Web |
|---|---|---|---|---|---|
| `camera` | yes | yes | yes | no | yes |
| `microphone` | yes | yes | yes | no | yes |
| `photos` | yes | yes | no | no | no |
| `photosAddOnly` | yes | no | no | no | no |
| `location` | yes | yes | yes | no | yes |
| `locationAlways` | yes | yes | no | no | no |
| `locationWhenInUse` | yes | yes | no | no | no |
| `notifications` | yes | yes | no | no | yes |
| `bluetooth` | yes | yes | no | no | no |
| `speech` | yes | yes | no | no | no |
| `calendar` | yes | yes | no | no | no |
| `contacts` | yes | yes | no | no | no |
| `reminders` | yes | no | no | no | no |
| `motion` | yes | yes | no | no | yes |
| `mediaLibrary` | yes | yes | no | no | no |
| `appTrackingTransparency` | yes | no | no | no | no |

## 5. Edge cases

Each row names the edge case, the platform it belongs to, what this library does, and the canonical tests from section 2. A `—` means no test on that side; the row says why. "All" rows are facade guarantees tested through a scripted backend.

| Edge case | Platform | Our behaviour | Rust test | TS test |
|---|---|---|---|---|
| iOS prompts once; after a denial only Settings can change it | apple | `NotDetermined` plus a request prompts exactly once. Once the raw answer is `Denied`, check and request return `permanentlyDenied` and the prompt seam is never touched again. | `ios_prompt_once_then_permanent_denial` | — (apple bridge logic; the TS suite has no apple backend) |
| A missing Podfile macro must surface as `ERR_PERMISSIONS_BRIDGE_UNAVAILABLE`, never a silent denial | apple | an unreachable raw seam resolves `ERR_PERMISSIONS_BRIDGE_UNAVAILABLE`; the bridge never guesses `denied` | `ios_missing_bridge_is_never_silent_denial` | `ios_missing_bridge_is_never_silent_denial` |
| iOS 14+ limited photo access is distinct from authorized | apple | `limited` is reachable only for `photos`, `photosAddOnly` and `mediaLibrary`; no other kind reports it | `ios_limited_photos_is_distinct` | — (apple bridge logic) |
| MDM and parental controls surface as restricted | apple | a raw `Restricted` answer surfaces as `restricted` on check and request; the user cannot change it from the app | `ios_restricted_surfaces_as_restricted` | — (apple bridge logic) |
| Provisional notification authorization (iOS 12+) has no status here | apple | `permission_handler` carries a sixth status, `provisional`; this status model does not, and the raw seam does not model it. Not surfaced in the first release. | — (the raw seam has no provisional state to script) | — |
| iOS Bluetooth service state is hidden while the permission is denied | apple | `permission_handler` documents that Bluetooth's service status is always disabled when the permission is denied, and that iOS 13.0 misreported every check as denied. This API has no service-status query; bluetooth answers the authorization state only, so neither limitation applies. | — (no service-status surface exists to test) | — |
| iOS 14+ reduced location accuracy has no API here | apple | a user may grant approximate location; react-native-permissions exposes `checkLocationAccuracy` and `requestLocationAccuracy` for it. This release exposes no accuracy API, so location statuses do not distinguish full from reduced access. | — (no accuracy API this release) | — |
| Android 13+ notifications have no permanent-denial API, so denial is inferred from `shouldShowRequestPermissionRationale` | android | on API 33+, after a raw denied answer, rationale true resolves `denied` (the user denied without "don't ask again", so the dialog can still show) and rationale false resolves `permanentlyDenied` (the dialog will never show) | `android_notification_denial_is_inferred_from_rationale` | — (android bridge logic) |
| `check` reports the inferred permanent denial too | android | `permission_handler` returns `permanentlyDenied` only from a request, never a status check. Here the rationale inference answers check and request alike, so a caller learns the dialog is gone without requesting. A deliberate divergence. | `android_notification_denial_is_inferred_from_rationale` | — (android bridge logic) |
| targetSdk 32 or lower never shows the notification dialog | android | a notifications request resolves the same answer as check: a declared kind resolves `granted` (the legacy install-time grant), an undeclared kind resolves `permanentlyDenied` through the undeclared rule, and the dialog seam is never started. The pre-13 reality, where the answer is the notification-channel state (`areNotificationsEnabled()`), is not modelled; that is our documented simplification. | `android_target_sdk_32_never_shows_the_dialog` | — (android bridge logic) |
| Before API 33, a declared notifications kind is granted at install | android | with the kind declared and `api_level` below 33, check returns `granted` (legacy install-time grant) | — (the frozen list names no test for the legacy-grant path) | — |
| An undeclared Android manifest permission can never be granted | android | a kind missing from the manifest resolves `permanentlyDenied` on check and request, and the dialog is never started; Android cannot grant what is not declared | `android_undeclared_kind_never_prompts` | — (android bridge logic) |
| The photos permission changes name with the API level | android | API 32 and lower use `READ_EXTERNAL_STORAGE`, API 33+ uses `READ_MEDIA_IMAGES` (per `permission_handler`'s photos docs). The raw seam's `declared(kind)` hides the name; with neither declared, the undeclared rule above applies. | `android_undeclared_kind_never_prompts` | — (android bridge logic) |
| Android 14 partial photo selection is `limited` in `permission_handler` | android | the dialog seam answers granted or denied only, so `limited` is not reachable on Android in this release; a partial grant arrives as `granted` | — (the seam cannot produce `limited`) | — |
| Background location needs foreground location first (Android 10+) | android | `locationAlways` and `locationWhenInUse` follow the same rationale inference as notifications. Granting foreground before background is the platform's rule, owned by the raw seam. | — (the frozen list names the inference test for notifications only) | — |
| Android 11+ one-time grants are auto-revoked | android | a camera, microphone or location grant the user answered as "only this time" is revoked by the system 30-60 s after the app is terminated or backgrounded, so a `granted` status can go stale; callers re-check before each use | — (no dedicated canonical test this release) | — |
| MDM or work policy restricts a kind | android | `restricted()` true resolves `restricted`; the dialog never starts | `android_restricted_surfaces_as_restricted` | — (android bridge logic) |
| Android bluetooth is a manifest permission, not a runtime dialog | android | `permission_handler` treats bluetooth as install-time on older APIs: no dialog, its private `checkBluetoothPermissionStatus()` (`PermissionManager.java`) reads the manifest declaration and answers denied when undeclared, and the Dart surface is `Permission.bluetooth.status`. This bridge routes bluetooth through the generic declared/rationale path (bluetooth is in the android matrix) and the raw seam answers for it. | — (covered by the generic android tests; no dedicated canonical name) | — |
| Kinds outside the Android matrix refuse at once | android | `photosAddOnly`, `reminders`, `appTrackingTransparency` resolve `ERR_PERMISSIONS_UNSUPPORTED_PLATFORM` | `android_unsupported_kinds_are_coded` | — (android bridge logic) |
| Windows privacy toggles off mean the service is disabled | windows | the privacy toggle off for a kind resolves `ERR_PERMISSIONS_SERVICE_DISABLED` on check and request | `windows_privacy_disabled_reports_service_disabled` | — (windows bridge logic) |
| A locked policy surfaces as restricted | windows | `policy_locked(kind)` true resolves `restricted` | — (the frozen list names no Windows test for the locked-policy path) | — |
| Windows per-call consent prompts are not modelled this release | windows | Windows can surface a per-call prompt: `AppCapability.CheckAccess` can answer `UserPromptRequired`, and react-native-permissions' bridge calls `AppCapability.RequestAccessAsync` for exactly that case. This release's bridge models a raw status seam with no dialog, so `request` resolves the same answer as `check`, and the user changes the permission in Settings, reached through `open_settings`. A documented adaptation, not a platform fact. | — (the frozen list's Windows tests cover the toggle and the unsupported kinds) | — |
| Kinds outside the Windows matrix refuse at once | windows | everything but camera, microphone and location resolves `ERR_PERMISSIONS_UNSUPPORTED_PLATFORM` | `windows_unsupported_kinds_are_coded` | — (windows bridge logic) |
| Linux supports nothing | linux | every kind resolves `ERR_PERMISSIONS_UNSUPPORTED_PLATFORM` at once on check, request and `open_settings`; there is no permission broker to ask in this release | `linux_every_kind_is_coded_unsupported` | — (linux bridge logic) |
| A dismissed web notification prompt resolves denied | web | a dismissed `Notification.requestPermission` dialog answers as denied, and the call resolves exactly once | `web_notification_dismissed_is_denied` | `web_notification_dismissed_is_denied` |
| The web has no settings app | web | `open_settings` resolves `ERR_PERMISSIONS_UNSUPPORTED_PLATFORM` | — (the frozen list's web tests cover kinds and the dismissed dialog; no name covers `open_settings`) | — (same) |
| Kinds outside the web matrix refuse at once | web | everything but camera, microphone, location, notifications and motion resolves `ERR_PERMISSIONS_UNSUPPORTED_PLATFORM` | `web_unsupported_kinds_are_coded` | `web_unsupported_kinds_are_coded` |
| A backend that answers twice resolves the caller once | all | the facade takes the first answer and drops later ones | `a_call_resolves_exactly_once` | `a_call_resolves_exactly_once` |
| A backend that never answers resolves `Timeout` at the deadline | all | the facade resolves `ERR_PERMISSIONS_TIMEOUT` when the deadline passes (30 s request, 10 s query by default) | `deadline_resolves_with_timeout` | `deadline_resolves_with_timeout` |
| A second request while one runs is refused | all | one in-flight request per kind; the second resolves `ERR_PERMISSIONS_IN_PROGRESS` at once | `second_request_while_one_runs_is_in_progress` | `second_request_while_one_runs_is_in_progress` |
| Cancelling resolves the running request once | all | `cancel(kind)` returns true only when a request for that kind is running, and that request resolves `ERR_PERMISSIONS_CANCELLED` exactly once | `cancel_resolves_with_cancelled` | `cancel_resolves_with_cancelled` |
| An unknown kind string is refused | all | a kind string outside the sixteen wire names resolves `ERR_PERMISSIONS_INVALID_ARGUMENT` | `unknown_kind_string_is_refused` | `unknown_kind_string_is_refused` |
