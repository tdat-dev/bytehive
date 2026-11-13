import { signIn } from "../auth";
import { el, gate } from "./ui";

/** `chooseDevice` runs on the click and answers the id to sign in as, or null when it reloaded the page instead. */
export function showSignIn(root: HTMLElement, deviceId: string, message?: string, chooseDevice?: () => Promise<string | null>): void {
  const b = el("button", "Sign in with GitHub", "btn primary wide");
  b.type = "button";
  b.onclick = async () => {
    if (b.disabled) return;
    b.disabled = true;
    try {
      const id = chooseDevice ? await chooseDevice() : deviceId;
      if (id) signIn(id);
    } catch {
      b.disabled = false;
    }
  };
  // Back from the login page in a browser that kept this one as it was.
  window.addEventListener("pageshow", () => { b.disabled = false; });
  const notes = [
    "Read and answer your agents from this device while they run on your computer.",
    "Messages and terminal output are encrypted end to end. The relay passes them on and can't read them.",
  ];
  gate(root, "ByteHive Remote", message ? [message, ...notes] : notes, b);
}
