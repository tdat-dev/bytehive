# Remote session store: read any session from any machine

Date: 2026-11-13. Status: owner decisions taken in chat, building.
Extends `2026-11-05-bytehive-remote-design.md` (section 7.3, "an encrypted
copy"), now on by default and built in the native app.

## Owner decisions (2026-11-13)

- When Remote is on, every session in the open projects is pushed as an
  encrypted copy, like Claude Code remote.
- Another machine reads them at remote.zoldify.com after a GitHub sign-in and
  the Remote passphrase, typed once and remembered on that device for 15 days.
- The server never holds a key and cannot read a session.
- Relay at relay.zoldify.com, web at remote.zoldify.com, on the owner's
  Cloudflare.

## What is stored

A session is stored as chat items, not as the CLI's raw file: what you typed,
the agent's replies, and one line per step ("Ran cargo test", failed). The
native reader already builds these for Claude (`native/src/transcript`). This
is smaller, reads the same for every CLI, and leaves tool output (file
contents, command output, secrets in logs) on the machine.

Items are cut into segments of 200. A segment is sealed and stored under its
number. Only segments that changed are sent again; in practice the last one or
two.

Each session also has a sealed header: title (the first ask), CLI, project
folder name, machine name, agent name, started, updated, item count.

## Keys

- The passphrase goes through PBKDF2-HMAC-SHA256, 600,000 rounds, with a
  random 16-byte salt kept per account on the relay. HKDF-SHA256 splits the
  result in two: an encryption key and an id key.
- Sealing is AES-256-GCM with a random 12-byte nonce. A sealed blob is
  `1 | nonce | ciphertext and tag`, base64url in JSON. Its additional data is
  the slot it belongs in (`<sid>/meta` or `<sid>/<n>`), so the relay cannot
  move a segment to another session or place.
- A session's id on the relay is `HMAC(id key, machine:cli:session)` cut to 32
  hex characters: stable for the machine, meaningless to the relay.
- The relay keeps a check blob (a known text sealed with the key), so a wrong
  passphrase is told at once instead of showing garbage.
- On the machine, the derived key (never the passphrase) is kept in a file
  protected by Windows DPAPI for the current user.
- In the browser, the encryption key is a non-extractable WebCrypto key in
  IndexedDB with an expiry 15 days out; after that the passphrase is asked
  again. "Forget this device" deletes it at once.
- Changing the passphrase makes a new salt and key; the machine seals and sends
  everything again, and other devices must type the new one.

## Relay

Plain HTTPS beside the existing socket routes, with the existing login token
(`Authorization: Bearer`). A machine is a peer `m:...`, a device `d:...`.

| Route | Who | What |
|-------|-----|------|
| `GET /vault` | any | salt, rounds, check blob; 404 before one is set |
| `PUT /vault` | machine | set them; replacing needs `replace: true` and drops every stored session |
| `POST /store/put` | machine | `{ sid, meta, updated, segments: [{ n, sealed }] }` |
| `POST /store/drop` | machine | `{ sid }` |
| `GET /store/list` | any | every session: sid, machine, meta, updated, segment count |
| `GET /store/segments?sid=&from=` | any | sealed segments from `from`, up to about 2 MB a page |

Stored in the account's Durable Object SQLite: `vault`, `sessions (sid,
machine, meta, updated, bytes)`, `segments (sid, n, sealed)`. Limits per
account: a sealed segment up to 512 KB, a header up to 8 KB, 1 GB in all;
over that the put is refused with `quota` and the app says so. A revoked
machine or device gets 403 here as on the socket.

## Native app

- `native/src/cloud/`: `crypto.rs` (derive, seal, open), `relay.rs` (sign in
  through the loopback port as the Tauri app did, HTTP calls), `sync.rs` (the
  loop), `state.rs` (what was sent: segment hashes per session).
- The loop runs off the UI thread every 20 s while Remote is on: for every
  agent with a session in an open project it reads the transcript, builds the
  items, seals the changed segments and the header, and sends them. A failure
  waits and tries again; nothing blocks the app.
- Settings, Remote: the switch, Sign in with GitHub, Set passphrase (typed
  twice, with a line that says a forgotten passphrase loses what is stored),
  and a status line ("12 sessions saved, last 30 s ago").
- First CLI: Claude. Codex, OpenCode and Antigravity follow as their readers
  learn to build the same items.

## Web

`remote.zoldify.com`: Sign in with GitHub, then the passphrase (once per 15
days), then machines and their sessions, newest first, each opening as a chat
read only. Live control (sending tasks, the terminal) stays the paired-device
path of the original spec.

## Not in this step

Sending messages from the web, the live terminal, moving a session between
machines, sharing with other people.

## Before deploy (owner)

- `! npx wrangler login` once, with zoldify.com in that Cloudflare account.
- A GitHub OAuth app with callback `https://relay.zoldify.com/auth/callback`;
  its id and secret go in as Worker secrets with `TOKEN_SECRET` and the VAPID
  keys.
