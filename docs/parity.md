# Parity with `permission_handler` and `react-native-permissions`

One row per public API item of Flutter's `permission_handler` 13.0.2 (platform interface 4.4.1) and `react-native-permissions` 5.6.3, both read from `.refs/` on 2026-11-09, and what this library does with it. Section 1 is our own API, the yardstick for every row below.

No CI run has passed for the permissions implementation: the only green run (42565023126) is the initial scaffold commit, before any of this code. Every **Implemented** row below reads *implemented in this checkout; CI run pending*. AGENTS.md: a claim with no passing run is unverified.

Status words:

- **Implemented (CI pending)**: coded in this checkout, with the named test. The platform bridges are pure logic this release: every mapping is tested against a scripted raw seam, and every production seam (the NSH host integration point) answers `ERR_PERMISSIONS_BRIDGE_UNAVAILABLE`, so no device has answered anything yet. A missing bridge is a coded error, never a silent `denied`.
- **Not this release**: out of scope; section 4 says why and what stands in.
- **Not applicable**: no counterpart in our API; the row says why.

Inline Rust tests sit beside the code in `crates/nsh-permissions/src/` (`error.rs`, `native.rs`, `status.rs`, `permission.rs`, `platform/*.rs`); the rest are in `crates/nsh-permissions/tests/`. TypeScript tests are in `packages/nsh-permissions/tests/`; where a name has a TypeScript twin, the twin carries the same name.

## 1. Our API

### The 16 kinds

TypeScript uses the wire string as the name: `check("camera")`. An unknown string is refused with `ERR_PERMISSIONS_INVALID_ARGUMENT` (`unknown_kind_string_is_refused`; `kind_wire_names_round_trip` in Rust).

| Kind | Wire string |
|---|---|
| `Camera` | `camera` |
| `Microphone` | `microphone` |
| `Photos` | `photos` |
| `PhotosAddOnly` | `photosAddOnly` |
| `Location` | `location` |
| `LocationAlways` | `locationAlways` |
| `LocationWhenInUse` | `locationWhenInUse` |
| `Notifications` | `notifications` |
| `Bluetooth` | `bluetooth` |
| `Speech` | `speech` |
| `Calendar` | `calendar` |
| `Contacts` | `contacts` |
| `Reminders` | `reminders` |
| `Motion` | `motion` |
| `MediaLibrary` | `mediaLibrary` |
| `AppTrackingTransparency` | `appTrackingTransparency` |

### The 5 statuses

| Status | Wire string | Meaning |
|---|---|---|
| `Granted` | `granted` | the app may use the permission |
| `Denied` | `denied` | the app may not; asking again can still show the system's prompt |
| `PermanentlyDenied` | `permanentlyDenied` | the app may not, and only the system's settings can change that |
| `Limited` | `limited` | the app may use part of it, such as iOS limited photo access |
| `Restricted` | `restricted` | a policy the user cannot lift, such as MDM or parental controls, holds the permission |

`status_wire_names_round_trip` covers every name both ways.

### The 8 error codes

`Display` writes the code; `PermissionsError::status()` carries the optional platform status on `Backend`.

| Code | Message |
|---|---|
| `ERR_PERMISSIONS_UNSUPPORTED_PLATFORM` | This platform has no such permission. |
| `ERR_PERMISSIONS_INVALID_ARGUMENT` | An unknown kind string or another bad argument. |
| `ERR_PERMISSIONS_BRIDGE_UNAVAILABLE` | The host bridge is missing; this is a setup failure, never a silent denial. |
| `ERR_PERMISSIONS_SERVICE_DISABLED` | A global service the permission needs is off, such as location services. |
| `ERR_PERMISSIONS_IN_PROGRESS` | A request for this kind is already running; one runs at a time. |
| `ERR_PERMISSIONS_CANCELLED` | The caller or the system cancelled the request. |
| `ERR_PERMISSIONS_TIMEOUT` | The deadline passed before the platform answered. |
| `ERR_PERMISSIONS_BACKEND` | The platform's permission service failed. |

`every_kind_has_its_code_and_a_message` (`src/error.rs`, `tests/errors.test.ts`) and `every_kind_reaches_the_caller_with_its_code` (`tests/permissions.rs`, `tests/permissions.test.ts`).

### The facade

| | Rust | TypeScript | Rule |
|---|---|---|---|
| Check | `Permissions::check(kind)` | `check(kind)` | answers a `Status` within the query deadline (10 s default) |
| Request | `Permissions::request(kind)` | `request(kind)` | shows the system prompt where the platform shows one, within the request deadline (30 s default); one request per kind at a time, and a second while one runs answers `ERR_PERMISSIONS_IN_PROGRESS` at once |
| Open settings | `Permissions::open_settings(kind)` | `openSettings(kind)` | opens the system's settings page for the kind; the web has no settings app and answers `ERR_PERMISSIONS_UNSUPPORTED_PLATFORM` |
| Cancel | `Permissions::cancel(kind) -> bool` | `cancel(kind): boolean` | `true` when a request for the kind was running; that request resolves `ERR_PERMISSIONS_CANCELLED` exactly once |
| Construction | `Permissions::new()` selects the platform backend; `with_backend(Arc<dyn PermissionBackend>)`; `with_timeouts(Timeouts)` | `new Permissions()` picks the rust backend under an NSH host and the web backend otherwise; `Permissions.withBackend(...)` for tests | `Timeouts { request: 30 s, query: 10 s }` by default |

The hard rule, tested on both sides: every call resolves exactly once (a status, a coded error, or a cancel) within its deadline (`a_call_resolves_exactly_once`, `deadline_resolves_with_timeout`); a backend that answers twice resolves the caller once; cancel resolves the running request once (`cancel_resolves_with_cancelled`). A scripted backend for tests ships on both sides (`FakeBackend` in Rust, `memoryBackend` in TypeScript).

### The support matrix

A platform that does not serve a kind answers `ERR_PERMISSIONS_UNSUPPORTED_PLATFORM` from check, request and open_settings. `support_matrix_matches_the_spec` asserts every cell, in Rust (`src/permission.rs`) and TypeScript (`tests/permission.test.ts`).

| Kind | apple | android | windows | linux | web |
|---|---|---|---|---|---|
| `Camera` | yes | yes | yes | — | yes |
| `Microphone` | yes | yes | yes | — | yes |
| `Photos` | yes | yes | — | — | — |
| `PhotosAddOnly` | yes | — | — | — | — |
| `Location` | yes | yes | yes | — | yes |
| `LocationAlways` | yes | yes | — | — | — |
| `LocationWhenInUse` | yes | yes | — | — | — |
| `Notifications` | yes | yes | — | — | yes |
| `Bluetooth` | yes | yes | — | — | — |
| `Speech` | yes | yes | — | — | — |
| `Calendar` | yes | yes | — | — | — |
| `Contacts` | yes | yes | — | — | — |
| `Reminders` | yes | — | — | — | — |
| `Motion` | yes | yes | — | — | yes |
| `MediaLibrary` | yes | yes | — | — | — |
| `AppTrackingTransparency` | yes | — | — | — | — |

### The platform rules behind the matrix

- **apple**: the prompt seam is touched at most once per kind: `NotDetermined` plus a request prompts; once the raw answer says denied, check and request answer `PermanentlyDenied` and the seam is not touched. `Limited` is reachable only for photos, photosAddOnly and mediaLibrary. Raw restricted (MDM, parental controls) answers `Restricted`. An unreachable raw seam answers `ERR_PERMISSIONS_BRIDGE_UNAVAILABLE`. (`ios_prompt_once_then_permanent_denial`, `ios_limited_photos_is_distinct`, `ios_restricted_surfaces_as_restricted`, `ios_missing_bridge_is_never_silent_denial`, the last also in `tests/permissions.test.ts`.)
- **android**: an undeclared kind answers `PermanentlyDenied` on check and request and the dialog is never started. On `api_level >= 33`, a raw denied answer for notifications resolves by the rationale flag: rationale true is `Denied` (the user denied without 'don't ask again', so the dialog can still show), rationale false is `PermanentlyDenied` (the dialog will never show); `locationAlways` and `locationWhenInUse` follow the same inference. On a legacy notifications path (`api_level < 33` or `target_sdk <= 32`), check and request resolve the same answer: a declared kind `Granted` (the install-time grant), an undeclared kind `PermanentlyDenied` through the undeclared rule, and the dialog seam is never started. Raw restricted answers `Restricted`. (`android_notification_denial_is_inferred_from_rationale`, `android_target_sdk_32_never_shows_the_dialog`, `android_undeclared_kind_never_prompts`, `android_restricted_surfaces_as_restricted`, `android_unsupported_kinds_are_coded`.)
- **windows**: the privacy toggle off answers `ERR_PERMISSIONS_SERVICE_DISABLED` on check and request; policy locked answers `Restricted`; otherwise the raw answer. Windows itself can surface a per-call consent prompt (`CheckAccess` answering `UserPromptRequired`), but our bridge models a raw-status seam without the dialog this release, so request resolves the same answer as check, the documented adaptation. (`windows_privacy_disabled_reports_service_disabled`, `windows_unsupported_kinds_are_coded`.)
- **linux**: every kind answers `ERR_PERMISSIONS_UNSUPPORTED_PLATFORM`, immediately, on all three calls. (`linux_every_kind_is_coded_unsupported`.)
- **web**: the permissions-query states map straight through; a dismissed notification request answers `Denied` exactly once; `openSettings` answers `ERR_PERMISSIONS_UNSUPPORTED_PLATFORM`. (`web_notification_dismissed_is_denied`, `web_unsupported_kinds_are_coded`, both also in `tests/web.test.ts`.)

## 2. Flutter `permission_handler`

### `Permission` values

All 41 values, in the order `Permission.values` lists them.

| `permission_handler` | Here | Status | Covered by |
|---|---|---|---|
| `calendar` (deprecated) | `Calendar` (adapted: one kind where Flutter is deprecating for a read/write split) | Implemented (CI pending) | `support_matrix_matches_the_spec` |
| `camera` | `Camera` (same name) | Implemented (CI pending) | `support_matrix_matches_the_spec` |
| `contacts` | `Contacts` (same name) | Implemented (CI pending) | `support_matrix_matches_the_spec` |
| `location` | `Location` (same name) | Implemented (CI pending) | `support_matrix_matches_the_spec` |
| `locationAlways` | `LocationAlways` (same name) | Implemented (CI pending) | `support_matrix_matches_the_spec` |
| `locationWhenInUse` | `LocationWhenInUse` (same name) | Implemented (CI pending) | `support_matrix_matches_the_spec` |
| `mediaLibrary` | `MediaLibrary` (same name) | Implemented (CI pending) | `support_matrix_matches_the_spec` |
| `microphone` | `Microphone` (same name) | Implemented (CI pending) | `support_matrix_matches_the_spec` |
| `phone` | none (telephony is outside the 16 kinds) | Not this release | section 4 |
| `photos` | `Photos` (same name) | Implemented (CI pending) | `support_matrix_matches_the_spec`; `ios_limited_photos_is_distinct` |
| `photosAddOnly` | `PhotosAddOnly` (same name; apple only in the matrix) | Implemented (CI pending) | `support_matrix_matches_the_spec` |
| `reminders` | `Reminders` (same name; apple only) | Implemented (CI pending) | `support_matrix_matches_the_spec` |
| `sensors` | `Motion` (adapted: `react-native-permissions`' name; one kind for CoreMotion and the Android body-sensor permissions) | Implemented (CI pending) | `support_matrix_matches_the_spec` |
| `sms` | none (messaging is outside the 16 kinds) | Not this release | section 4 |
| `speech` | `Speech` (same name) | Implemented (CI pending) | `support_matrix_matches_the_spec` |
| `storage` | none (storage is not a runtime permission we model) | Not this release | section 4 |
| `ignoreBatteryOptimizations` | none (a special consent) | Not this release | section 4 |
| `notification` | `Notifications` (adapted: plural, wire string `notifications`; one kind for the plain notification permission) | Implemented (CI pending) | `support_matrix_matches_the_spec`; `android_notification_denial_is_inferred_from_rationale` |
| `accessMediaLocation` | none (a special consent) | Not this release | section 4 |
| `activityRecognition` | `Motion` (adapted: folds in beside `sensors`) | Implemented (CI pending) | `support_matrix_matches_the_spec` |
| `unknown` | none (a sentinel Flutter never lets you request) | Not applicable | — |
| `bluetooth` | `Bluetooth` (same name) | Implemented (CI pending) | `support_matrix_matches_the_spec` |
| `manageExternalStorage` | none (storage is out of scope) | Not this release | section 4 |
| `systemAlertWindow` | none (a special consent, not a runtime permission dialog) | Not this release | section 4 |
| `requestInstallPackages` | none (same) | Not this release | section 4 |
| `appTrackingTransparency` | `AppTrackingTransparency` (same name; apple only) | Implemented (CI pending) | `support_matrix_matches_the_spec` |
| `criticalAlerts` | none (section 4) | Not this release | section 4 |
| `accessNotificationPolicy` | none (do-not-disturb access, a special consent) | Not this release | section 4 |
| `bluetoothScan` | `Bluetooth` (adapted: the Android 12 granular split folds into one kind) | Implemented (CI pending) | `support_matrix_matches_the_spec` |
| `bluetoothAdvertise` | `Bluetooth` (same fold) | Implemented (CI pending) | `support_matrix_matches_the_spec` |
| `bluetoothConnect` | `Bluetooth` (same fold) | Implemented (CI pending) | `support_matrix_matches_the_spec` |
| `nearbyWifiDevices` | none (a special consent) | Not this release | section 4 |
| `videos` | none (media libraries beyond photos are out of scope) | Not this release | section 4 |
| `audio` | none (same) | Not this release | section 4 |
| `scheduleExactAlarm` | none (a special consent) | Not this release | section 4 |
| `sensorsAlways` | none (background sensors, a special consent) | Not this release | section 4 |
| `calendarWriteOnly` | `Calendar` (adapted: the iOS 16 write-only/full split is not modelled) | Implemented (CI pending) | `support_matrix_matches_the_spec` |
| `calendarFullAccess` | `Calendar` (same fold) | Implemented (CI pending) | `support_matrix_matches_the_spec` |
| `assistant` | none (SiriKit, section 4) | Not this release | section 4 |
| `backgroundRefresh` | none (a status read, not a consent) | Not this release | section 4 |
| `accessLocalNetwork` | none (out of scope this release) | Not this release | section 4 |

### `PermissionStatus` values

| `permission_handler` | Here | Status | Covered by |
|---|---|---|---|
| `denied` | `Denied` (`denied`; same name) | Implemented (CI pending) | `status_wire_names_round_trip` |
| `granted` | `Granted` (`granted`; same name) | Implemented (CI pending) | `status_wire_names_round_trip` |
| `restricted` | `Restricted` (`restricted`; same name. MDM and parental controls, surfaced on apple and android) | Implemented (CI pending) | `status_wire_names_round_trip`; `ios_restricted_surfaces_as_restricted` |
| `limited` | `Limited` (`limited`; same name. Reachable only for photos, photosAddOnly and mediaLibrary on apple) | Implemented (CI pending) | `status_wire_names_round_trip`; `ios_limited_photos_is_distinct` |
| `permanentlyDenied` | `PermanentlyDenied` (`permanentlyDenied`; same name. On android inferred from the rationale flag going false after a denial, Android having no permanent-denial API) | Implemented (CI pending) | `status_wire_names_round_trip`; `android_notification_denial_is_inferred_from_rationale` |
| `provisional` | none (not modelled: the five statuses have no provisional value, and no bridge implements or tests a provisional path this release) | Not this release | section 4 |

### Methods and helpers

| `permission_handler` | Here | Status | Covered by |
|---|---|---|---|
| `Permission.status` (the check) | `Permissions::check(kind)` / `Permissions.check(kind)` | Implemented (CI pending) | `every_kind_reaches_the_caller_with_its_code` |
| `Permission.request()` | `request(kind)` (same shape; one request in flight per kind) | Implemented (CI pending) | `second_request_while_one_runs_is_in_progress` |
| `List<Permission>.request()` (batch) | not a method (compose over the single-kind calls; each call resolves once, and the same kind twice answers `ERR_PERMISSIONS_IN_PROGRESS`) | Not applicable | `a_call_resolves_exactly_once` |
| `shouldShowRequestRationale` | not public (the android bridge reads the rationale flag itself to infer `PermanentlyDenied` when the flag goes false) | Not applicable | `android_notification_denial_is_inferred_from_rationale` |
| `onDeniedCallback`, `onGrantedCallback`, `onPermanentlyDeniedCallback`, `onRestrictedCallback`, `onLimitedCallback`, `onProvisionalCallback` | none (ours returns the status or a coded error; the app wires its own reactions) | Not applicable | — |
| `isGranted`, `isDenied`, `isRestricted`, `isLimited`, `isPermanentlyDenied`, `isProvisional` | none as sugar (compare the `Status` value directly) | Not applicable | — |
| `PermissionWithService.serviceStatus`, `ServiceStatus` (`disabled`/`enabled`/`notApplicable`) | no service query (a global service off, such as location services or the Windows privacy toggle, surfaces as `ERR_PERMISSIONS_SERVICE_DISABLED` from check and request; the radio's own state is not queryable) | Not applicable | `windows_privacy_disabled_reports_service_disabled` |
| `isDisabled`, `isEnabled`, `isNotApplicable` (`ServiceStatusGetters`, `FutureServiceStatusGetters`) | none as sugar (there is no service query to sugar over; a global service off surfaces as `ERR_PERMISSIONS_SERVICE_DISABLED`) | Not applicable | `windows_privacy_disabled_reports_service_disabled` |
| `openAppSettings()` | `open_settings(kind)` / `openSettings(kind)` (adapted: keyed by kind; the web refuses; the platform seam is the NSH integration point) | Implemented (CI pending) | `web_unsupported_kinds_are_coded`; the facade once-only tests |
| `Permission.byValue`, `values`, `toString` | `kind_wire_name` / `kind_from_wire`, `isPermission` (TypeScript) (adapted: wire strings, not ints; an unknown string is a coded error) | Implemented (CI pending) | `unknown_kind_string_is_refused`; `kind_wire_names_round_trip` |

## 3. `react-native-permissions`

RN names each permission by a full platform string (`PERMISSIONS.IOS.CAMERA` is `"ios.permission.CAMERA"`). Ours has 16 kinds, so many strings fold into one kind and the rest are out of scope. The Android map has no `POST_NOTIFICATIONS` entry; RN addresses `android.permission.POST_NOTIFICATIONS` through a private constant in `src/methods.android.ts`, and it maps to our `Notifications`.

### `PERMISSIONS.IOS` (18 values)

| RN | Here | Status | Covered by |
|---|---|---|---|
| `APP_TRACKING_TRANSPARENCY` | `AppTrackingTransparency` (same meaning; apple only) | Implemented (CI pending) | `support_matrix_matches_the_spec` |
| `BLUETOOTH` | `Bluetooth` | Implemented (CI pending) | `support_matrix_matches_the_spec` |
| `CALENDARS` | `Calendar` | Implemented (CI pending) | `support_matrix_matches_the_spec` |
| `CALENDARS_WRITE_ONLY` | `Calendar` (adapted: the write-only/full split is not modelled) | Implemented (CI pending) | `support_matrix_matches_the_spec` |
| `CAMERA` | `Camera` | Implemented (CI pending) | `support_matrix_matches_the_spec` |
| `CONTACTS` | `Contacts` | Implemented (CI pending) | `support_matrix_matches_the_spec` |
| `FACE_ID` | none (not a data permission) | Not this release | section 4 |
| `LOCATION_ALWAYS` | `LocationAlways` | Implemented (CI pending) | `support_matrix_matches_the_spec` |
| `LOCATION_WHEN_IN_USE` | `LocationWhenInUse` | Implemented (CI pending) | `support_matrix_matches_the_spec` |
| `MEDIA_LIBRARY` | `MediaLibrary` | Implemented (CI pending) | `support_matrix_matches_the_spec` |
| `MICROPHONE` | `Microphone` | Implemented (CI pending) | `support_matrix_matches_the_spec` |
| `MOTION` | `Motion` (same name) | Implemented (CI pending) | `support_matrix_matches_the_spec` |
| `PHOTO_LIBRARY` | `Photos` (`permission_handler`'s name) | Implemented (CI pending) | `support_matrix_matches_the_spec` |
| `PHOTO_LIBRARY_ADD_ONLY` | `PhotosAddOnly` | Implemented (CI pending) | `support_matrix_matches_the_spec` |
| `REMINDERS` | `Reminders` | Implemented (CI pending) | `support_matrix_matches_the_spec` |
| `SIRI` | none (SiriKit, section 4) | Not this release | section 4 |
| `SPEECH_RECOGNITION` | `Speech` | Implemented (CI pending) | `support_matrix_matches_the_spec` |
| `STOREKIT` | none (a StoreKit entitlement, not a consent) | Not this release | section 4 |

### `PERMISSIONS.ANDROID` (40 values)

| RN | Here | Status | Covered by |
|---|---|---|---|
| `CAMERA` | `Camera` | Implemented (CI pending) | `support_matrix_matches_the_spec` |
| `RECORD_AUDIO` | `Microphone` (Android's name for it) | Implemented (CI pending) | `support_matrix_matches_the_spec` |
| `ACCESS_FINE_LOCATION`, `ACCESS_COARSE_LOCATION` | `Location`, `LocationWhenInUse` (adapted: the fine/coarse split is not modelled) | Implemented (CI pending) | `support_matrix_matches_the_spec` |
| `ACCESS_BACKGROUND_LOCATION` | `LocationAlways` | Implemented (CI pending) | `support_matrix_matches_the_spec` |
| `READ_CALENDAR`, `WRITE_CALENDAR` | `Calendar` | Implemented (CI pending) | `support_matrix_matches_the_spec` |
| `READ_CONTACTS`, `WRITE_CONTACTS` | `Contacts` | Implemented (CI pending) | `support_matrix_matches_the_spec` |
| `BODY_SENSORS`, `ACTIVITY_RECOGNITION` | `Motion` (one kind for both) | Implemented (CI pending) | `support_matrix_matches_the_spec` |
| `BLUETOOTH_SCAN`, `BLUETOOTH_ADVERTISE`, `BLUETOOTH_CONNECT` | `Bluetooth` (the granular Android 12 split folds into one kind) | Implemented (CI pending) | `support_matrix_matches_the_spec` |
| `READ_MEDIA_IMAGES` | `Photos` | Implemented (CI pending) | `support_matrix_matches_the_spec` |
| `READ_MEDIA_VIDEO`, `READ_MEDIA_AUDIO` | none (media libraries beyond photos are out of scope) | Not this release | section 4 |
| `READ_MEDIA_VISUAL_USER_SELECTED` | none (Android 14 partial photo selection is not surfaced; on apple the same user choice answers `Limited`) | Not this release | section 4 |
| `READ_EXTERNAL_STORAGE`, `WRITE_EXTERNAL_STORAGE`, `ACCESS_MEDIA_LOCATION`, `BODY_SENSORS_BACKGROUND`, `NEARBY_WIFI_DEVICES` | none (storage and special consents) | Not this release | section 4 |
| the 17 telephony and messaging strings (`ACCEPT_HANDOVER`, `ADD_VOICEMAIL`, `ANSWER_PHONE_CALLS`, `CALL_PHONE`, `GET_ACCOUNTS`, `PROCESS_OUTGOING_CALLS`, `READ_CALL_LOG`, `READ_PHONE_NUMBERS`, `READ_PHONE_STATE`, `READ_SMS`, `RECEIVE_MMS`, `RECEIVE_SMS`, `RECEIVE_WAP_PUSH`, `SEND_SMS`, `USE_SIP`, `UWB_RANGING`, `WRITE_CALL_LOG`) | none (outside the frozen 16 kinds) | Not this release | section 4 |

### `PERMISSIONS.WINDOWS` (140 values)

| RN | Here | Status | Covered by |
|---|---|---|---|
| `WEBCAM` | `Camera` (Windows' name for it) | Implemented (CI pending) | `support_matrix_matches_the_spec` |
| `MICROPHONE` | `Microphone` | Implemented (CI pending) | `support_matrix_matches_the_spec` |
| `LOCATION` | `Location` | Implemented (CI pending) | `support_matrix_matches_the_spec` |
| the other 137 capability strings | none (RN mirrors Windows' whole packaged-app capability catalog; ours serves the three the matrix names) | Not this release | section 4 |

### `RESULTS`

| RN | Here | Status | Covered by |
|---|---|---|---|
| `UNAVAILABLE` | `ERR_PERMISSIONS_UNSUPPORTED_PLATFORM` (adapted: a kind the platform does not serve is a coded error, not a status) | Implemented (CI pending) | `android_unsupported_kinds_are_coded`; `windows_unsupported_kinds_are_coded`; `linux_every_kind_is_coded_unsupported`; `web_unsupported_kinds_are_coded` |
| `BLOCKED` | `PermanentlyDenied` (`permission_handler`'s name for it) | Implemented (CI pending) | `android_notification_denial_is_inferred_from_rationale` |
| `DENIED` | `Denied` | Implemented (CI pending) | `status_wire_names_round_trip` |
| `GRANTED` | `Granted` | Implemented (CI pending) | `status_wire_names_round_trip` |
| `LIMITED` | `Limited` | Implemented (CI pending) | `ios_limited_photos_is_distinct` |

RN has no `restricted`; ours adds it for Apple MDM and parental controls and Android restricted profiles.

### Methods

| RN | Here | Status | Covered by |
|---|---|---|---|
| `check(permission)` | `check(kind)` | Implemented (CI pending) | `every_kind_reaches_the_caller_with_its_code` |
| `request(permission, rationale?)` | `request(kind)` (adapted: no rationale UI parameter; the system dialog is the only prompt, and the android bridge infers permanent denial from the rationale flag itself) | Implemented (CI pending) | `second_request_while_one_runs_is_in_progress`; `android_notification_denial_is_inferred_from_rationale` |
| `checkMultiple(permissions)` | not a method (compose over check; every call resolves once within its deadline) | Not applicable | `a_call_resolves_exactly_once` |
| `requestMultiple(permissions)` | not a method (same over request; the same kind twice answers `ERR_PERMISSIONS_IN_PROGRESS`) | Not applicable | `second_request_while_one_runs_is_in_progress` |
| `openSettings(type?)` | `openSettings(kind)` (adapted: keyed by kind, not a settings-screen type; the web refuses) | Implemented (CI pending) | `web_unsupported_kinds_are_coded` |
| `checkNotifications()` | `check("notifications")` (adapted: a `Status`, not a settings object) | Implemented (CI pending) | `support_matrix_matches_the_spec` |
| `requestNotifications(options?, rationale?)` | `request("notifications")` (adapted: the alert/badge/sound options are not modelled; a dismissed dialog answers `Denied`; RN's Windows bridge answers request with a check, which matches our no-dialog adaptation there) | Implemented (CI pending) | `web_notification_dismissed_is_denied` |
| `NotificationSettings` | none (the notification permission is one status here) | Not this release | section 4 |
| `LocationAccuracy`, `LocationAccuracyOptions` (`checkLocationAccuracy`, `requestLocationAccuracy`) | none (accuracy is neither queried nor requested) | Not this release | section 4 |

## 4. Covered elsewhere or not now

Everything the references handle that this release intentionally does not, with the reason and what stands in.

| Reference capability | Why not now | What stands in |
|---|---|---|
| The real OS calls behind every bridge | The NSH host integration has not landed; every production seam is coded to answer `ERR_PERMISSIONS_BRIDGE_UNAVAILABLE` rather than guess. Each platform rule above is scripted and coded, and a missing bridge is never a silent `denied` | `ERR_PERMISSIONS_BRIDGE_UNAVAILABLE` |
| RN's detailed notification settings read (`NotificationSettings`: alert, badge, sound, carPlay, criticalAlert, provisional, providesAppSettings, lockScreen, notificationCenter) | The notification permission is one status, not a settings object | `Status` |
| Flutter's `PermissionStatus.provisional` | The five-status model has no provisional value; the apple raw seam has no provisional variant, so no bridge implements or tests a provisional path this release | nothing |
| RN's `LocationAccuracy` (`checkLocationAccuracy`, `requestLocationAccuracy`, `purposeKey`) | Accuracy (full/reduced) is not part of the status model | nothing; the `Location` kinds answer status only |
| RN `canScheduleExactAlarms`, `canUseFullScreenIntent` | Android special consents, outside the runtime-permission model we mirror | nothing |
| RN `openPhotoPicker`, `openContactPicker` | Pickers, not permissions | on apple, limited photo selection shows as `Limited` on `Photos` |
| The storage and media-library family (Flutter `storage`, `manageExternalStorage`, `videos`, `audio`; RN `READ_EXTERNAL_STORAGE`, `WRITE_EXTERNAL_STORAGE`, `READ_MEDIA_VIDEO`, `READ_MEDIA_AUDIO`) | Storage is not one of the 16 kinds; `Photos` covers the photo library only | `Photos` for images; nothing for video and audio |
| Telephony and messaging (Flutter `phone`, `sms`; 17 RN strings) | Outside the frozen 16 kinds | nothing |
| Android special consents (Flutter `systemAlertWindow`, `requestInstallPackages`, `ignoreBatteryOptimizations`, `accessNotificationPolicy`, `scheduleExactAlarm`, `sensorsAlways`; RN `ACCESS_MEDIA_LOCATION`, `BODY_SENSORS_BACKGROUND`, `NEARBY_WIFI_DEVICES`) | Special-case consents outside the runtime-permission model we mirror | nothing |
| Apple extras (Flutter `criticalAlerts`, `assistant`, `backgroundRefresh`; RN `SIRI`, `STOREKIT`, `FACE_ID`) | Out of scope; a Face ID prompt is the biometrics capability's territory, not a data permission | nothing |
| Android 14 partial photo selection (RN `READ_MEDIA_VISUAL_USER_SELECTED`) | Not surfaced on android this release | `Limited` covers the same user choice on apple |
| The granular Android 12 Bluetooth split | One `Bluetooth` kind; the bridge owns which underlying permissions it maps to | `Bluetooth` |
| The iOS 16 calendar split (Flutter `calendarWriteOnly`/`calendarFullAccess`, RN `CALENDARS_WRITE_ONLY`) | One `Calendar` kind; the read/write split is not modelled | `Calendar` |
| Service queries (Flutter `ServiceStatus`, `serviceStatus`) | No separate service method; a global service off surfaces from check and request, and the radio's own state (Bluetooth on/off) is not queryable | `ERR_PERMISSIONS_SERVICE_DISABLED` |
| RN's framework integration surfaces: the `./expo` config plugin, `./mock` (the Jest mock), `./app.plugin.js`, `./scripts/setup.rb` | React Native integration (Expo config, Jest mocking, CocoaPods setup); an NSH crate ships no Expo or CocoaPods layer, and tests script the memory backend instead of a Jest mock | `memoryBackend` (the `./memory` export) for tests |
| Batch requests (Flutter `List<Permission>.request()`; RN `checkMultiple`, `requestMultiple`) | Composition over the single-kind calls; no batch method to get wrong | the facade's per-kind guarantees |
