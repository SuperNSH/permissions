import type { Permission } from "./permission";
import type { Status } from "./status";

/** What a `Permissions` runs on: the Rust crate, the web's APIs, or a memory backend in tests. */
export interface PermissionsBackend {
  /** The status of `kind` right now, without a prompt. */
  check(kind: Permission): Promise<Status>;
  /** Shows the system's prompt for `kind` and answers with the resulting status. */
  request(kind: Permission): Promise<Status>;
  /** Opens the app's settings page for `kind`. */
  openSettings(kind: Permission): Promise<void>;
}
