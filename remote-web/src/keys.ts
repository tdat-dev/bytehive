// This browser's identity. The private key is kept wrapped by an AES key that
// WebCrypto won't export, so a copy of the database alone doesn't carry it.

const DB = "bytehive-remote";
const STORE = "kv";

export interface DeviceIdentity { id: string; name: string; private: Uint8Array; public: Uint8Array }
export interface PairedMachine { id: string; name: string; key: Uint8Array; vault?: Uint8Array }
interface Noise { generateKeys(): { private: Uint8Array; public: Uint8Array } }

function db(): Promise<IDBDatabase> {
  return new Promise((res, rej) => {
    const r = indexedDB.open(DB, 1);
    r.onupgradeneeded = () => r.result.createObjectStore(STORE);
    r.onsuccess = () => res(r.result);
    r.onerror = () => rej(r.error);
  });
}
async function get<T>(key: string): Promise<T | undefined> {
  const d = await db();
  return new Promise((res, rej) => {
    const r = d.transaction(STORE).objectStore(STORE).get(key);
    r.onsuccess = () => res(r.result as T | undefined);
    r.onerror = () => rej(r.error);
  });
}
async function put(key: string, value: unknown): Promise<void> {
  const d = await db();
  return new Promise((res, rej) => {
    const t = d.transaction(STORE, "readwrite");
    t.objectStore(STORE).put(value, key);
    t.oncomplete = () => res();
    t.onerror = () => rej(t.error);
  });
}

interface Stored { id: string; name: string; public: Uint8Array; wrapped: ArrayBuffer; iv: Uint8Array; wrapKey: CryptoKey }

function deviceName(): string {
  const ua = navigator.userAgent;
  const os = /iPhone|iPad/.test(ua) ? "iPhone" : /Android/.test(ua) ? "Android" : /Mac/.test(ua) ? "Mac" : /Windows/.test(ua) ? "Windows" : "Browser";
  return `${os} browser`;
}

export async function loadDevice(noise: Noise): Promise<DeviceIdentity> {
  const s = await get<Stored>("device");
  if (s) {
    const plain = await crypto.subtle.decrypt({ name: "AES-GCM", iv: s.iv as BufferSource }, s.wrapKey, s.wrapped);
    return { id: s.id, name: s.name, public: s.public, private: new Uint8Array(plain) };
  }
  const k = noise.generateKeys();
  const wrapKey = await crypto.subtle.generateKey({ name: "AES-GCM", length: 256 }, false, ["encrypt", "decrypt"]);
  const iv = crypto.getRandomValues(new Uint8Array(12));
  const wrapped = await crypto.subtle.encrypt({ name: "AES-GCM", iv }, wrapKey, k.private as BufferSource);
  const id = [...crypto.getRandomValues(new Uint8Array(8))].map((b) => b.toString(16).padStart(2, "0")).join("");
  const name = deviceName();
  await put("device", { id, name, public: k.public, wrapped, iv, wrapKey } satisfies Stored);
  return { id, name, public: k.public, private: k.private };
}

/** The id of the identity in storage now, which another tab may have replaced. */
export async function storedDeviceId(): Promise<string | null> {
  return (await get<Stored>("device"))?.id ?? null;
}

/**
 * A removed device starts over. The removal is account-wide at the relay, so
 * every machine needs a new code for a new identity: the old pins and vaults
 * could only dial machines that would no longer answer.
 */
export async function forgetDevice(): Promise<void> {
  await put("device", undefined);
  await put("machines", undefined);
}

export async function machines(): Promise<PairedMachine[]> {
  return (await get<PairedMachine[]>("machines")) ?? [];
}
export async function saveMachine(m: PairedMachine): Promise<void> {
  await put("machines", [...(await machines()).filter((x) => x.id !== m.id), m]);
}
export async function saveVault(id: string, vault: Uint8Array): Promise<void> {
  const m = (await machines()).find((x) => x.id === id);
  if (m) await saveMachine({ ...m, vault });
}
export async function saveToken(t: string | null): Promise<void> { await put("token", t); }
export async function loadToken(): Promise<string | null> { return (await get<string | null>("token")) ?? null; }

/** Tests only. */
export const resetForTest = Object.assign(
  async () => { await put("device", undefined); await put("machines", undefined); },
  { peek: (k: string) => get<any>(k) },
);
