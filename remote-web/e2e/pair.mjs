// End to end, with no window: the web app's own keys, Relay, MachineSession and
// the wasm Noise session talk through a running `wrangler dev` relay to the
// app's own link and Noise code (src-tauri/examples/remote_machine.rs).
//
//   1. relay/    npm run dev                       (needs DEV_LOGIN=1 in .dev.vars)
//   2. src-tauri cargo build --example remote_machine
//   3. remote-web node e2e/pair.mjs
//
// The machine is started by this script. BYTEHIVE_RELAY (default
// http://127.0.0.1:18787) and MACHINE_EXE override the relay and the binary.

import "fake-indexeddb/auto";
import { readFileSync } from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { createServer } from "vite";
import { RELAY, RELAY_WS, check, root, sleep, startMachine, waitFor, web } from "./lib.mjs";

const ACCOUNT = `e2e-${Date.now().toString(36)}`;

const FLEET = {
  type: "fleet.state",
  agents: [{ id: "a1", name: "claude", project: "demo", projectId: "p1", status: "working", title: "Fixing the build", hasChat: true, cols: 80, rows: 24 }],
  projects: [{ id: "p1", name: "demo" }],
  presets: [{ id: "claude", label: "Claude" }],
};

// What a browser has and Node does not.
globalThis.window = globalThis;
process.env.VITE_RELAY = RELAY;
process.env.VITE_RELAY_WS = RELAY_WS;

// The web app's real modules, through Vite so TypeScript and import.meta.env work.
const vite = await createServer({ root: web, configFile: path.join(web, "vite.config.ts"), logLevel: "error", server: { middlewareMode: true }, appType: "custom" });
const { Relay, MachineSession } = await vite.ssrLoadModule("/src/session.ts");
const { loadDevice, machines, saveMachine } = await vite.ssrLoadModule("/src/keys.ts");

const pkg = path.join(root, "remote", "noise-wasm", "pkg");
const wasm = await import(pathToFileURL(path.join(pkg, "bytehive_noise_wasm.js")).href);
wasm.initSync({ module: readFileSync(path.join(pkg, "bytehive_noise_wasm_bg.wasm")) });

let machine = null;
const cleanup = async () => {
  machine?.stop();
  await vite.close();
};

/** A device signs in the way the page does: the redirect carries the token in its fragment. */
async function signIn(deviceId) {
  const r = await fetch(`${RELAY}/auth/start?peer=d:${deviceId}&account=${ACCOUNT}`, { redirect: "manual" });
  const to = r.headers.get("location") ?? "";
  const m = to.match(/#token=(.+)$/);
  if (!m) throw new Error(`no token from the relay (${r.status} ${to})`);
  return decodeURIComponent(m[1]);
}

/** A device on the relay: the page's own Relay class, routed to its sessions. */
async function connectDevice(deviceId, deviceName, machineId) {
  const relay = new Relay({ token: await signIn(deviceId), deviceName });
  const sessions = new Map();
  relay.onFrame = (f) => sessions.get(f.peer)?.frame(f);
  await relay.connect();
  await waitFor(() => relay.peers.some((p) => p.id === `m:${machineId}` && p.online), "the machine to show online");
  return { relay, sessions };
}

/** What main.ts does to open one machine, with the messages kept for the checks. */
function open({ relay, sessions }, device, machineId, { code, expectKey } = {}) {
  const peer = `m:${machineId}`;
  const noise = new wasm.Session(device.private, code);
  const s = new MachineSession({ peer, noise, pairing: !!code, expectKey, deviceName: device.name, sendFrame: (b) => relay.send(b) });
  const got = { msgs: [], closed: null, noise };
  s.onMessage = (m) => got.msgs.push(m);
  s.onClosed = (why) => { got.closed = why; };
  s.onOpen = () => s.send({ type: "fleet.get" });
  sessions.set(peer, s);
  s.start();
  return { s, got };
}
const message = (got, type) => got.msgs.find((m) => m.type === type);

async function main() {
  const m = (machine = await startMachine(ACCOUNT));
  console.log(`machine ${m.id} (${m.name}) is on ${RELAY}, account ${ACCOUNT}`);

  // This browser's identity: the real keys.ts (WebCrypto wrapping, IndexedDB).
  const me = await loadDevice(wasm);
  const raw = await loadDevice(wasm);
  check(me.id === raw.id && me.private.length === 32, `the device keeps one identity (${me.id})`);

  // 1. First pairing, with the code.
  const link1 = await connectDevice(me.id, me.name, m.id);
  const first = open(link1, me, m.id, { code: m.firstCode });
  const hello = await waitFor(() => message(first.got, "hello"), "hello after pairing");
  check(hello.protocol === 1 && hello.machine === m.name, `pairing with the code gives hello from "${hello.machine}"`);
  check(first.s.machine?.id === m.id, "the session knows the machine id from the handshake");
  check(first.got.noise.confirmed === true, "the machine's key is confirmed only once a transport message decrypted");
  const key = first.s.remoteKey;
  check(key?.length === 32, "the machine's static key is known");
  await saveMachine({ id: m.id, name: first.s.machine.name, key });
  check((await machines())[0]?.name === m.name, "the paired machine is remembered");
  const fleet = await waitFor(() => message(first.got, "fleet.state"), "fleet.state after fleet.get");
  check(JSON.stringify(fleet) === JSON.stringify(FLEET), "fleet.get is answered with the machine's fleet.state, unchanged");
  link1.relay.stop();

  // 2. Back again, no code, the key pinned.
  const link2 = await connectDevice(me.id, me.name, m.id);
  const pinned = (await machines())[0];
  const again = open(link2, me, m.id, { expectKey: pinned.key });
  await waitFor(() => message(again.got, "hello"), "hello on reconnect");
  await waitFor(() => message(again.got, "fleet.state"), "fleet.state on reconnect");
  check(true, "reconnecting without a code gives hello and fleet.state again");
  check(again.got.closed === null, "the reconnected session stayed open");

  // 2b. A machine whose key is not the pinned one is hung up on before message 3.
  link2.sessions.delete(`m:${m.id}`);
  const other = open(link2, me, m.id, { expectKey: new Uint8Array(32).fill(7) });
  await waitFor(() => other.got.closed, "a hang-up on the wrong key");
  check(other.got.closed === "wrong-key" && other.got.msgs.length === 0, "a machine with another key than the pinned one gets no message 3");
  link2.relay.stop();

  // 3. A wrong code: the handshake runs, nothing opens.
  const kp = wasm.generateKeys();
  const stranger = { private: kp.private, id: [...crypto.getRandomValues(new Uint8Array(8))].map((b) => b.toString(16).padStart(2, "0")).join(""), name: "Stranger" };
  const code = await m.newCode();
  const wrong = code.slice(0, -1) + (code.endsWith("A") ? "B" : "A");
  const link3 = await connectDevice(stranger.id, stranger.name, m.id);
  const bad = open(link3, { private: stranger.private, name: stranger.name }, m.id, { code: wrong });
  await sleep(3000);
  check(bad.got.msgs.length === 0 && !message(bad.got, "hello"), "a wrong code gets no hello");

  // 3b. The machine is not stuck by it, and the spent code stays spent.
  link3.sessions.clear();
  const reuse = open(link3, { private: stranger.private, name: stranger.name }, m.id, { code });
  await sleep(2000);
  check(reuse.got.msgs.length === 0, "the code a wrong try spent cannot be used again");
  const fresh = await m.newCode();
  link3.sessions.clear();
  const good = open(link3, { private: stranger.private, name: stranger.name }, m.id, { code: fresh });
  await waitFor(() => message(good.got, "hello"), "hello for a second device with a new code");
  check(true, "a new code pairs a second device after the failed tries");
  link3.relay.stop();
}

let failed = false;
try {
  await main();
  console.log("PASS");
} catch (e) {
  failed = true;
  console.error(String(e?.stack ?? e));
} finally {
  await cleanup();
}
process.exit(failed ? 1 : 0);
