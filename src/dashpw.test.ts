import { describe, expect, it } from "vitest";
import { MIN_PASSWORD, passwordNotes, passwordProblem } from "./dashpw";

describe("dashboard password", () => {
  it("wants something long enough, typed the same twice", () => {
    expect(passwordProblem("", "")).toBe("Type a password.");
    expect(passwordProblem("abc", "abc")).toBe(`Use at least ${MIN_PASSWORD} characters.`);
    expect(passwordProblem("correct horse", "correct hose")).toBe("The two don't match.");
    expect(passwordProblem("correct horse", "correct horse")).toBeNull();
  });

  it("counts characters, not bytes", () => {
    expect(passwordProblem("mật khẩu", "mật khẩu")).toBeNull();
    expect(passwordProblem("ẩẩẩẩẩ", "ẩẩẩẩẩ")).not.toBeNull();
  });

  it("explains why LAN waits for a password", () => {
    expect(passwordNotes(false).lan).toMatch(/Set a password first/);
    expect(passwordNotes(true).pw).toMatch(/^Set\./);
    for (const n of [passwordNotes(false), passwordNotes(true)]) {
      expect(n.pw + n.lan).not.toContain(String.fromCharCode(0x2014));
    }
  });
});
