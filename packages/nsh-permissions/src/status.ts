/** The five statuses a permission can be in, as `permission_handler` names them. The wire string is the name. */
export type Status = "granted" | "denied" | "permanentlyDenied" | "limited" | "restricted";

const STATUSES: ReadonlySet<string> = new Set([
  "granted",
  "denied",
  "permanentlyDenied",
  "limited",
  "restricted",
]);

/** Whether `s` is one of the five status wire names. */
export function isStatus(s: string): s is Status {
  return STATUSES.has(s);
}
