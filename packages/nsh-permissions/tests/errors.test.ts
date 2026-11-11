import { expect, test } from "nsh/test";
import { asPermissionsError, PermissionsError, type PermissionsErrorCode } from "../src";

const CODES: PermissionsErrorCode[] = [
  "ERR_PERMISSIONS_UNSUPPORTED_PLATFORM",
  "ERR_PERMISSIONS_INVALID_ARGUMENT",
  "ERR_PERMISSIONS_BRIDGE_UNAVAILABLE",
  "ERR_PERMISSIONS_SERVICE_DISABLED",
  "ERR_PERMISSIONS_IN_PROGRESS",
  "ERR_PERMISSIONS_CANCELLED",
  "ERR_PERMISSIONS_TIMEOUT",
  "ERR_PERMISSIONS_BACKEND",
];

/** The same wording as the Rust crate's messages, so both sides say the same thing. */
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

test("every_kind_has_its_code_and_a_message", () => {
  for (const code of CODES) {
    const error = new PermissionsError(code);
    expect(error.code).toBe(code);
    expect(error.message).toBe(MESSAGES[code]);
    expect(error.name).toBe("PermissionsError");
  }
});

test("a native rejection keeps its code, and an unknown one is a backend failure", () => {
  expect(asPermissionsError({ code: "ERR_PERMISSIONS_SERVICE_DISABLED", message: "x" }).code).toBe(
    "ERR_PERMISSIONS_SERVICE_DISABLED",
  );
  expect(asPermissionsError({ code: "ERR_NSH_HOST_ERROR", message: "ERR_PERMISSIONS_CANCELLED: text" }).code).toBe(
    "ERR_PERMISSIONS_CANCELLED",
  );
  expect(asPermissionsError({ code: "ERR_NSH_HOST_ERROR", message: "ERR_PERMISSIONS_NO_SUCH_CODE: text" }).code).toBe(
    "ERR_PERMISSIONS_BACKEND",
  );
  expect(asPermissionsError(new Error("boom")).code).toBe("ERR_PERMISSIONS_BACKEND");
  expect(asPermissionsError(new PermissionsError("ERR_PERMISSIONS_TIMEOUT")).code).toBe("ERR_PERMISSIONS_TIMEOUT");
});
