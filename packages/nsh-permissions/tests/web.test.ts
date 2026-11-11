import { expect, test } from "nsh/test";
import {
  webBackend,
  type Permission,
  type WebBackendOptions,
  type WebNotificationAnswer,
  type WebPermissionState,
} from "../src";

/** A Permissions API stub that always answers `state`, recording the names it was asked. */
function permissionsService(state: WebPermissionState): { service: WebBackendOptions["permissions"]; names: () => string[] } {
  const names: string[] = [];
  return {
    service: {
      query: async (descriptor: { name: string }) => {
        names.push(descriptor.name);
        return { state };
      },
    },
    names: () => names,
  };
}

/** A Notification stub whose prompt answers `answer`, counting the prompts it showed. */
function notificationService(answer: WebNotificationAnswer): { service: WebBackendOptions["notification"]; prompts: () => number } {
  let prompts = 0;
  return {
    service: {
      requestPermission: async () => {
        prompts += 1;
        return answer;
      },
    },
    prompts: () => prompts,
  };
}

test("web_unsupported_kinds_are_coded", async () => {
  const backend = webBackend({
    permissions: permissionsService("granted").service,
    notification: notificationService("granted").service,
  });
  const unsupported: Permission[] = [
    "photos",
    "photosAddOnly",
    "locationAlways",
    "locationWhenInUse",
    "bluetooth",
    "speech",
    "calendar",
    "contacts",
    "reminders",
    "mediaLibrary",
    "appTrackingTransparency",
  ];
  for (const kind of unsupported) {
    await expect(backend.check(kind)).rejects.toMatchObject({ code: "ERR_PERMISSIONS_UNSUPPORTED_PLATFORM" });
    await expect(backend.request(kind)).rejects.toMatchObject({ code: "ERR_PERMISSIONS_UNSUPPORTED_PLATFORM" });
  }
  // The web has no settings app at all, so openSettings refuses for every kind.
  await expect(backend.openSettings("camera")).rejects.toMatchObject({ code: "ERR_PERMISSIONS_UNSUPPORTED_PLATFORM" });
});

test("web_notification_dismissed_is_denied", async () => {
  const permissions = permissionsService("prompt");
  const notification = notificationService("default");
  const backend = webBackend({ permissions: permissions.service, notification: notification.service });
  // The user dismissed the dialog: "default" has no third status to land in, and it lands once.
  expect(await backend.request("notifications")).toBe("denied");
  expect(notification.prompts()).toBe(1);
  const granted = webBackend({
    permissions: permissionsService("prompt").service,
    notification: notificationService("granted").service,
  });
  expect(await granted.request("notifications")).toBe("granted");
});

test("check maps the query states, and a decided kind is never re-prompted", async () => {
  for (const state of ["granted", "denied", "prompt"] as WebPermissionState[]) {
    const permissions = permissionsService(state);
    const notification = notificationService("granted");
    const backend = webBackend({ permissions: permissions.service, notification: notification.service });
    // Check: granted stays granted; denied and prompt both read denied, the web's own limit.
    expect(await backend.check("camera")).toBe(state === "granted" ? "granted" : "denied");
    // Request: a decided kind answers itself without prompting again.
    expect(await backend.request("camera")).toBe(state === "granted" ? "granted" : "denied");
    expect(notification.prompts()).toBe(0);
  }
});

test("the web names reach the Permissions API, and a refused query keeps its code", async () => {
  const permissions = permissionsService("granted");
  const backend = webBackend({ permissions: permissions.service });
  await backend.check("location");
  await backend.check("motion");
  expect(permissions.names()).toEqual(["geolocation", "accelerometer"]);
  const broken = webBackend({
    permissions: {
      query: async () => {
        throw new Error("unsupported");
      },
    },
  });
  await expect(broken.check("camera")).rejects.toMatchObject({ code: "ERR_PERMISSIONS_BACKEND" });
});

test("notifications fall back to Notification's own answer when the query refuses", async () => {
  const backend = webBackend({
    permissions: {
      query: async () => {
        throw new Error("no notifications query");
      },
    },
    notification: { requestPermission: async () => "granted", permission: "denied" },
  });
  expect(await backend.check("notifications")).toBe("denied");
});

test("with no seams and no globals, every call reports the missing bridge", async () => {
  const backend = webBackend();
  await expect(backend.check("camera")).rejects.toMatchObject({ code: "ERR_PERMISSIONS_BRIDGE_UNAVAILABLE" });
  await expect(backend.request("notifications")).rejects.toMatchObject({ code: "ERR_PERMISSIONS_BRIDGE_UNAVAILABLE" });
});

test("a notification seam alone never answers another kind", async () => {
  const backend = webBackend({
    notification: { requestPermission: async () => "granted", permission: "granted" },
  });
  // No Permissions service: camera, microphone, location and motion have no seam left, and the
  // notification's granted state is not theirs to borrow.
  for (const kind of ["camera", "microphone", "location", "motion"] as Permission[]) {
    await expect(backend.check(kind)).rejects.toMatchObject({ code: "ERR_PERMISSIONS_BRIDGE_UNAVAILABLE" });
    await expect(backend.request(kind)).rejects.toMatchObject({ code: "ERR_PERMISSIONS_BRIDGE_UNAVAILABLE" });
  }
  // The notifications kind, whose seam it is, still reads it.
  expect(await backend.check("notifications")).toBe("granted");
});
