// Gives a local run (`npm run dev`) the VAPID key pair Web Push needs, in .dev.vars,
// which git ignores. Does nothing if a key is already there. For the deployed Worker
// make a pair the same way and keep it with `wrangler secret put VAPID_PRIVATE_JWK`;
// the private half is never committed.
//
//   node vapid-keys.mjs

import { appendFileSync, existsSync, readFileSync } from "node:fs";

const FILE = new URL("./.dev.vars", import.meta.url);
const has = (name) => existsSync(FILE) && new RegExp(`^${name}=.+`, "m").test(readFileSync(FILE, "utf8"));

if (has("VAPID_PRIVATE_JWK") && has("VAPID_PUBLIC")) {
  console.log("VAPID key already in .dev.vars");
} else {
  const pair = await crypto.subtle.generateKey({ name: "ECDSA", namedCurve: "P-256" }, true, ["sign", "verify"]);
  const b64url = (b) => Buffer.from(b).toString("base64url");
  const lines = [
    `VAPID_PRIVATE_JWK=${JSON.stringify(await crypto.subtle.exportKey("jwk", pair.privateKey))}`,
    `VAPID_PUBLIC=${b64url(await crypto.subtle.exportKey("raw", pair.publicKey))}`,
    ...(has("VAPID_SUBJECT") ? [] : ["VAPID_SUBJECT=mailto:dev@localhost.invalid"]),
  ];
  appendFileSync(FILE, `\n${lines.join("\n")}\n`);
  console.log("VAPID key added to .dev.vars");
}
