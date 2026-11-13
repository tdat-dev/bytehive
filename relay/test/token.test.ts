import { describe, expect, it } from "vitest";
import { sign, verify } from "../src/token";

const secret = "test-secret-test-secret-test-secret";

describe("tokens", () => {
  it("reads back what was signed until it runs out", async () => {
    const t = await sign({ typ: "login", account: "42", peer: "d:a" }, 60, secret);
    expect(await verify(t, secret, "login")).toMatchObject({ account: "42", peer: "d:a" });
    expect(await verify(await sign({ typ: "login", account: "42" }, -1, secret), secret, "login")).toBeNull();
  });
  it("refuses a changed token and another secret", async () => {
    const t = await sign({ typ: "login", account: "42" }, 60, secret);
    expect(await verify(t.replace(/^.{4}/, "AAAA"), secret, "login")).toBeNull();
    expect(await verify(t, "another-secret-another-secret-aaa", "login")).toBeNull();
  });
  it("only verifies as the type it was signed with", async () => {
    const t = await sign({ typ: "state", peer: "d:a" }, 60, secret);
    expect(await verify(t, secret, "state")).not.toBeNull();
    expect(await verify(t, secret, "login")).toBeNull();
    expect(await verify(t, secret, "ticket")).toBeNull();
    expect(await verify(await sign({ account: "42" }, 60, secret), secret, "login")).toBeNull();
  });
});
