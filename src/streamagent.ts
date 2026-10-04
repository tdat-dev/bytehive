// Claude agents Maestro talks to directly, in JSON lines, instead of typing
// into a terminal and reading its screen back.
//
// Such an agent runs `claude -p --input-format stream-json --output-format
// stream-json` on plain pipes (pty_spawn with pipe: true). A message is one
// line in; what it writes comes out as events: the reply as it is written,
// each tool call, every permission it needs and question it asks (as a
// request waiting for an answer), and a result when the turn ends. Nothing is
// guessed from the screen, so a message can't be lost in the CLI's box, a
// question can't be misread, and the reply shows word by word.
//
// The conversation itself is still read from Claude's transcript file (the
// chat view does that for every Claude agent); this module only carries what
// the transcript doesn't: input, live text, what it waits on, whether it works.
// Its terminal shows a plain log of the exchange.

import { setInputRouter, writeRaw, type InputRoute } from "./ipc";
import type { Ask, AskOption } from "./askparse";
import type { CliCommand, CliFacts } from "./cliprofile";
import type { LimitHit } from "./limits";

/** How `claude` is started for this: print mode, JSON lines both ways, the
 *  reply in pieces as it is written, and permissions asked of us. */
export const STREAM_ARGS = [
  "-p",
  "--input-format", "stream-json",
  "--output-format", "stream-json",
  "--verbose",
  "--include-partial-messages",
  "--permission-prompt-tool", "stdio",
];

type Json = Record<string, unknown>;
const obj = (v: unknown): Json => (v && typeof v === "object" && !Array.isArray(v) ? (v as Json) : {});
const str = (v: unknown): string => (typeof v === "string" ? v : "");

/** A question or permission the agent is waiting on. */
interface Pending {
  requestId: string;
  tool: string;
  input: Json;
  suggestions: unknown[];
  ask: Ask;
  /** AskUserQuestion: the answers so far, one question at a time. */
  answers: Record<string, string>;
  /** Which of its questions is on show. */
  qi: number;
}

interface Stream {
  paneId: string;
  sessionId: string | null;
  dec: TextDecoder;
  buf: string;
  /** Messages sent whose turn hasn't ended yet (one runs, the rest queue). */
  turns: number;
  /** The reply block being written, as far as it has come. */
  live: string;
  /** Thinking before it writes. */
  thinking: boolean;
  /** The tool it is calling or running now. */
  tool: string | null;
  pending: Pending[];
  facts: CliFacts | null;
  models: Array<{ value: string; label: string }>;
  model: string | null;
  permissionMode: string | null;
  problem: string | null;
  limit: LimitHit | null;
  stderr: string[];
  nextReq: number;
  waiting: Map<string, (r: Json | null) => void>;
  log: (text: string) => void;
  startedTurnAt: number;
}

const streams = new Map<string, Stream>();
const listeners = new Set<(paneId: string) => void>();

/** Be told when an agent's live state changes (a piece of reply, a question). */
export function onStreamChange(cb: (paneId: string) => void): () => void {
  listeners.add(cb);
  return () => listeners.delete(cb);
}
function changed(s: Stream): void {
  for (const cb of listeners) cb(s.paneId);
}

export function isStream(paneId: string): boolean {
  return streams.has(paneId);
}

/** Write one JSON line to the agent. */
function write(s: Stream, msg: Json): Promise<void> {
  return writeRaw(s.paneId, `${JSON.stringify(msg)}\n`);
}

/** Ask the CLI something of its own (initialize, interrupt, set_model…). */
function control(s: Stream, request: Json, timeoutMs = 30_000): Promise<Json | null> {
  const id = `m${(s.nextReq += 1)}`;
  return new Promise((resolve) => {
    const t = window.setTimeout(() => { s.waiting.delete(id); resolve(null); }, timeoutMs);
    s.waiting.set(id, (r) => { window.clearTimeout(t); resolve(r); });
    void write(s, { type: "control_request", request_id: id, request }).catch(() => { s.waiting.delete(id); resolve(null); });
  });
}

const ANSI = { dim: "\x1b[90m", user: "\x1b[36m", warn: "\x1b[33m", err: "\x1b[31m", off: "\x1b[0m" };
const crlf = (t: string) => t.replace(/\r?\n/g, "\r\n");
function say(s: Stream, color: string, text: string): void {
  s.log(`${color}${crlf(text)}${color ? ANSI.off : ""}\r\n`);
}

/** Start talking to the agent that just started on pipes in this pane. */
export function openStream(paneId: string, opts: { sessionId: string | null; log: (text: string) => void }): void {
  const s: Stream = {
    paneId, sessionId: opts.sessionId, dec: new TextDecoder(), buf: "",
    turns: 0, live: "", thinking: false, tool: null, pending: [],
    facts: null, models: [], model: null, permissionMode: null, problem: null, limit: null,
    stderr: [], nextReq: 0, waiting: new Map(), log: opts.log, startedTurnAt: 0,
  };
  streams.set(paneId, s);
  say(s, ANSI.dim, "Maestro talks to Claude directly here: this is a log of the conversation, not its terminal.");
  changed(s);
}

/** Its process is running: ask it what it has (commands, models). Not
 *  before, or the request goes nowhere. */
export function startStream(paneId: string): void {
  const s = streams.get(paneId);
  if (!s) return;
  void control(s, { subtype: "initialize", hooks: null }, 120_000).then((r) => {
    if (!r || streams.get(paneId) !== s) return;
    const commands = Array.isArray(r.commands) ? r.commands : [];
    s.facts = {
      commands: commands.map((c): CliCommand => {
        const name = str(obj(c).name);
        return { name, kind: name.includes(":") ? "plugin" : / \((user|project)\)$/.test(str(obj(c).description)) ? "skill" : "command" };
      }).filter((c) => c.name),
      model: s.model,
    };
    s.models = (Array.isArray(r.models) ? r.models : []).map((m) => ({ value: str(obj(m).value), label: str(obj(m).displayName) || str(obj(m).value) })).filter((m) => m.value);
    changed(s);
  });
}

/** The agent's process ended: nothing runs and nothing waits any more. */
export function closeStream(paneId: string, code?: number): void {
  const s = streams.get(paneId);
  if (!s) return;
  if (code && !s.problem) {
    const tail = s.stderr.slice(-3).join(" ").trim();
    s.problem = tail || `Claude stopped (exit code ${code}).`;
  }
  for (const done of s.waiting.values()) done(null);
  s.waiting.clear();
  Object.assign(s, { turns: 0, live: "", thinking: false, tool: null, pending: [] });
  changed(s);
}

/** Forget an agent entirely (removed, or started again in the old way). */
export function dropStream(paneId: string): void {
  if (streams.delete(paneId)) for (const cb of listeners) cb(paneId);
}

/** Bytes from the agent: whole JSON lines, one event each. */
export function feedStream(paneId: string, bytes: Uint8Array): void {
  const s = streams.get(paneId);
  if (!s) return;
  s.buf += s.dec.decode(bytes, { stream: true });
  let i: number;
  let any = false;
  while ((i = s.buf.indexOf("\n")) >= 0) {
    const line = s.buf.slice(0, i).trim();
    s.buf = s.buf.slice(i + 1);
    if (!line) continue;
    let m: Json;
    try { m = JSON.parse(line); } catch { say(s, ANSI.dim, line); continue; }
    if (handle(s, m)) any = true;
  }
  if (any) changed(s);
}

/** One event. True when what the chat shows may have changed. */
export function handle(s: Stream, m: Json): boolean {
  switch (m.type) {
    case "stream_event": {
      const e = obj(m.event);
      if (e.type === "content_block_start") {
        const b = obj(e.content_block);
        if (b.type === "text") { s.live = ""; s.thinking = false; }
        else if (b.type === "thinking") s.thinking = true;
        else if (b.type === "tool_use") { s.tool = str(b.name); s.thinking = false; }
        return true;
      }
      if (e.type === "content_block_delta") {
        const d = obj(e.delta);
        if (d.type === "text_delta") { s.live += str(d.text); return true; }
        return false;
      }
      return false;
    }
    case "assistant": {
      for (const b of Array.isArray(obj(m.message).content) ? (obj(m.message).content as unknown[]) : []) {
        const block = obj(b);
        if (block.type === "text" && str(block.text).trim()) say(s, "", str(block.text));
        else if (block.type === "tool_use") say(s, ANSI.dim, `● ${str(block.name)} ${brief(obj(block.input))}`);
      }
      if (typeof obj(m.message).model === "string") s.model = str(obj(m.message).model);
      return true;
    }
    case "user": {
      // its tool results: the tool is done
      s.tool = null;
      return true;
    }
    case "result": {
      s.turns = Math.max(0, s.turns - 1);
      s.live = "";
      s.thinking = false;
      s.tool = null;
      const secs = Math.round(Number(m.duration_ms ?? 0) / 1000);
      if (m.is_error && m.stop_reason !== null && m.subtype !== "error_during_execution") {
        const why = str(m.result) || (Array.isArray(m.errors) ? (m.errors as unknown[]).map(str).join(" ") : "") || "The turn ended with an error.";
        s.problem = why;
        say(s, ANSI.err, why);
      } else if (m.is_error) {
        say(s, ANSI.dim, "Stopped.");
      } else {
        s.problem = null;
        say(s, ANSI.dim, `Done${secs ? ` in ${secs}s` : ""}.`);
      }
      return true;
    }
    case "system": {
      if (m.subtype === "init") {
        if (typeof m.model === "string") s.model = m.model;
        if (typeof m.permissionMode === "string") s.permissionMode = m.permissionMode;
        if (typeof m.session_id === "string") s.sessionId = m.session_id;
        if (s.facts) s.facts = { ...s.facts, model: s.model };
      } else if (m.subtype === "status" && typeof m.permissionMode === "string") {
        s.permissionMode = m.permissionMode;
      }
      return true;
    }
    case "rate_limit_event": {
      const info = obj(m.rate_limit_info);
      if (info.status === "rejected") {
        const at = Number(info.resetsAt ?? 0);
        s.limit = { what: "Claude's usage limit is reached", resetsAt: at ? at * 1000 : null };
      } else s.limit = null;
      return true;
    }
    case "control_request": {
      const req = obj(m.request);
      const id = str(m.request_id);
      if (req.subtype === "can_use_tool") {
        const p = pendingOf(id, req);
        s.pending.push(p);
        say(s, ANSI.warn, `? ${p.ask.prompt}${p.ask.detail ? `  ${p.ask.detail}` : ""}`);
        return true;
      }
      // Nothing else is ours to answer (no hooks or MCP servers are run from here).
      void write(s, { type: "control_response", response: { subtype: "error", request_id: id, error: `Maestro doesn't handle ${str(req.subtype)}` } });
      return false;
    }
    case "control_response": {
      const r = obj(m.response);
      const done = s.waiting.get(str(r.request_id));
      if (done) { s.waiting.delete(str(r.request_id)); done(r.subtype === "success" ? obj(r.response) : null); }
      return false;
    }
    case "control_cancel_request": {
      const before = s.pending.length;
      s.pending = s.pending.filter((p) => p.requestId !== str(m.request_id));
      return s.pending.length !== before;
    }
    case "maestro_stderr": {
      const text = str(m.text);
      s.stderr.push(text);
      if (s.stderr.length > 40) s.stderr.shift();
      say(s, ANSI.err, text);
      return false;
    }
  }
  return false;
}

/** A tool's input in a few words, for the log. */
function brief(input: Json): string {
  const v = str(input.command) || str(input.file_path) || str(input.pattern) || str(input.url) || str(input.description) || str(input.prompt);
  const line = v.split(/\r?\n/)[0];
  return line.length > 120 ? `${line.slice(0, 119)}…` : line;
}

const RUNS = new Set(["Bash", "PowerShell"]);
const EDITS = new Set(["Edit", "Write", "MultiEdit", "NotebookEdit"]);

/** What a can_use_tool request asks, in the shape the ask card and the chat
 *  already answer: numbered options, the last one "no, and say why". */
export function askOf(tool: string, input: Json, description: string, suggestions: unknown[], qi = 0): Ask {
  if (tool === "AskUserQuestion") {
    const qs = Array.isArray(input.questions) ? (input.questions as unknown[]).map(obj) : [];
    const q = qs[qi] ?? {};
    const opts = Array.isArray(q.options) ? (q.options as unknown[]).map(obj) : [];
    const options: AskOption[] = opts.map((o, k) => ({ n: k + 1, label: str(o.label), key: String(k + 1) }));
    return {
      kind: "question",
      prompt: str(q.question) || "Claude has a question",
      title: str(q.header) || undefined,
      options,
      freeTextOption: options.length + 1,
    };
  }
  const options: AskOption[] = [{ n: 1, label: "Yes", key: "1" }];
  if (suggestions.length) options.push({ n: 2, label: "Yes, and don't ask again for this", key: "2", always: true });
  const no = options.length + 1;
  options.push({ n: no, label: "No, and tell Claude what to do instead", key: String(no), deny: true });
  const kind = RUNS.has(tool) ? "run" : EDITS.has(tool) ? "edit" : "run";
  const detail = RUNS.has(tool) ? str(input.command) : EDITS.has(tool) ? str(input.file_path) || str(input.notebook_path) : tool;
  const prompt = description || (kind === "edit" ? `Claude wants to change ${detail}` : `Claude wants to use ${tool}`);
  return { kind, prompt, title: tool, detail: detail || undefined, options, freeTextOption: no };
}

function pendingOf(requestId: string, req: Json): Pending {
  const tool = str(req.tool_name);
  const input = obj(req.input);
  const suggestions = Array.isArray(req.permission_suggestions) ? (req.permission_suggestions as unknown[]) : [];
  return { requestId, tool, input, suggestions, ask: askOf(tool, input, str(req.description), suggestions), answers: {}, qi: 0 };
}

function respond(s: Stream, p: Pending, response: Json): void {
  s.pending = s.pending.filter((x) => x !== p);
  void write(s, { type: "control_response", response: { subtype: "success", request_id: p.requestId, response } });
  changed(s);
}

/** Answer what it waits on with option `n` (a number key, as on its screen). */
export function answerPending(paneId: string, n: number): boolean {
  const s = streams.get(paneId);
  const p = s?.pending[0];
  if (!s || !p) return false;
  const o = p.ask.options.find((x) => x.n === n);
  if (!o) return false;
  if (p.tool === "AskUserQuestion") return answerQuestion(s, p, o.label);
  if (o.deny) { respond(s, p, { behavior: "deny", message: "The user said no." }); say(s, ANSI.dim, "You said no."); return true; }
  respond(s, p, o.always
    ? { behavior: "allow", updatedInput: p.input, updatedPermissions: p.suggestions }
    : { behavior: "allow", updatedInput: p.input });
  say(s, ANSI.dim, o.always ? "You allowed it, and it won't ask again." : "You allowed it.");
  return true;
}

/** Answer in your own words: the reply to its question, or why not and what
 *  to do instead. */
export function answerPendingText(paneId: string, text: string): boolean {
  const s = streams.get(paneId);
  const p = s?.pending[0];
  const t = text.trim();
  if (!s || !p || !t) return false;
  if (p.tool === "AskUserQuestion") return answerQuestion(s, p, t);
  respond(s, p, { behavior: "deny", message: t });
  say(s, ANSI.dim, `You said: ${t}`);
  return true;
}

function answerQuestion(s: Stream, p: Pending, answer: string): boolean {
  const qs = Array.isArray(p.input.questions) ? (p.input.questions as unknown[]).map(obj) : [];
  p.answers[str(qs[p.qi]?.question)] = answer;
  say(s, ANSI.dim, `You answered: ${answer}`);
  if (p.qi + 1 < qs.length) {
    // the next of its questions, same request
    p.qi += 1;
    p.ask = askOf(p.tool, p.input, "", p.suggestions, p.qi);
    changed(s);
    return true;
  }
  respond(s, p, { behavior: "allow", updatedInput: { ...p.input, answers: p.answers } });
  return true;
}

/** Hand it a message. Sent while it works, it waits its turn. Pictures go
 *  with it as images (data URLs), not as paths it has to go and read. */
export async function sendToStream(paneId: string, text: string, images: string[] = []): Promise<boolean> {
  const s = streams.get(paneId);
  if (!s) return false;
  const pics = images.map((url) => url.match(/^data:([^;]+);base64,(.*)$/)).filter((m): m is RegExpMatchArray => !!m);
  const content: unknown = pics.length
    ? [...pics.map((m) => ({ type: "image", source: { type: "base64", media_type: m[1], data: m[2] } })), ...(text ? [{ type: "text", text }] : [])]
    : text;
  try {
    await write(s, { type: "user", message: { role: "user", content }, parent_tool_use_id: null, session_id: s.sessionId ?? "" });
  } catch {
    return false;
  }
  s.turns += 1;
  s.problem = null;
  if (s.turns === 1) s.startedTurnAt = Date.now();
  say(s, ANSI.user, `› ${text}${pics.length ? ` (${pics.length} picture${pics.length === 1 ? "" : "s"})` : ""}`);
  changed(s);
  return true;
}

/** Stop what it is doing; a question it waits on is answered "no". */
export async function interruptStream(paneId: string): Promise<void> {
  const s = streams.get(paneId);
  if (!s) return;
  for (const p of [...s.pending]) respond(s, p, { behavior: "deny", message: "The user stopped this.", interrupt: true });
  if (s.turns > 0) await control(s, { subtype: "interrupt" }, 10_000);
}

export async function setStreamModel(paneId: string, model: string): Promise<boolean> {
  const s = streams.get(paneId);
  if (!s) return false;
  const ok = !!(await control(s, { subtype: "set_model", model }));
  if (ok) { s.model = model; say(s, ANSI.dim, `Model: ${model}`); changed(s); }
  return ok;
}

export async function setStreamPermissionMode(paneId: string, mode: string): Promise<boolean> {
  const s = streams.get(paneId);
  if (!s) return false;
  const r = await control(s, { subtype: "set_permission_mode", mode });
  if (r) { s.permissionMode = mode; changed(s); }
  return !!r;
}

/** What the rest of Maestro needs to know about it right now. */
export interface StreamFacts {
  working: boolean;
  ask: Ask | null;
  live: string;
  thinking: boolean;
  tool: string | null;
  problem: string | null;
  limit: LimitHit | null;
  facts: CliFacts | null;
  models: Array<{ value: string; label: string }>;
  model: string | null;
  permissionMode: string | null;
  turnStartedAt: number;
}

export function streamFacts(paneId: string): StreamFacts | null {
  const s = streams.get(paneId);
  if (!s) return null;
  return {
    working: s.turns > 0,
    ask: s.pending[0]?.ask ?? null,
    live: s.live,
    thinking: s.thinking,
    tool: s.tool,
    problem: s.problem,
    limit: s.limit,
    facts: s.facts,
    models: s.models,
    model: s.model,
    permissionMode: s.permissionMode,
    turnStartedAt: s.startedTurnAt,
  };
}

// Messages and keys meant for a stream agent go to it in its own terms: a
// message as a JSON line, Esc as an interrupt, a number as the answer to what
// it waits on. Typing into its log does nothing.
const route = (paneId: string): InputRoute => ({
  message: (text) => sendToStream(paneId, text),
  key: async (data) => {
    if (data === "\x1b") { await interruptStream(paneId); return; }
    const n = Number(data);
    if (Number.isInteger(n) && n > 0) answerPending(paneId, n);
  },
  text: async (text) => { answerPendingText(paneId, text); },
});
/** Send input meant for stream agents their way. Called once at startup. */
export function routeStreamInput(): void {
  setInputRouter((paneId) => (streams.has(paneId) ? route(paneId) : null));
}
