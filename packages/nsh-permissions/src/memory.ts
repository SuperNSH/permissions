import type { PermissionsBackend } from "./backend";
import { type PermissionsErrorCode, PermissionsError } from "./errors";
import type { Permission } from "./permission";
import { isStatus, type Status } from "./status";

/** One scripted answer: a status resolves the call, an error code rejects it. */
export type MemoryAnswer = Status | PermissionsErrorCode;

/** The answers a memory backend starts with; every part is optional. */
export interface MemoryPermissionsConfig {
  /** What `check` answers per kind; a kind left out answers `granted`. */
  check?: Partial<Record<Permission, MemoryAnswer>>;
  /** The answers a `request` hands out per kind, in order; an empty queue grants. */
  request?: Partial<Record<Permission, MemoryAnswer[]>>;
  /** The code an `openSettings` for the kind rejects with; a kind left out resolves. */
  openSettings?: Partial<Record<Permission, PermissionsErrorCode>>;
}

/** A backend the test scripts and drives, the twin of the Rust crate's `FakeBackend`. */
export interface MemoryPermissions extends PermissionsBackend {
  /** Sets what `check` answers for `kind`. */
  setCheck(kind: Permission, answer: MemoryAnswer): void;
  /** Queues the next answer a `request` for `kind` returns; once the queue runs dry, the last answer repeats. */
  setRequestAnswer(kind: Permission, answer: MemoryAnswer): void;
  /** Makes `openSettings` for `kind` reject with `code`, or resolve when `code` is left out. */
  setOpenSettings(kind: Permission, code?: PermissionsErrorCode): void;
  /** Makes every request wait for the test to finish it, as a prompt the user has not answered. */
  holdRequestsForTheTest(): void;
  /** Returns to answering each request at once from the script. */
  answerRequestsAtOnce(): void;
  /** Makes every request never answer, as a platform that hangs. */
  neverAnswerRequests(): void;
  /** Makes every request deliver its answer twice, as a misbehaving host. */
  answerRequestsTwice(): void;
  /** Makes the next request fail before it starts, as a platform that cannot show a prompt. */
  failNextRequest(code: PermissionsErrorCode): void;
  /** Whether a held request for `kind` waits for the test. */
  requestPending(kind: Permission): boolean;
  /** Answers the held request for `kind`; returns `false` when none waits. */
  finishRequest(kind: Permission, answer: MemoryAnswer): boolean;
  /** How many requests were started; one that failed to start does not count. */
  requestsStarted(): number;
}

/** How the backend answers requests: from the script, or one of the staged platform behaviours. */
type RequestMode = "answers" | "never" | "twice" | "manual";

/** A memory backend, for tests and for apps that run without a host. */
export function memoryBackend(config: MemoryPermissionsConfig = {}): MemoryPermissions {
  let mode: RequestMode = "answers";
  let failNext: PermissionsErrorCode | null = null;
  const checks: Partial<Record<Permission, MemoryAnswer>> = { ...config.check };
  const settings: Partial<Record<Permission, PermissionsErrorCode>> = { ...config.openSettings };
  const queues: Partial<Record<Permission, MemoryAnswer[]>> = {};
  const last: Partial<Record<Permission, MemoryAnswer>> = {};
  const waiting: Partial<Record<Permission, (answer: MemoryAnswer) => void>> = {};
  let started = 0;

  for (const kind of Object.keys(config.request ?? {}) as Permission[]) {
    queues[kind] = [...(config.request?.[kind] ?? [])];
  }

  const deliver = (
    resolve: (status: Status) => void,
    reject: (error: PermissionsError) => void,
    answer: MemoryAnswer,
  ) => {
    if (isStatus(answer)) resolve(answer);
    else reject(new PermissionsError(answer));
  };

  /** The next scripted answer for `kind`, repeating the last one; an unscripted kind grants. */
  const nextAnswer = (kind: Permission): MemoryAnswer => {
    const queue = queues[kind];
    if (queue !== undefined && queue.length > 0) {
      const answer = queue.shift() as MemoryAnswer;
      last[kind] = answer;
      return answer;
    }
    return last[kind] ?? "granted";
  };

  return {
    check: (kind) =>
      new Promise<Status>((resolve, reject) => {
        deliver(resolve, reject, checks[kind] ?? "granted");
      }),
    request: (kind) =>
      new Promise<Status>((resolve, reject) => {
        if (failNext !== null) {
          const code = failNext;
          failNext = null;
          reject(new PermissionsError(code));
          return;
        }
        started += 1;
        const answer = nextAnswer(kind);
        if (mode === "never") return;
        if (mode === "twice") {
          // A promise settles once however often the delivery runs: the caller sees one answer.
          deliver(resolve, reject, answer);
          deliver(resolve, reject, answer);
          return;
        }
        if (mode === "manual") {
          waiting[kind] = (finish) => deliver(resolve, reject, finish);
          return;
        }
        deliver(resolve, reject, answer);
      }),
    openSettings: (kind) =>
      new Promise<void>((resolve, reject) => {
        const code = settings[kind];
        if (code !== undefined) reject(new PermissionsError(code));
        else resolve();
      }),
    setCheck: (kind, answer) => {
      checks[kind] = answer;
    },
    setRequestAnswer: (kind, answer) => {
      const queue = queues[kind] ?? [];
      queue.push(answer);
      queues[kind] = queue;
    },
    setOpenSettings: (kind, code) => {
      if (code === undefined) delete settings[kind];
      else settings[kind] = code;
    },
    holdRequestsForTheTest: () => {
      mode = "manual";
    },
    answerRequestsAtOnce: () => {
      mode = "answers";
    },
    neverAnswerRequests: () => {
      mode = "never";
    },
    answerRequestsTwice: () => {
      mode = "twice";
    },
    failNextRequest: (code) => {
      failNext = code;
    },
    requestPending: (kind) => waiting[kind] !== undefined,
    finishRequest: (kind, answer) => {
      const done = waiting[kind];
      delete waiting[kind];
      done?.(answer);
      return done !== undefined;
    },
    requestsStarted: () => started,
  };
}
