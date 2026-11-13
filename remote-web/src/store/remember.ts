// The store key, kept on this device for 15 days so the passphrase is typed once.
// It is a WebCrypto key that cannot be exported: a copy of the database gives
// someone a handle that only works in this browser, never the key itself.

const DB = "bytehive-store";
const KV = "kv";
export const REMEMBER_MS = 15 * 24 * 3600 * 1000;

interface Kept { key: CryptoKey; until: number; account: string }

function db(): Promise<IDBDatabase> {
  return new Promise((res, rej) => {
    const r = indexedDB.open(DB, 1);
    r.onupgradeneeded = () => r.result.createObjectStore(KV);
    r.onsuccess = () => res(r.result);
    r.onerror = () => rej(r.error);
  });
}

async function run<T>(mode: IDBTransactionMode, f: (s: IDBObjectStore) => IDBRequest<T>): Promise<T> {
  const d = await db();
  return new Promise((res, rej) => {
    const r = f(d.transaction(KV, mode).objectStore(KV));
    r.onsuccess = () => res(r.result);
    r.onerror = () => rej(r.error);
  });
}

/** Keep `key` for `account` until 15 days from now. */
export async function remember(key: CryptoKey, account: string, now = Date.now()): Promise<void> {
  await run("readwrite", (s) => s.put({ key, until: now + REMEMBER_MS, account } satisfies Kept, "key"));
}

/** The kept key for `account`, or null once it is past its 15 days (it is then forgotten). */
export async function recall(account: string, now = Date.now()): Promise<CryptoKey | null> {
  const k = await run<Kept | undefined>("readonly", (s) => s.get("key"));
  if (!k) return null;
  if (k.until <= now || k.account !== account) {
    await forget();
    return null;
  }
  return k.key;
}

export async function forget(): Promise<void> {
  await run("readwrite", (s) => s.delete("key"));
}
