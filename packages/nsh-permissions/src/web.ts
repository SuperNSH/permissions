import type { PermissionsBackend } from "./backend";
import { asPermissionsError, PermissionsError } from "./errors";
import { supports, type Permission } from "./permission";
import type { Status } from "./status";

/** A state of the browser's Permissions API. */
export type WebPermissionState = "granted" | "denied" | "prompt";

/** The part of the browser's Permissions API this backend needs. */
export interface WebPermissionsService {
  query(descriptor: { name: string }): Promise<{ state: WebPermissionState }>;
}

/** A `Notification.requestPermission` answer. A dismissed dialog answers `default`. */
export type WebNotificationAnswer = "granted" | "denied" | "default";

/** The part of the browser's Notification API this backend needs. */
export interface WebNotificationService {
  /** Shows the browser's prompt and answers the user's decision. */
  requestPermission(): Promise<WebNotificationAnswer>;
  /** The last answer, where the browser publishes it. */
  readonly permission?: WebNotificationAnswer;
}

/** The seams the web backend runs on; each defaults to the matching browser global. */
export interface WebBackendOptions {
  permissions?: WebPermissionsService;
  notification?: WebNotificationService;
}

/** The browser's own names for the kinds the web matrix serves. */
const WEB_NAMES: Partial<Record<Permission, string>> = {
  camera: "camera",
  microphone: "microphone",
  location: "geolocation",
  notifications: "notifications",
  motion: "accelerometer",
};

/**
 * The web's permissions, over `navigator.permissions` and `Notification.requestPermission`. Where
 * no browser globals exist — an NSH sandbox exposes no `navigator` — every call reports
 * `ERR_PERMISSIONS_BRIDGE_UNAVAILABLE` rather than guess an answer.
 */
export function webBackend(options: WebBackendOptions = {}): PermissionsBackend {
  return new WebBackend(options);
}

class WebBackend implements PermissionsBackend {
  private readonly options: WebBackendOptions;

  constructor(options: WebBackendOptions) {
    this.options = options;
  }

  async check(kind: Permission): Promise<Status> {
    return checkStatus(await this.state(kind));
  }

  async request(kind: Permission): Promise<Status> {
    const state = await this.state(kind);
    // A decided kind answers itself: the web never re-prompts granted or denied.
    if (state !== "prompt") return checkStatus(state);
    if (kind === "notifications") {
      const notification = this.notification();
      if (notification === null) throw new PermissionsError("ERR_PERMISSIONS_BRIDGE_UNAVAILABLE");
      // A dismissed dialog answers "default"; the web has no third status, so it lands as denied.
      return notificationStatus(await notification.requestPermission());
    }
    // The Permissions API exposes no prompt for these kinds, so the not-asked state stays denied.
    return "denied";
  }

  async openSettings(kind: Permission): Promise<void> {
    // The web has no settings app to open.
    throw new PermissionsError("ERR_PERMISSIONS_UNSUPPORTED_PLATFORM");
  }

  /** The query state of `kind`, refusing kinds outside the web matrix. */
  private async state(kind: Permission): Promise<WebPermissionState> {
    const name = WEB_NAMES[kind];
    if (name === undefined || !supports("web", kind)) {
      throw new PermissionsError("ERR_PERMISSIONS_UNSUPPORTED_PLATFORM");
    }
    const service = this.service();
    if (service === null) {
      // Only notifications has a seam beside the query; the other web kinds answer nowhere else,
      // and guessing from the notification state would report one permission as another.
      if (kind !== "notifications") throw new PermissionsError("ERR_PERMISSIONS_BRIDGE_UNAVAILABLE");
      return this.notificationState();
    }
    try {
      return (await service.query({ name })).state;
    } catch (error) {
      // Some browsers refuse the notifications query outright; Notification's own answer stands in.
      if (kind !== "notifications") throw asPermissionsError(error);
    }
    return this.notificationState();
  }

  /** The notifications state as Notification itself reports it, the only kind it may answer for. */
  private notificationState(): WebPermissionState {
    const notification = this.notification();
    if (notification?.permission !== undefined) {
      // "default" is the browser's name for not asked yet, which is a prompt.
      return notification.permission === "default" ? "prompt" : notification.permission;
    }
    throw new PermissionsError("ERR_PERMISSIONS_BRIDGE_UNAVAILABLE");
  }

  private service(): WebPermissionsService | null {
    return this.options.permissions ?? (globalThis as { navigator?: { permissions?: WebPermissionsService } }).navigator?.permissions ?? null;
  }

  private notification(): WebNotificationService | null {
    return this.options.notification ?? (globalThis as { Notification?: WebNotificationService }).Notification ?? null;
  }
}

/** Check maps granted to granted, and denied and prompt both to denied: the web cannot tell "not asked" from "denied". */
function checkStatus(state: WebPermissionState): Status {
  return state === "granted" ? "granted" : "denied";
}

function notificationStatus(answer: WebNotificationAnswer): Status {
  return answer === "granted" ? "granted" : "denied";
}
