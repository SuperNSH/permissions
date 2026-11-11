import { expect, test } from "nsh/test";
import { memoryBackend, Permissions, type PermissionsBackend, type Permission, type Status } from "../src";

/** Lets the answer of a finished backend call reach the facade, which settles a microtask later. */
async function delivered(): Promise<void> {
  for (let turn = 0; turn < 4; turn += 1) await Promise.resolve();
}

test("a_call_resolves_exactly_once", async () => {
  const backend = memoryBackend({ request: { camera: ["granted"] } });
  backend.answerRequestsTwice();
  const permissions = Permissions.withBackend(backend, { request: 1000 });
  let answers = 0;
  const call = permissions.request("camera").then((status) => {
    answers += 1;
    return status;
  });
  expect(await call).toBe("granted");
  await delivered();
  // The second delivery found the caller served; the gate is free for the next request.
  expect(answers).toBe(1);
  expect(await permissions.request("camera")).toBe("granted");
});

test("cancel_resolves_with_cancelled", async () => {
  const backend = memoryBackend();
  backend.holdRequestsForTheTest();
  const permissions = Permissions.withBackend(backend, { request: 1000 });
  const call = permissions.request("camera");
  expect(backend.requestPending("camera")).toBe(true);
  expect(permissions.cancel("camera")).toBe(true);
  await expect(call).rejects.toMatchObject({ code: "ERR_PERMISSIONS_CANCELLED" });
  expect(permissions.cancel("camera")).toBe(false);
  // The prompt the backend held still waits there; finishing it changes nothing now.
  expect(backend.finishRequest("camera", "granted")).toBe(true);
  expect(backend.requestPending("camera")).toBe(false);
  await delivered();
  const next = permissions.request("camera");
  expect(backend.finishRequest("camera", "granted")).toBe(true);
  expect(await next).toBe("granted");
});

test("deadline_resolves_with_timeout", async () => {
  // A request a hanging platform never answers ends at its deadline, and the gate recovers.
  const backend = memoryBackend();
  backend.neverAnswerRequests();
  const permissions = Permissions.withBackend(backend, { request: 20 });
  await expect(permissions.request("camera")).rejects.toMatchObject({ code: "ERR_PERMISSIONS_TIMEOUT" });
  await expect(permissions.request("camera")).rejects.toMatchObject({ code: "ERR_PERMISSIONS_TIMEOUT" });
  expect(backend.requestsStarted()).toBe(2);
  // A check that never answers meets the query deadline the same way.
  const silent: PermissionsBackend = {
    check: () => new Promise<Status>(() => {}),
    request: (kind: Permission) => memoryBackend().request(kind),
    openSettings: () => new Promise<void>(() => {}),
  };
  const quiet = Permissions.withBackend(silent, { query: 20 });
  await expect(quiet.check("camera")).rejects.toMatchObject({ code: "ERR_PERMISSIONS_TIMEOUT" });
});

test("the script answers checks and requests, and the queue repeats its last answer", async () => {
  const backend = memoryBackend({ check: { camera: "denied" }, request: { camera: ["limited", "granted"] } });
  const permissions = Permissions.withBackend(backend, { request: 500, query: 500 });
  expect(await permissions.check("camera")).toBe("denied");
  expect(await permissions.request("camera")).toBe("limited");
  expect(await permissions.request("camera")).toBe("granted");
  expect(await permissions.request("camera")).toBe("granted");
  expect(backend.requestsStarted()).toBe(3);
});

test("a request that fails before it starts is not counted and keeps its code", async () => {
  const backend = memoryBackend();
  backend.failNextRequest("ERR_PERMISSIONS_SERVICE_DISABLED");
  const permissions = Permissions.withBackend(backend, { request: 500 });
  await expect(permissions.request("camera")).rejects.toMatchObject({
    code: "ERR_PERMISSIONS_SERVICE_DISABLED",
  });
  expect(backend.requestsStarted()).toBe(0);
  expect(await permissions.request("camera")).toBe("granted");
});

test("returning to the script answers requests at once again", async () => {
  const backend = memoryBackend({ request: { camera: ["denied"] } });
  const permissions = Permissions.withBackend(backend, { request: 500 });
  backend.holdRequestsForTheTest();
  const held = permissions.request("camera");
  expect(backend.requestPending("camera")).toBe(true);
  expect(backend.finishRequest("camera", "granted")).toBe(true);
  expect(await held).toBe("granted");
  // Back to the script: the next request answers itself, with no finishRequest to drive it.
  backend.answerRequestsAtOnce();
  expect(await permissions.request("camera")).toBe("denied");
  expect(backend.requestPending("camera")).toBe(false);
});
