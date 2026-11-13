// The session store's keys, the browser side of native/src/cloud/crypto.rs.
// The passphrase never leaves this page and the key it makes cannot be exported.

const te = new TextEncoder();
const CHECK = "bytehive-remote-v1";
const CHECK_SLOT = "vault/check";

export const unb64 = (s: string): Uint8Array => {
  const b = atob(s.replace(/-/g, "+").replace(/_/g, "/"));
  return Uint8Array.from(b, (c) => c.charCodeAt(0));
};

/** PBKDF2-HMAC-SHA256, then HKDF-SHA256 with no salt and the "enc" label: the sealing key's bits. */
export async function encBits(passphrase: string, salt: Uint8Array, rounds: number): Promise<ArrayBuffer> {
  const base = await crypto.subtle.importKey("raw", te.encode(passphrase), "PBKDF2", false, ["deriveBits"]);
  const master = await crypto.subtle.deriveBits({ name: "PBKDF2", hash: "SHA-256", salt: salt as BufferSource, iterations: rounds }, base, 256);
  const hk = await crypto.subtle.importKey("raw", master, "HKDF", false, ["deriveBits"]);
  return crypto.subtle.deriveBits({ name: "HKDF", hash: "SHA-256", salt: new Uint8Array(0), info: te.encode("bytehive remote enc") }, hk, 256);
}

/** The key that opens what the machine sealed. It can only decrypt, and never be read out. */
export async function deriveKey(passphrase: string, salt: Uint8Array, rounds: number): Promise<CryptoKey> {
  return crypto.subtle.importKey("raw", await encBits(passphrase, salt, rounds), "AES-GCM", false, ["decrypt"]);
}

/** The plain bytes of a sealed blob in `slot`, or null for a wrong key, slot or damage. */
export async function open(key: CryptoKey, slot: string, sealed: string): Promise<Uint8Array | null> {
  try {
    const raw = unb64(sealed);
    if (raw.length < 1 + 12 + 16 || raw[0] !== 1) return null;
    const plain = await crypto.subtle.decrypt(
      { name: "AES-GCM", iv: raw.subarray(1, 13) as BufferSource, additionalData: te.encode(slot) },
      key,
      raw.subarray(13) as BufferSource,
    );
    return new Uint8Array(plain);
  } catch {
    return null;
  }
}

export async function openJson<T>(key: CryptoKey, slot: string, sealed: string): Promise<T | null> {
  const plain = await open(key, slot, sealed);
  if (!plain) return null;
  try {
    return JSON.parse(new TextDecoder().decode(plain)) as T;
  } catch {
    return null;
  }
}

/** Whether this key opens the vault's check blob: the passphrase was right. */
export async function checks(key: CryptoKey, check: string): Promise<boolean> {
  const plain = await open(key, CHECK_SLOT, check);
  return !!plain && new TextDecoder().decode(plain) === CHECK;
}
