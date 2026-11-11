import type { PermissionsBackend } from "./backend";
import { asPermissionsError, PermissionsError } from "./errors";
import { isPermission, type Permission } from "./permission";
import { rustBackend } from "./rust";
import type { Status } from "./status";
import { webBackend } from "./web";

/** The deadlines a call runs under, in milliseconds. Expiry resolves the call `ERR_PERMISSIONS_TIMEOUT`, once. */
export interface Timeouts {
  /** The longest a `request` runs, `30000` by default. A prompt can outlast this on a platform that cannot be dismissed; the call still resolves once, at its deadline. */
  request: number;
  /** The longest a `check` or an `openSettings` runs, `10000` by default. */
  query: number;
}

const DEFAULT_TIMEOUTS: Timeouts = { request: 30_000, query: 10_000 };

/** What a `Permissions` is built from; every part is optional. */
export interface PermissionsOptions {
  /** The backend the calls run on; defaults to the platform's own. */
  backend?: PermissionsBackend;
  /** Overrides the deadlines; unset ones keep their defaults. */
  timeouts?: Partial<Timeouts>;
}

type Outcome = { ok: true; value: Status | void } | { ok: false; error: PermissionsError };
type Gate = (outcome: Outcome) => void;

/**
 * The process-wide request gates, one per kind: every handle the constructor makes shares them,
 * as all of Rust's `Permissions::new()` clones share the singleton's gates. A request one handle
 * holds, no other handle may start — wherever in the process it was made.
 */
const sharedGates = new Map<Permission, Gate>();

/**
 * The permissions facade: check a status, request a prompt, open the app's settings. One request
 * per kind runs at a time — a second resolves `ERR_PERMISSIONS_IN_PROGRESS` at once — and every
 * call resolves exactly once: with its answer, a cancel, or `ERR_PERMISSIONS_TIMEOUT` at its
 * deadline. `cancel` ends the running request for a kind with `ERR_PERMISSIONS_CANCELLED`.
 * Every handle the constructor makes shares the process-wide request gates, as Rust's
 * `Permissions::new()` clones do; `withBackend` handles gate alone.
 */
export class Permissions {
  private readonly backend: PermissionsBackend;
  private readonly timeouts: Timeouts;
  // Reassigned only by withBackend, which gates alone; every other handle shares the module's.
  private running: Map<Permission, Gate>;

  constructor(options: PermissionsOptions = {}) {
    this.backend = options.backend ?? defaultBackend();
    this.timeouts = { ...DEFAULT_TIMEOUTS, ...options.timeouts };
    this.running = sharedGates;
  }

  /** Permissions through `backend`, for tests; `timeouts` override the deadlines. This handle gates alone, as Rust's `with_backend` builds a fresh instance. */
  static withBackend(backend: PermissionsBackend, timeouts?: Partial<Timeouts>): Permissions {
    const permissions = new Permissions({ backend, timeouts });
    permissions.running = new Map();
    return permissions;
  }

  /** The status of `kind` right now, without a prompt, within the query deadline. */
  check(kind: Permission): Promise<Status> {
    if (!isPermission(kind)) return Promise.reject(new PermissionsError("ERR_PERMISSIONS_INVALID_ARGUMENT"));
    return this.run(kind, this.timeouts.query, () => this.backend.check(kind)) as Promise<Status>;
  }

  /**
   * Shows the system's prompt for `kind` and waits for the resulting status, within the request
   * deadline. A second request for the same kind while one runs resolves
   * `ERR_PERMISSIONS_IN_PROGRESS` immediately.
   */
  request(kind: Permission): Promise<Status> {
    if (!isPermission(kind)) return Promise.reject(new PermissionsError("ERR_PERMISSIONS_INVALID_ARGUMENT"));
    if (this.running.has(kind)) return Promise.reject(new PermissionsError("ERR_PERMISSIONS_IN_PROGRESS"));
    return this.run(kind, this.timeouts.request, () => this.backend.request(kind), true) as Promise<Status>;
  }

  /** Opens the app's settings page for `kind`, within the query deadline. */
  openSettings(kind: Permission): Promise<void> {
    if (!isPermission(kind)) return Promise.reject(new PermissionsError("ERR_PERMISSIONS_INVALID_ARGUMENT"));
    return this.run(kind, this.timeouts.query, () => this.backend.openSettings(kind)) as Promise<void>;
  }

  /** Cancels the running request for `kind`; it resolves `ERR_PERMISSIONS_CANCELLED` exactly once. Returns `true` when a request was running. */
  cancel(kind: Permission): boolean {
    const settle = this.running.get(kind);
    if (settle === undefined) return false;
    this.running.delete(kind);
    settle({ ok: false, error: new PermissionsError("ERR_PERMISSIONS_CANCELLED") });
    return true;
  }

  /** Runs one call: the deadline races the backend, and `track` guards the kind's request gate. */
  private run(
    kind: Permission,
    deadline: number,
    operation: () => Promise<Status> | Promise<void>,
    track = false,
  ): Promise<Status | void> {
    return new Promise((resolve, reject) => {
      let signal: AbortSignal;
      try {
        signal = AbortSignal.timeout(deadline);
      } catch {
        // AbortSignal.timeout refuses a non-finite or negative deadline; that is a bad argument.
        reject(new PermissionsError("ERR_PERMISSIONS_INVALID_ARGUMENT"));
        return;
      }
      let settled = false;
      const settle = (outcome: Outcome) => {
        // First answer wins: a late delivery, a late abort or a late cancel changes nothing.
        if (settled) return;
        settled = true;
        if (this.running.get(kind) === settle) this.running.delete(kind);
        if (outcome.ok) resolve(outcome.value);
        else reject(outcome.error);
      };
      if (track) this.running.set(kind, settle);
      signal.addEventListener("abort", () => settle({ ok: false, error: new PermissionsError("ERR_PERMISSIONS_TIMEOUT") }));
      let call: Promise<Status | void>;
      try {
        call = operation();
      } catch (error) {
        // A backend may throw before returning its promise; that rejects the call like any failure.
        call = Promise.reject(error);
      }
      call.then(
        (value) => settle({ ok: true, value }),
        (error: unknown) => settle({ ok: false, error: asPermissionsError(error) }),
      );
    });
  }
}

/**
 * The backend `new Permissions()` runs on: the Rust crate's, wherever an NSH host loads this
 * bundle — the crate's own web bridge covers the web target — and the web APIs only in a browser
 * without NSH, the one place `navigator.permissions` can exist.
 */
function defaultBackend(): PermissionsBackend {
  const navigator = (globalThis as { navigator?: { permissions?: unknown } }).navigator;
  return navigator !== undefined && navigator.permissions !== undefined ? webBackend() : rustBackend();
}
