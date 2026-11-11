import { expect, test } from "nsh/test";
import { isPermission, type Permission, type Platform, platformOf, supports } from "../src";

const KINDS: Permission[] = [
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
];

test("support_matrix_matches_the_spec", () => {
  const matrix: Record<Platform, Permission[]> = {
    apple: KINDS,
    android: [
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
    ],
    windows: ["camera", "microphone", "location"],
    linux: [],
    web: ["camera", "microphone", "location", "notifications", "motion"],
  };
  for (const platform of Object.keys(matrix) as Platform[]) {
    for (const kind of KINDS) {
      expect(supports(platform, kind)).toBe(matrix[platform].includes(kind));
    }
  }
});

test("isPermission accepts the sixteen wire names and refuses impostors", () => {
  for (const kind of KINDS) {
    expect(isPermission(kind)).toBe(true);
  }
  for (const stranger of ["Camera", "photosaddonly", "locationalways", "camera ", ""]) {
    expect(isPermission(stranger)).toBe(false);
  }
});

test("platformOf prefers its hint, and a browser's permissions API means the web", () => {
  expect(platformOf("linux")).toBe("linux");
  const scope = globalThis as { navigator?: { permissions?: unknown } };
  const hadNavigator = scope.navigator !== undefined;
  scope.navigator = { permissions: {} };
  try {
    expect(platformOf()).toBe("web");
    expect(platformOf("android")).toBe("android");
  } finally {
    if (!hadNavigator) delete scope.navigator;
  }
  // The headless test engine runs as iOS, which the matrix calls apple.
  expect(platformOf()).toBe("apple");
});
