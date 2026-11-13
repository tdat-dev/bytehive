import { beforeEach, describe, expect, it } from "vitest";
import { forgetDevice, loadDevice, machines, resetForTest, saveMachine, storedDeviceId } from "./keys";

const noise = { generateKeys: () => ({ private: new Uint8Array(32).fill(1), public: new Uint8Array(32).fill(2) }) };

describe("device identity", () => {
  beforeEach(resetForTest);

  it("makes one identity and keeps it", async () => {
    const a = await loadDevice(noise);
    const b = await loadDevice(noise);
    expect(b.id).toBe(a.id);
    expect(b.private).toEqual(a.private);
  });
  it("stores the private key wrapped, not as it is", async () => {
    await loadDevice(noise);
    const raw = await resetForTest.peek("device");
    expect(raw.private).toBeUndefined();
    expect(raw.wrapped).toBeInstanceOf(ArrayBuffer);
  });
  it("remembers paired machines", async () => {
    await saveMachine({ id: "m1", name: "Desk", key: new Uint8Array([1]) });
    expect((await machines())[0].name).toBe("Desk");
  });
  it("forgets the identity and the machines it paired, which a new identity could not talk to", async () => {
    const noiseB = { generateKeys: () => ({ private: new Uint8Array(32).fill(3), public: new Uint8Array(32).fill(4) }) };
    const a = await loadDevice(noise);
    await saveMachine({ id: "m1", name: "Desk", key: new Uint8Array([1]) });
    await forgetDevice();
    const b = await loadDevice(noiseB);
    expect(b.id).not.toBe(a.id);
    expect(b.private).toEqual(new Uint8Array(32).fill(3));
    expect(await machines()).toEqual([]);
  });
  it("tells which identity is stored, or none", async () => {
    expect(await storedDeviceId()).toBeNull();
    const a = await loadDevice(noise);
    expect(await storedDeviceId()).toBe(a.id);
    await forgetDevice();
    expect(await storedDeviceId()).toBeNull();
  });
});
