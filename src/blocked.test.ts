import { describe, expect, it } from "vitest";
import { blockedOnScreen } from "./blocked";

describe("blockedOnScreen", () => {
  it("catches sign-in and login screens", () => {
    expect(blockedOnScreen("Please sign in to continue")?.kind).toBe("auth");
    expect(blockedOnScreen("You must log in to use Claude Code")?.kind).toBe("auth");
    expect(blockedOnScreen("Authentication expired. Please log in again.")?.kind).toBe("auth");
    expect(blockedOnScreen("Error: Not logged in. Run /login to authenticate.")?.kind).toBe("auth");
  });

  it("catches CAPTCHA and human-verification screens", () => {
    expect(blockedOnScreen("Please complete the CAPTCHA to continue")?.kind).toBe("captcha");
    expect(blockedOnScreen("Verify you are human to proceed")?.kind).toBe("captcha");
  });

  it("catches missing or invalid API keys", () => {
    expect(blockedOnScreen("Error: Invalid API key. Set a valid ANTHROPIC_API_KEY.")?.kind).toBe("key");
    expect(blockedOnScreen("API key not configured")?.kind).toBe("key");
    expect(blockedOnScreen("Set your OPENAI_API_KEY environment variable to continue")?.kind).toBe("key");
  });

  it("keeps the line that said so", () => {
    expect(blockedOnScreen("Please sign in to continue")).toMatchObject({ kind: "auth", what: "Please sign in to continue" });
  });

  it("sees through box drawing around the notice", () => {
    expect(blockedOnScreen("│ Please sign in to continue │")?.kind).toBe("auth");
  });

  it("ignores a notice that scrolled out of the tail", () => {
    const old = ["Please sign in to continue", ...Array.from({ length: 20 }, (_, i) => `working line ${i}`)].join("\n");
    expect(blockedOnScreen(old)).toBeNull();
  });

  it("says nothing on a normal working screen", () => {
    expect(blockedOnScreen("⠋ Reading src/app.ts\n> fix the login flow")).toBeNull();
    expect(blockedOnScreen("")).toBeNull();
  });
});
