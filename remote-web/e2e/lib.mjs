// Shared by pair.mjs and browser.mjs: the machine example, and small waiting helpers.

import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import path from "node:path";
import { createInterface } from "node:readline";
import { fileURLToPath } from "node:url";

export const web = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
export const root = path.resolve(web, "..");
export const RELAY = process.env.BYTEHIVE_RELAY ?? "http://127.0.0.1:18787";
export const RELAY_WS = RELAY.replace(/^http/, "ws");
/** Where the page is served. The relay sends a login back to its APP_ORIGIN, which must match. */
export const APP_PORT = Number(process.env.APP_PORT ?? 17640);
const EXE = process.env.MACHINE_EXE ?? path.join(root, "src-tauri", "target", "debug", "examples", "remote_machine.exe");

export const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

export const check = (cond, what) => {
  if (!cond) throw new Error(`FAILED: ${what}`);
  console.log(`ok   ${what}`);
};

export async function waitFor(pred, what, ms = 10_000) {
  const until = Date.now() + ms;
  while (Date.now() < until) {
    const v = pred();
    if (v) return v;
    await sleep(20);
  }
  throw new Error(`timed out waiting for ${what}`);
}

/** The machine: JSON lines out (`ready`, `code`, `term`, `chat`, `prompt`), the lines `invite` and `send <json>` in. */
export async function startMachine(account) {
  if (!existsSync(EXE)) throw new Error(`no ${EXE}: run "cargo build --example remote_machine" in src-tauri`);
  const child = spawn(EXE, [], { env: { ...process.env, BYTEHIVE_RELAY: RELAY, REMOTE_E2E_ACCOUNT: account }, stdio: ["pipe", "pipe", "inherit"] });
  const events = [];
  createInterface({ input: child.stdout }).on("line", (l) => { try { events.push(JSON.parse(l)); } catch { /* not ours */ } });
  child.on("exit", (code) => events.push({ event: "exit", code }));
  const ready = await waitFor(() => events.find((e) => e.event === "ready" || e.event === "exit"), "the machine to be ready", 30_000);
  if (ready.event === "exit") throw new Error(`the machine exited with ${ready.code}`);
  return {
    id: ready.machine,
    name: ready.name,
    firstCode: ready.code,
    /** The terminal calls the machine has heard, in order: `open a1`, `resize a1 100x30`, `close a1`. */
    terms: () => events.filter((e) => e.event === "term").map((e) => `${e.op.slice(5)} ${e.agent}${e.op === "term.resize" ? ` ${e.cols}x${e.rows}` : ""}`),
    /** How many key bytes the machine has heard from the page, never which. */
    keyBytes: () => events.filter((e) => e.event === "keys").reduce((n, e) => n + e.n, 0),
    /** The chat calls the machine has heard, in order. `n` is how long a message was, never its words. */
    chats: () => events.filter((e) => e.event === "chat").map((e) => ({ op: e.op.slice(5), agent: e.agent, id: e.id, n: e.n, before: e.before })),
    /** The prompt answers the machine has heard: the id of the question and the key. */
    prompts: () => events.filter((e) => e.event === "prompt").map((e) => ({ id: e.id, keys: e.keys })),
    /** Has the machine leave a sealed note for each paired device that is not online, as the app does when an agent needs you. */
    note(title, body) {
      child.stdin.write(`note ${title}|${body}\n`);
    },
    /** Has the machine say a message of its own to every device online. */
    say(msg) {
      child.stdin.write(`send ${JSON.stringify(msg)}\n`);
    },
    async newCode() {
      const seen = events.filter((e) => e.event === "code").length;
      child.stdin.write("invite\n");
      const e = await waitFor(() => events.filter((x) => x.event === "code")[seen], "a new code");
      return e.code;
    },
    /** Ends this machine only, by the pid this script started. */
    stop() {
      child.stdin.end();
      setTimeout(() => child.kill(), 1500).unref();
    },
  };
}
