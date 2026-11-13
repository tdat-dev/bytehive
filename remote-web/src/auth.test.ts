import { beforeEach, describe, expect, it } from "vitest";
import { NONCE_KEY, takeTokenFromUrl } from "./auth";
import { loadToken, resetForTest, saveToken } from "./keys";

const b64 = (s: string) => btoa(s).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
const tokenFor = (peer: string) => `${b64(JSON.stringify({ typ: "login", account: "1", peer }))}.sig`;

describe("taking a login from the address", () => {
  beforeEach(async () => {
    await resetForTest();
    await saveToken(null);
    sessionStorage.clear();
    history.replaceState(null, "", "/");
  });

  it("keeps a login this tab asked for, for this device, and clears the fragment", async () => {
    sessionStorage.setItem(NONCE_KEY, "n");
    location.hash = `#token=${encodeURIComponent(tokenFor("d:mine"))}`;
    await takeTokenFromUrl("mine");
    expect(await loadToken()).toBe(tokenFor("d:mine"));
    expect(location.hash).toBe("");
    expect(sessionStorage.getItem(NONCE_KEY)).toBeNull();
  });
  it("ignores a login nobody here asked for, and still clears the fragment", async () => {
    await saveToken("old");
    location.hash = `#token=${encodeURIComponent(tokenFor("d:mine"))}`;
    await takeTokenFromUrl("mine");
    expect(await loadToken()).toBe("old");
    expect(location.hash).toBe("");
  });
  it("ignores a login made for another device or a machine", async () => {
    for (const peer of ["d:other", "m:mine"]) {
      sessionStorage.setItem(NONCE_KEY, "n");
      location.hash = `#token=${encodeURIComponent(tokenFor(peer))}`;
      await takeTokenFromUrl("mine");
      expect(await loadToken()).toBeNull();
      expect(location.hash).toBe("");
    }
  });
  it("ignores a token that is not a token", async () => {
    sessionStorage.setItem(NONCE_KEY, "n");
    location.hash = "#token=nonsense";
    await takeTokenFromUrl("mine");
    expect(await loadToken()).toBeNull();
  });
  it("lets an address with no login alone", async () => {
    sessionStorage.setItem(NONCE_KEY, "n");
    location.hash = "#/m/abc";
    await takeTokenFromUrl("mine");
    expect(location.hash).toBe("#/m/abc");
    expect(sessionStorage.getItem(NONCE_KEY)).toBe("n");
  });
});
