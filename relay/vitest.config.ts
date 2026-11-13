import { cloudflareTest } from "@cloudflare/vitest-plugin";
import { defineConfig } from "vitest/config";

// A key pair made for this run: nothing signed in a test is worth keeping, so no key is committed.
const pair = await crypto.subtle.generateKey({ name: "ECDSA", namedCurve: "P-256" }, true, ["sign", "verify"]);
const b64url = (b: ArrayBuffer) => btoa(String.fromCharCode(...new Uint8Array(b))).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");

export default defineConfig({
  plugins: [
    cloudflareTest({
      wrangler: { configPath: "./wrangler.jsonc" },
      miniflare: {
        bindings: {
          TOKEN_SECRET: "test-secret-test-secret-test-secret",
          GITHUB_CLIENT_ID: "id",
          GITHUB_CLIENT_SECRET: "secret",
          DEV_LOGIN: "1",
          VAPID_PRIVATE_JWK: JSON.stringify(await crypto.subtle.exportKey("jwk", pair.privateKey)),
          VAPID_PUBLIC: b64url(await crypto.subtle.exportKey("raw", pair.publicKey)),
          VAPID_SUBJECT: "mailto:test@example.com",
        },
      },
    }),
  ],
});
