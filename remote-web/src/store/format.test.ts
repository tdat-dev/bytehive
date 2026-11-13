import { describe, expect, it } from "vitest";
import { accountOf } from "../views/saved";
import { toChat, type Wire } from "./format";

describe("stored items", () => {
  it("become the chat log's items", () => {
    expect(toChat({ kind: "user", id: "c0", text: "Fix the build" })).toEqual({ kind: "user", id: "c0", text: "Fix the build", images: 0, at: 0 });
    expect(toChat({ kind: "step", id: "c1", verb: "Ran", target: "cargo test", code: true, error: true })).toMatchObject({ kind: "step", verb: "Ran", error: true, done: true, code: true });
  });

  it("leave out what is not one", () => {
    expect(toChat({ kind: "image", id: "x" } as unknown as Wire)).toBeNull();
    expect(toChat({ kind: "text", id: "x" } as unknown as Wire)).toBeNull();
    expect(toChat(null as unknown as Wire)).toBeNull();
  });
});

describe("accountOf", () => {
  it("reads the account from a login token's body", () => {
    const body = btoa(JSON.stringify({ typ: "login", account: "123", peer: "d:x" })).replace(/=+$/, "");
    expect(accountOf(`${body}.sig`)).toBe("123");
    expect(accountOf("garbage")).toBe("");
  });
});
