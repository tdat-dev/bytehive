import { describe, expect, it } from "vitest";
import { checks, deriveKey, encBits, open } from "./crypto";

// The shared vector: native/src/cloud/crypto_tests.rs pins the same values.
const PASS = "correct horse battery staple";
const SALT = new TextEncoder().encode("bytehive-salt-01");
const ROUNDS = 1000;
const hex = (b: ArrayBuffer) => [...new Uint8Array(b)].map((x) => x.toString(16).padStart(2, "0")).join("");

describe("store crypto", () => {
  it("derives the key the machine derives", async () => {
    expect(hex(await encBits(PASS, SALT, ROUNDS))).toBe("1880b42ad33347acbb9290e909ae7c12b91d2d4c4c67133a9c143190ff62ff38");
  });

  it("opens what the machine sealed, only in its own slot", async () => {
    const key = await deriveKey(PASS, SALT, ROUNDS);
    const sealed = "AQcHBwcHBwcHBwcHB8dvBfsKqrtJPA98h8dzr683MFLazg";
    expect(new TextDecoder().decode((await open(key, "abc/0", sealed))!)).toBe("hello");
    expect(await open(key, "abc/1", sealed)).toBeNull();
    expect(await open(await deriveKey("wrong", SALT, ROUNDS), "abc/0", sealed)).toBeNull();
    expect(await open(key, "abc/0", "not sealed")).toBeNull();
  });

  it("says no to a check blob it cannot open", async () => {
    const key = await deriveKey(PASS, SALT, ROUNDS);
    expect(await checks(key, "AQcHBwcHBwcHBwcHB8dvBfsKqrtJPA98h8dzr683MFLazg")).toBe(false);
  });
});
