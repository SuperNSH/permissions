/** The codes a permissions rejection carries. Apps match on `code`. */
export type PermissionsErrorCode =
  | "ERR_PERMISSIONS_UNSUPPORTED_PLATFORM"
  | "ERR_PERMISSIONS_INVALID_ARGUMENT"
  | "ERR_PERMISSIONS_BRIDGE_UNAVAILABLE"
  | "ERR_PERMISSIONS_SERVICE_DISABLED"
  | "ERR_PERMISSIONS_IN_PROGRESS"
  | "ERR_PERMISSIONS_CANCELLED"
  | "ERR_PERMISSIONS_TIMEOUT"
  | "ERR_PERMISSIONS_BACKEND";

/** Same wording as the Rust crate's messages. None holds the platform's own status. */
const MESSAGES: Record<PermissionsErrorCode, string> = {
  ERR_PERMISSIONS_UNSUPPORTED_PLATFORM: "This platform has no such permission.",
  ERR_PERMISSIONS_INVALID_ARGUMENT: "An unknown kind string or another bad argument.",
  ERR_PERMISSIONS_BRIDGE_UNAVAILABLE: "The host bridge is missing; this is a setup failure, never a silent denial.",
  ERR_PERMISSIONS_SERVICE_DISABLED: "A global service the permission needs is off, such as location services.",
  ERR_PERMISSIONS_IN_PROGRESS: "A request for this kind is already running; one runs at a time.",
  ERR_PERMISSIONS_CANCELLED: "The caller or the system cancelled the request.",
  ERR_PERMISSIONS_TIMEOUT: "The deadline passed before the platform answered.",
  ERR_PERMISSIONS_BACKEND: "The platform's permission service failed.",
};

const KNOWN = new Set<string>(Object.keys(MESSAGES));

/** A rejection of the permissions API. `code` is the stable name; the message says what to do. */
export class PermissionsError extends Error {
  readonly code: PermissionsErrorCode;

  constructor(code: PermissionsErrorCode) {
    super(MESSAGES[code]);
    this.name = "PermissionsError";
    this.code = code;
  }
}

/**
 * Turns whatever the native side or a host threw into a coded `PermissionsError`. A host's
 * rejection carries `ERR_NSH_HOST_ERROR`, and its message starts with the code, as
 * `ERR_PERMISSIONS_CANCELLED: text`. Anything unknown is a backend failure.
 */
export function asPermissionsError(error: unknown): PermissionsError {
  if (error instanceof PermissionsError) return error;
  const raw = (error ?? {}) as { code?: unknown; message?: unknown };
  const code = typeof raw.code === "string" ? raw.code : "";
  const message = typeof raw.message === "string" ? raw.message : "";
  if (code === "ERR_NSH_HOST_ERROR") {
    const named = /^(ERR_PERMISSIONS_[A-Z_]+): /.exec(message)?.[1];
    return named !== undefined && KNOWN.has(named)
      ? new PermissionsError(named as PermissionsErrorCode)
      : new PermissionsError("ERR_PERMISSIONS_BACKEND");
  }
  return KNOWN.has(code) ? new PermissionsError(code as PermissionsErrorCode) : new PermissionsError("ERR_PERMISSIONS_BACKEND");
}
