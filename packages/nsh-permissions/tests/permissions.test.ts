import { expect, test } from "nsh/test";
import {
  memoryBackend,
  Permissions,
  type PermissionsBackend,
  type PermissionsErrorCode,
  type Permission,
} from "../src";

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

test("every_kind_reaches_the_caller_with_its_code", async () => {
  const backend = memoryBackend();
  const permissions = Permissions.withBackend(backend, { request: 500, query: 500 });
  // Seven codes ride a scripted backend, through check and request alike; the eighth is the
  // facade's own refusal of a second request while one runs.
  for (const code of CODES) {
    if (code === "ERR_PERMISSIONS_IN_PROGRESS") continue;
    backend.setCheck("camera", code);
    backend.setRequestAnswer("camera", code);
    await expect(permissions.check("camera")).rejects.toMatchObject({ code });
    await expect(permissions.request("camera")).rejects.toMatchObject({ code });
  }
  backend.holdRequestsForTheTest();
  const held = permissions.request("camera");
  await expect(permissions.request("camera")).rejects.toMatchObject({ code: "ERR_PERMISSIONS_IN_PROGRESS" });
  backend.finishRequest("camera", "granted");
  expect(await held).toBe("granted");
});

test("unknown_kind_string_is_refused", async () => {
  const backend = memoryBackend();
  const permissions = Permissions.withBackend(backend, { request: 500, query: 500 });
  await expect(permissions.check("locationalways" as Permission)).rejects.toMatchObject({
    code: "ERR_PERMISSIONS_INVALID_ARGUMENT",
  });
  await expect(permissions.request("" as Permission)).rejects.toMatchObject({
    code: "ERR_PERMISSIONS_INVALID_ARGUMENT",
  });
  await expect(permissions.openSettings("Camera" as Permission)).rejects.toMatchObject({
    code: "ERR_PERMISSIONS_INVALID_ARGUMENT",
  });
  // None of them reached the backend.
  expect(backend.requestsStarted()).toBe(0);
});

test("second_request_while_one_runs_is_in_progress", async () => {
  const backend = memoryBackend();
  backend.holdRequestsForTheTest();
  const permissions = Permissions.withBackend(backend, { request: 1000 });
  const first = permissions.request("camera");
  await expect(permissions.request("camera")).rejects.toMatchObject({ code: "ERR_PERMISSIONS_IN_PROGRESS" });
  // The gate is per kind: another kind's request runs beside it.
  const other = permissions.request("microphone");
  expect(backend.requestsStarted()).toBe(2);
  expect(backend.finishRequest("camera", "granted")).toBe(true);
  expect(await first).toBe("granted");
  expect(backend.finishRequest("microphone", "denied")).toBe(true);
  expect(await other).toBe("denied");
  // With the first answered, a new camera request runs again.
  const again = permissions.request("camera");
  await expect(permissions.request("camera")).rejects.toMatchObject({ code: "ERR_PERMISSIONS_IN_PROGRESS" });
  backend.finishRequest("camera", "limited");
  expect(await again).toBe("limited");
});

test("ios_missing_bridge_is_never_silent_denial", async () => {
  const backend = memoryBackend();
  backend.setCheck("camera", "ERR_PERMISSIONS_BRIDGE_UNAVAILABLE");
  backend.setRequestAnswer("camera", "ERR_PERMISSIONS_BRIDGE_UNAVAILABLE");
  const permissions = Permissions.withBackend(backend, { request: 500, query: 500 });
  // The code reaches the caller as itself, never dressed up as a denied status.
  await expect(permissions.check("camera")).rejects.toMatchObject({ code: "ERR_PERMISSIONS_BRIDGE_UNAVAILABLE" });
  await expect(permissions.request("camera")).rejects.toMatchObject({ code: "ERR_PERMISSIONS_BRIDGE_UNAVAILABLE" });
});

test("statuses pass through, openSettings resolves, and a scripted refusal keeps its code", async () => {
  const backend = memoryBackend({ check: { photos: "limited" } });
  const permissions = Permissions.withBackend(backend, { request: 500, query: 500 });
  expect(await permissions.check("photos")).toBe("limited");
  backend.setRequestAnswer("photos", "permanentlyDenied");
  expect(await permissions.request("photos")).toBe("permanentlyDenied");
  expect(await permissions.openSettings("photos")).toBeUndefined();
  backend.setOpenSettings("photos", "ERR_PERMISSIONS_SERVICE_DISABLED");
  await expect(permissions.openSettings("photos")).rejects.toMatchObject({
    code: "ERR_PERMISSIONS_SERVICE_DISABLED",
  });
  // With the script cleared, the kind resolves again.
  backend.setOpenSettings("photos");
  expect(await permissions.openSettings("photos")).toBeUndefined();
});

test("a backend that throws before it promises rejects with a code", async () => {
  const memory = memoryBackend();
  let wedge = true;
  const backend: PermissionsBackend = {
    ...memory,
    check: (kind) => {
      if (wedge) throw new Error("wedge");
      return memory.check(kind);
    },
  };
  const permissions = Permissions.withBackend(backend, { query: 500 });
  await expect(permissions.check("camera")).rejects.toMatchObject({ code: "ERR_PERMISSIONS_BACKEND" });
  wedge = false;
  expect(await permissions.check("camera")).toBe("granted");
});

test("an invalid deadline is refused without calling the backend", async () => {
  const backend = memoryBackend();
  const permissions = Permissions.withBackend(backend, { request: Number.NaN });
  await expect(permissions.request("camera")).rejects.toMatchObject({ code: "ERR_PERMISSIONS_INVALID_ARGUMENT" });
  expect(backend.requestsStarted()).toBe(0);
});

test("a browser's navigator takes the default backend to the web APIs", async () => {
  const scope = globalThis as { navigator?: { permissions?: unknown } };
  const hadNavigator = scope.navigator !== undefined;
  scope.navigator = { permissions: {} };
  try {
    const permissions = new Permissions({ timeouts: { query: 500 } });
    // photos is outside the web matrix, so the web backend refuses it: proof of which backend runs.
    await expect(permissions.check("photos")).rejects.toMatchObject({
      code: "ERR_PERMISSIONS_UNSUPPORTED_PLATFORM",
    });
  } finally {
    if (!hadNavigator) delete scope.navigator;
  }
});

test("default-constructed instances share one process-wide gate registry", async () => {
  const backend = memoryBackend();
  backend.holdRequestsForTheTest();
  const first = new Permissions({ backend, timeouts: { request: 1000 } });
  const second = new Permissions({ backend, timeouts: { request: 1000 } });
  const call = first.request("camera");
  // The second handle is another object, but the process allows one camera request at a time.
  await expect(second.request("camera")).rejects.toMatchObject({ code: "ERR_PERMISSIONS_IN_PROGRESS" });
  // A withBackend handle gates alone: the same kind runs beside the shared one, on its own backend.
  const isolatedBackend = memoryBackend();
  isolatedBackend.holdRequestsForTheTest();
  const beside = Permissions.withBackend(isolatedBackend, { request: 1000 }).request("camera");
  expect(backend.finishRequest("camera", "granted")).toBe(true);
  expect(await call).toBe("granted");
  expect(isolatedBackend.finishRequest("camera", "denied")).toBe(true);
  expect(await beside).toBe("denied");
});
