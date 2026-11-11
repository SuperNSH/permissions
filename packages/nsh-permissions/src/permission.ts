import { platform as hostPlatform } from "nsh";

/** The sixteen permission kinds, as the wire names them. The wire string is the name. */
export type Permission =
  | "camera"
  | "microphone"
  | "photos"
  | "photosAddOnly"
  | "location"
  | "locationAlways"
  | "locationWhenInUse"
  | "notifications"
  | "bluetooth"
  | "speech"
  | "calendar"
  | "contacts"
  | "reminders"
  | "motion"
  | "mediaLibrary"
  | "appTrackingTransparency";

const KINDS: ReadonlySet<Permission> = new Set<Permission>([
  "camera",
  "microphone",
  "photos",
  "photosAddOnly",
  "location",
  "locationAlways",
  "locationWhenInUse",
  "notifications",
  "bluetooth",
  "speech",
  "calendar",
  "contacts",
  "reminders",
  "motion",
  "mediaLibrary",
  "appTrackingTransparency",
]);

/** Whether `s` is one of the sixteen kind wire names. */
export function isPermission(s: string): s is Permission {
  // The cast is the guard: membership is exactly the question the set answers.
  return KINDS.has(s as Permission);
}

/** The platforms this library serves, as the support matrix names them. iOS and macOS are both `apple`. */
export type Platform = "apple" | "android" | "windows" | "linux" | "web";

/** The kinds each platform serves this release. A `no` cell answers `ERR_PERMISSIONS_UNSUPPORTED_PLATFORM`. */
const MATRIX: Record<Platform, ReadonlySet<Permission>> = {
  apple: KINDS,
  android: new Set([
    "camera",
    "microphone",
    "photos",
    "location",
    "locationAlways",
    "locationWhenInUse",
    "notifications",
    "bluetooth",
    "speech",
    "calendar",
    "contacts",
    "motion",
    "mediaLibrary",
  ]),
  windows: new Set(["camera", "microphone", "location"]),
  linux: new Set(),
  web: new Set(["camera", "microphone", "location", "notifications", "motion"]),
};

/** Whether `platform` serves `kind` this release. */
export function supports(platform: Platform, kind: Permission): boolean {
  return MATRIX[platform].has(kind);
}

/**
 * The platform the app runs on, for the support matrix. `hint` overrides the detection. A
 * browser's Permissions API means the web: no NSH runtime exposes `navigator`, so that check can
 * only fire outside one. Under NSH the engine's own platform decides.
 */
export function platformOf(hint?: Platform): Platform {
  if (hint !== undefined) return hint;
  const navigator = (globalThis as { navigator?: { permissions?: unknown } }).navigator;
  if (navigator !== undefined && navigator.permissions !== undefined) return "web";
  switch (hostPlatform.os) {
    case "ios":
    case "macos":
      return "apple";
    case "android":
      return "android";
    case "windows":
      return "windows";
    case "web":
      return "web";
    default:
      return "linux";
  }
}
