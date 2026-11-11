import { NativePermissions } from "nsh-permissions/native";
import type { PermissionsBackend } from "./backend";
import { asPermissionsError } from "./errors";
import type { Permission } from "./permission";
import type { Status } from "./status";

/**
 * The Rust crate's permissions (`nsh-permissions/native`). The app or package must list the
 * crate under `"rust"` in `nsh.json`; importing this module without it fails the bundle.
 */
export function rustBackend(): PermissionsBackend {
  return new RustBackend();
}

class RustBackend implements PermissionsBackend {
  private readonly native = new NativePermissions();

  check(kind: Permission): Promise<Status> {
    return this.settle(async () => (await this.native.check(kind)) as Status);
  }

  request(kind: Permission): Promise<Status> {
    return this.settle(async () => (await this.native.request(kind)) as Status);
  }

  openSettings(kind: Permission): Promise<void> {
    return this.settle(() => this.native.openSettings(kind));
  }

  private async settle<T>(operation: () => Promise<T>): Promise<T> {
    try {
      return await operation();
    } catch (error) {
      throw asPermissionsError(error);
    }
  }
}
