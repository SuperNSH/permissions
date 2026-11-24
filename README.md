# nsh-permissions

Check, request and open settings for camera, microphone, photos, location, notifications and more.

Part of the Native Shell (NSH) capability set. Same layout as the other SuperNSH repos: a Rust crate, docs in `docs/`, CI on GitHub Actions, and a web status page on GitHub Pages at https://supernsh.github.io/permissions/.

## Install

In the app's directory:

```sh
nsh add nsh-permissions
```

`nsh add` needs NSH main or the nightly: the 0.6.0 and 0.6.1 releases do not have it. In a Rust app, `cargo add nsh-permissions` works with any of them.

TSX code imports from `nsh-permissions`.

## Status

**Implemented and verified by CI.** Run 44376370620 on `master` (https://github.com/SuperNSH/permissions/actions/runs/44376370620) is green on every job: `format`; `rust` on Linux, macOS and Windows (clippy `-D warnings`, `cargo test --all-targets`); `check` for aarch64-apple-ios-sim, aarch64-linux-android and wasm32-unknown-unknown; and `typescript` (`nsh check`, `nsh build` with no warnings, `nsh test`). The Rust crate in `crates/nsh-permissions` has the frozen API: 16 permission kinds, 5 statuses, 8 error codes, and the `Permissions` facade (check, request, open_settings, cancel) with the one-request-per-kind, resolve-exactly-once and deadline guarantees tested against a scripted backend. The TypeScript package in `packages/nsh-permissions` mirrors it, and every canonical test name in [docs/goals.md](docs/goals.md) exists on the side the table assigns it.

**Blocked.** The platform bridges are pure logic over scripted seams: each production seam answers `ERR_PERMISSIONS_BRIDGE_UNAVAILABLE` until the NSH host integration lands, so no real device or OS dialog has answered anything. The one exception is notifications on Apple, Android and the web: NSH 0.6.0 has `notification_permission` and `request_notification_permission`, and `platform/notifications.rs` serves that kind through them. It is tested against a scripted source only, and the new tests postdate the last CI run, so it is unverified until CI runs them. NSH 0.6.0 has no call for any other kind (checked in `nsh-core` `element/ambient.rs`). Device testing, screenshots on a simulator and real-OS permission prompts need that integration plus hardware, and are not claimed. Read [docs/brief.md](docs/brief.md) for the approach and [docs/parity.md](docs/parity.md) for the row-by-row API ledger against Flutter's `permission_handler` and `react-native-permissions`.

## Approach

The status model is `permission_handler`'s: `granted`, `denied`, `permanentlyDenied`, `limited`, `restricted`, as a Rust enum and a TypeScript union with stable wire names. Check, request, then open settings when permanently denied. One request runs per kind at a time, and every call resolves exactly once within its deadline (a status, a coded error, or a cancel). Each platform's rules (iOS prompts once, then only Settings changes it; Android infers permanent denial from the rationale flag; the Windows privacy toggle gates everything) sit behind one `PermissionBackend` trait, scripted in tests, with the production seams waiting on the NSH host integration.

## Layout

- `crates/nsh-permissions`: the Rust crate, one member of the workspace at the repo root.
- `packages/nsh-permissions`: the TypeScript package.
- `docs/`: brief, decisions, goals and the parity ledger.
- `.github/workflows/ci.yml`: the CI gates; `site/` is the status page, published by `.github/workflows/pages.yml`.
- `.zcode/workflow.ts`: the phases this area will run through.

## Checks

Every compile and test runs in GitHub Actions. Nothing is built on a developer machine.

## Licence

MIT. See [LICENSE](LICENSE).

Repository: https://github.com/SuperNSH/permissions
