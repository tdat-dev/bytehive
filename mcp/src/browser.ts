/* Browser tools for bytehive-mcp: the agent's side of "ByteHive for Chrome".
 *
 * Calls go over one WebSocket to the hub inside the ByteHive app (port and
 * token in ~/.bytehive/browser-hub.json). The hub routes each call to the
 * browser this agent picked, where the extension runs it in the agent's own
 * tab group. Results come back already MCP-shaped ({ content: [...] }). */

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { z } from "zod";

type Content = { type: "text"; text: string } | { type: "image"; data: string; mimeType: string };
type RecordFile = { name: string; size: number };
export type ToolResult = { content: Content[]; isError?: boolean; gif?: string; part?: string; record?: { files: RecordFile[] } };

/** The hub of the ByteHive that started this agent (it sets BYTEHIVE_HUB_FILE),
 *  else the installed ByteHive's. Guessing "the last ByteHive opened" sent agents
 *  to a test build's hub, and to a dead port once it closed. */
export function hubFile(home: string = os.homedir()): string {
  const env = process.env.BYTEHIVE_HUB_FILE || process.env.MAESTRO_HUB_FILE;
  if (env) return env;
  const now = path.join(home, ".bytehive", "browser-hub.json");
  const old = path.join(home, ".maestro", "browser-hub.json");
  // An app from before the rename (Maestro) still publishes the old one.
  return !fs.existsSync(now) && fs.existsSync(old) ? old : now;
}

const CALL_TIMEOUT_MS = 90_000;
const CONNECT_TIMEOUT_MS = 5_000;
/** How long a click held for the user's OK may wait. */
const HOLD_TIMEOUT_MS = 10 * 60_000;

export class HubClient {
  private ws: WebSocket | null = null;
  private ready: Promise<void> | null = null;
  private next = 1;
  private waiting = new Map<number, { resolve: (r: ToolResult) => void; timer: NodeJS.Timeout; tool: string }>();

  constructor(
    private agent: string,
    private readHub: () => { port: number; token: string } = () => JSON.parse(fs.readFileSync(hubFile(), "utf8")),
  ) {}

  private open(): Promise<void> {
    if (this.ready) return this.ready;
    this.ready = new Promise<void>((resolve, reject) => {
      let hub: { port: number; token: string };
      try {
        hub = this.readHub();
      } catch {
        return reject(new Error("ByteHive isn't running, so there is no browser to use. Open ByteHive and try again."));
      }
      if (typeof WebSocket === "undefined") return reject(new Error("Browser tools need Node 22 or newer."));
      const ws = new WebSocket(`ws://127.0.0.1:${hub.port}/`);
      const slow = setTimeout(() => {
        ws.close();
        drop("ByteHive did not answer.");
        reject(new Error("ByteHive did not answer. Is it open?"));
      }, CONNECT_TIMEOUT_MS);
      const drop = (why: string) => {
        this.ws = null;
        this.ready = null;
        for (const [, w] of this.waiting) {
          clearTimeout(w.timer);
          w.resolve(err(why));
        }
        this.waiting.clear();
      };
      ws.addEventListener("open", () => {
        clearTimeout(slow);
        ws.send(JSON.stringify({ type: "hello", role: "agent", token: hub.token, agent: this.agent }));
        this.ws = ws;
        resolve();
      });
      ws.addEventListener("message", (e) => this.onMessage(String(e.data)));
      ws.addEventListener("error", () => {
        clearTimeout(slow);
        drop("Lost the connection to ByteHive.");
        reject(new Error("Can't reach ByteHive. Is it open?"));
      });
      ws.addEventListener("close", () => drop("ByteHive closed the connection."));
    });
    return this.ready;
  }

  private onMessage(raw: string) {
    let m: { type?: string; id?: number; result?: ToolResult; error?: string };
    try {
      m = JSON.parse(raw);
    } catch {
      return;
    }
    if (typeof m.id !== "number") return;
    const w = this.waiting.get(m.id);
    if (!w) return;
    if (m.type === "hold") {
      // ByteHive is asking the user first: wait for their answer, not the usual limit.
      clearTimeout(w.timer);
      const id = m.id;
      w.timer = setTimeout(() => {
        this.waiting.delete(id);
        w.resolve(err("The user did not answer in time, so it was not done."));
      }, HOLD_TIMEOUT_MS);
      return;
    }
    this.waiting.delete(m.id);
    clearTimeout(w.timer);
    // Only the MCP fields: the browser adds its own notes for ByteHive.
    if (m.type === "result" && m.result?.content)
      w.resolve({
        content: m.result.content,
        ...(m.result.isError ? { isError: true } : {}),
        ...(m.result.gif ? { gif: m.result.gif } : {}),
        ...(typeof m.result.part === "string" ? { part: m.result.part } : {}),
        ...(m.result.record ? { record: m.result.record } : {}),
      });
    else w.resolve(err(m.error ?? "The browser did not answer."));
  }

  async call(tool: string, args: Record<string, unknown>): Promise<ToolResult> {
    try {
      await this.open();
    } catch (e) {
      return err(e instanceof Error ? e.message : String(e));
    }
    const id = this.next++;
    return new Promise<ToolResult>((resolve) => {
      const timer = setTimeout(() => {
        this.waiting.delete(id);
        resolve(err(`The browser took longer than ${CALL_TIMEOUT_MS / 1000}s on ${tool}.`));
      }, CALL_TIMEOUT_MS);
      this.waiting.set(id, { resolve, timer, tool });
      this.ws!.send(JSON.stringify({ type: "call", id, tool, args }));
    });
  }
}

export const err = (text: string): ToolResult => ({ content: [{ type: "text", text }], isError: true });

const tabId = z.number().int().optional().describe("Tab id from browser_tabs; defaults to your most recent tab");

// Names the actions plainly: a client gets them as browser_<action> tools or as
// the actions of one browser tool (browsergate.ts).
export const BROWSER_INSTRUCTIONS = `Browser: you can use the user's real Chrome (their logins included) through the browser tools (browser_<action>, or one browser tool taking an action). You work in your own tab group, named after you. Reuse your tab: navigate goes to a page in it (and opens it the first time); open another with tab_new only when you need two pages side by side, and close tabs you are done with. Read a page with read_page (gives refs), act with computer (click a ref or a screenshot coordinate, type, key), and check with a screenshot. When you hit a login page or a CAPTCHA, stop and ask the user to handle it. Never submit payments or send messages the user didn't ask for. Clicks that send, post, pay or delete wait for the user to allow them in ByteHive; if they say no, don't retry.`;

/** Absolute paths for files to upload: relative ones are taken from the
 *  workspace; every file must exist, so the page never gets a bad path. */
export function uploadPaths(paths: string[], base: string): string[] {
  return paths.map((p) => {
    const abs = path.isAbsolute(p) ? p : path.resolve(base, p);
    if (!fs.existsSync(abs) || !fs.statSync(abs).isFile()) throw new Error(`No such file: ${abs}`);
    return abs;
  });
}

/** Where a recording is saved when the agent names no path: a .gif file, or
 *  a folder for a video (video, timeline.md, frames/). */
export function recordingPath(base: string, agent: string, ext = "", now = new Date()): string {
  const stamp = now.toISOString().replace(/[:.]/g, "-").slice(0, 19);
  return path.join(base, ".bytehive", "recordings", `${agent.replace(/[^\w-]+/g, "_")}-${stamp}${ext}`);
}

/** A finished video recording: pull every file from the browser in parts and
 *  write it under dir. The names come from the extension, so each must stay
 *  inside dir. Always frees the recording in the browser at the end. */
export async function saveRecording(hub: Pick<HubClient, "call">, dir: string, files: RecordFile[]): Promise<number> {
  let total = 0;
  try {
    for (const f of files) {
      const file = path.resolve(dir, f.name);
      const rel = path.relative(dir, file);
      if (!rel || rel.startsWith("..") || path.isAbsolute(rel)) throw new Error(`Bad file name ${f.name}`);
      fs.mkdirSync(path.dirname(file), { recursive: true });
      const fd = fs.openSync(file, "w");
      try {
        let offset = 0;
        while (offset < f.size) {
          const r = await hub.call("record", { action: "part", name: f.name, offset });
          if (r.isError || typeof r.part !== "string") throw new Error(r.content.map((c) => ("text" in c ? c.text : "")).join(" ") || `No data for ${f.name}`);
          const bytes = Buffer.from(r.part, "base64");
          if (!bytes.length) break;
          fs.writeSync(fd, bytes);
          offset += bytes.length;
        }
        total += offset;
      } finally {
        fs.closeSync(fd);
      }
    }
  } finally {
    await hub.call("record", { action: "done" });
  }
  return total;
}
