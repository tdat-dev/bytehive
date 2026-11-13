import "fake-indexeddb/auto";
import { describe, expect, it } from "vitest";
import { REMEMBER_MS, forget, recall, remember } from "./remember";

const key = () => crypto.subtle.generateKey({ name: "AES-GCM", length: 256 }, false, ["decrypt"]) as Promise<CryptoKey>;

describe("remembered store key", () => {
  it("keeps the key for 15 days, then forgets it", async () => {
    await remember(await key(), "acct", 0);
    // The store hands back a clone of the key, still unexportable.
    const back = await recall("acct", REMEMBER_MS - 1);
    expect(back?.type).toBe("secret");
    expect(back?.extractable).toBe(false);
    expect(await recall("acct", REMEMBER_MS)).toBeNull();
    expect(await recall("acct", 0)).toBeNull();
  });

  it("is not handed to another account", async () => {
    await remember(await key(), "acct", 0);
    expect(await recall("other", 1)).toBeNull();
  });

  it("forgets at once when asked", async () => {
    await remember(await key(), "acct", 0);
    await forget();
    expect(await recall("acct", 1)).toBeNull();
  });
});
