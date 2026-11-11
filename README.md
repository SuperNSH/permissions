# nsh-permissions

Check, request and open settings for camera, microphone, photos, location, notifications and more.

Part of the Native Shell (NSH) capability set. Same layout as the other SuperNSH repos: a Rust crate, docs in `docs/`, CI on GitHub Actions, and a web status page on GitHub Pages at https://supernsh.github.io/permissions/.

## Status

**Implemented in this checkout; not verified by CI.** The Rust crate in `crates/nsh-permissions` has the frozen API: 16 permission kinds, 5 statuses, 8 error codes, and the `Permissions` facade (check, request, open_settings, cancel) with the one-request-per-kind and resolve-exactly-once guarantees tested against a scripted backend. The TypeScript package in `packages/nsh-permissions` mirrors it. The platform bridges are pure logic over scripted seams this release: each production seam answers `ERR_PERMISSIONS_BRIDGE_UNAVAILABLE` until the NSH host integration lands, so no device has answered anything. CI (`.github/workflows/ci.yml`) runs on the next push to `master`: `cargo fmt --all -- --check`, clippy and tests on a Linux/macOS/Windows matrix, cross-checks for the iOS-simulator, Android and wasm targets, and `nsh check`, `nsh build` (a warning fails it) and `nsh test` on the TypeScript package; until that run is green, nothing here is verified. Read [docs/brief.md](docs/brief.md) for the approach and [docs/parity.md](docs/parity.md) for the row-by-row API ledger against Flutter's `permission_handler` and `react-native-permissions`.

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
