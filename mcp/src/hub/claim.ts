/* Finding a hub, or becoming one. ByteHive's hub, when it answers, always wins:
 * a standalone hub starts only when the installed hub file names nobody, and
 * steps down as soon as the file names another hub that answers (the app wrote
 * it when it opened). A lock file keeps two bytehive-mcp processes from starting
 * a hub at the same moment; it is held only while one starts. */

import fs from "node:fs";
import net from "node:net";
import os from "node:os";
import path from "node:path";
import { Hub } from "./hub.js";

export type HubAddr = { port: number; token: string };

const WATCH_MS = 2_000;
/** A lock older than this was left by a process that died while starting. */
const STALE_LOCK_MS = 10_000;
const WAIT_FOR_OTHER_MS = 3_000;
export const TOOK_OVER = "ByteHive just opened and took over the browser; try your step again.";

export function installedHubFile(home: string = os.homedir()): string {
  return path.join(home, ".bytehive", "browser-hub.json");
}

export function readHubFile(file: string): HubAddr | null {
  try {
    const v = JSON.parse(fs.readFileSync(file, "utf8"));
    return typeof v.port === "number" && typeof v.token === "string" ? { port: v.port, token: v.token } : null;
  } catch {
    return null;
  }
}

/** Something listens on this local port. */
export function answers(port: number, ms = 500): Promise<boolean> {
  return new Promise((resolve) => {
    const s = net.connect({ host: "127.0.0.1", port });
    const done = (ok: boolean) => {
      clearTimeout(t);
      s.destroy();
      resolve(ok);
    };
    const t = setTimeout(() => done(false), ms);
    s.once("connect", () => done(true));
    s.once("error", () => done(false));
  });
}

function alive(pid: number): boolean {
  try {
    process.kill(pid, 0);
    return true;
  } catch (e) {
    return (e as NodeJS.ErrnoException).code === "EPERM";
  }
}

/** Take the start lock; a lock left by a dead or long-gone starter is removed. */
export function takeLock(lock: string): boolean {
  fs.mkdirSync(path.dirname(lock), { recursive: true });
  for (let i = 0; i < 2; i++) {
    try {
      const fd = fs.openSync(lock, "wx");
      fs.writeSync(fd, String(process.pid));
      fs.closeSync(fd);
      return true;
    } catch (e) {
      if ((e as NodeJS.ErrnoException).code !== "EEXIST") return false;
      try {
        const pid = Number(fs.readFileSync(lock, "utf8"));
        const old = Date.now() - fs.statSync(lock).mtimeMs > STALE_LOCK_MS;
        if (!old && pid && alive(pid)) return false;
        fs.unlinkSync(lock);
      } catch {
        /* gone already, or another process is removing it: try again */
      }
    }
  }
  return false;
}

export function releaseLock(lock: string) {
  try {
    if (fs.readFileSync(lock, "utf8") === String(process.pid)) fs.unlinkSync(lock);
  } catch {
    /* not ours or gone */
  }
}

function publish(file: string, hub: Hub) {
  const body = JSON.stringify({ port: hub.port, token: hub.token, pid: process.pid, standalone: true });
  fs.mkdirSync(path.dirname(file), { recursive: true });
  const tmp = `${file}.${process.pid}.tmp`;
  try {
    fs.writeFileSync(tmp, body);
    fs.renameSync(tmp, file);
  } catch {
    try { fs.unlinkSync(tmp); } catch { /* never written */ }
    fs.writeFileSync(file, body);
  }
}

/** The hub this process runs, if any. */
let own: { hub: Hub; timer: NodeJS.Timeout } | null = null;

export function ownHub(): Hub | null {
  return own && !own.hub.stopped ? own.hub : null;
}

/** Run a hub here and publish it in the installed hub file. */
export async function startOwn(file: string, opts: { grace?: number; hooks?: boolean } = {}): Promise<Hub> {
  const hub = new Hub(opts.grace);
  await hub.start();
  publish(file, hub);
  const timer = setInterval(async () => {
    if (hub.stopped) return clearInterval(timer);
    const named = readHubFile(file);
    if (named?.port === hub.port) return;
    if (named && (await answers(named.port))) {
      clearInterval(timer);
      if (own?.hub === hub) own = null;
      await hub.stop(TOOK_OVER);
    } else publish(file, hub); // gone or dead: ours again
  }, WATCH_MS);
  timer.unref();
  own = { hub, timer };
  if (opts.hooks !== false) {
    process.once("exit", () => {
      if (!hub.stopped && readHubFile(file)?.port === hub.port) {
        try { fs.unlinkSync(file); } catch { /* best effort */ }
      }
    });
    // The client closed our stdio: the session is over. Without this the open
    // server would keep the process, and the user's browser, forever.
    process.stdin.once("end", () => process.exit(0));
  }
  return hub;
}

/** The hub to dial: the one the hub file names if it answers, else (when this
 *  agent may) a standalone hub, ours or one another bytehive-mcp just started. */
export async function locateHub(named: string, opts: { mayStart: boolean; file?: string; grace?: number; hooks?: boolean }): Promise<HubAddr> {
  const hub = readHubFile(named);
  if (hub && (await answers(hub.port))) return hub;
  if (!opts.mayStart) throw new Error("The ByteHive that started this agent is closed, so there is no browser to use. Open it again and try again.");
  const mine = ownHub();
  if (mine) return { port: mine.port, token: mine.token };
  const file = opts.file ?? installedHubFile();
  const lock = path.join(path.dirname(file), "browser-hub.lock");
  const until = Date.now() + WAIT_FOR_OTHER_MS;
  for (;;) {
    if (takeLock(lock)) {
      try {
        const again = readHubFile(file);
        if (again && (await answers(again.port))) return again;
        const h = await startOwn(file, opts);
        return { port: h.port, token: h.token };
      } finally {
        releaseLock(lock);
      }
    }
    const other = readHubFile(file);
    if (other && (await answers(other.port))) return other;
    if (Date.now() > until) throw new Error("Another agent is starting the browser hub and has not finished. Try again in a moment.");
    await new Promise((r) => setTimeout(r, 200));
  }
}
