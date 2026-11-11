import { mount, signal, Text, View } from "nsh";
import { Permissions, PermissionsError, type Status } from "./src";

/** The device's permissions, through the Rust crate. */
const permissions = new Permissions();

const message = signal("camera checking");

function codeOf(error: unknown): string {
  return error instanceof PermissionsError ? error.code : "unknown error";
}

// The screen's one read: the camera status, or the code its rejection carries.
permissions.check("camera").then(
  (status: Status) => message.set(`camera ${status}`),
  (error: unknown) => message.set(`camera error ${codeOf(error)}`),
);

function App() {
  const cancel = () => {
    const stopped = permissions.cancel("camera");
    message.set(stopped ? "cancelled the request" : "no request was running");
  };
  return (
    <View class="flex-1 gap-3 p-4">
      <Text testID="title" class="text-foreground">nsh-permissions</Text>
      <Text testID="status" class="text-foreground">{message()}</Text>
      <Text testID="cancel" role="button" onPress={cancel} class="rounded-(--radius) bg-muted px-3 py-2">
        Cancel
      </Text>
    </View>
  );
}

mount(App);
