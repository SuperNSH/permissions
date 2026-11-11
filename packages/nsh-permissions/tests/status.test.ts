import { expect, test } from "nsh/test";
import { isStatus, type Status } from "../src";

test("status_wire_names_round_trip", () => {
  const names: Status[] = ["granted", "denied", "permanentlyDenied", "limited", "restricted"];
  for (const name of names) {
    expect(isStatus(name)).toBe(true);
  }
  for (const stranger of ["Granted", "permanentlydenied", "provisional", "notDetermined", "granted ", ""]) {
    expect(isStatus(stranger)).toBe(false);
  }
});
