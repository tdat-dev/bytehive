import { describe, expect, it, vi } from "vitest";
import { showSignIn } from "./signin";

const auth = vi.hoisted(() => ({ signIn: vi.fn() }));
vi.mock("../auth", () => auth);

describe("the sign-in page", () => {
  it("goes out once for two presses", async () => {
    auth.signIn.mockClear();
    const root = document.createElement("div");
    showSignIn(root, "dev1");
    const b = root.querySelector("button")!;
    b.click();
    b.click();
    await vi.waitFor(() => expect(auth.signIn).toHaveBeenCalledWith("dev1"));
    expect(auth.signIn).toHaveBeenCalledTimes(1);
    expect(b.disabled).toBe(true);
  });

  it("asks for the device once, and signs in as what it answers", async () => {
    auth.signIn.mockClear();
    const choose = vi.fn(async () => "fresh");
    const root = document.createElement("div");
    showSignIn(root, "old", "This device was removed.", choose);
    expect(root.querySelector("p")?.textContent).toBe("This device was removed.");
    const b = root.querySelector("button")!;
    b.click();
    b.click();
    await vi.waitFor(() => expect(auth.signIn).toHaveBeenCalledWith("fresh"));
    expect(choose).toHaveBeenCalledTimes(1);
  });

  it("can be pressed again after the browser comes back to the page", () => {
    const root = document.createElement("div");
    showSignIn(root, "dev1");
    const b = root.querySelector("button")!;
    b.click();
    window.dispatchEvent(new Event("pageshow"));
    expect(b.disabled).toBe(false);
  });
});
