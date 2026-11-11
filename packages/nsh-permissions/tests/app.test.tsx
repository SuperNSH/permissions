import { expect, open, test } from "nsh/test";

test("the screen opens, shows the camera status, and cancel with no request says so", async () => {
  const app = await open("../App.tsx");
  // The status line must land on a status or a coded error, whichever the platform answers;
  // the opening "camera checking" placeholder does not count.
  await app.waitFor("text:/^camera (granted|denied|permanentlyDenied|limited|restricted|error ERR_PERMISSIONS_[A-Z_]+)$/");
  await app.press("id:cancel");
  await app.waitFor("text:no request was running");
});
