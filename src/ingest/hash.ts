/** SHA-256 of a message, as the cache key. Web Crypto is in the Tauri webview
 *  and in Node, so the same key works in the app and in tests. */
export async function sha256(text: string): Promise<string> {
  const bytes = new TextEncoder().encode(text);
  const digest = await crypto.subtle.digest("SHA-256", bytes);
  return [...new Uint8Array(digest)].map((b) => b.toString(16).padStart(2, "0")).join("");
}
