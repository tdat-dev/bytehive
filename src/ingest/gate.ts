/** The gate: hash, cache, rules, provider, code, then the pane. It never
 *  guesses. A failure at any step that is not a confident "this is small
 *  talk" sends the original message through unchanged. */
import { normalize } from "./normalize";
import { rules } from "./rules";
import type {
  BlockedEntry,
  Classified,
  ClassifyReply,
  GateSettings,
  LlmUsage,
  LogEntry,
  Outcome,
  Request,
  Tool,
  ToolResult,
} from "./types";

export interface GateDeps {
  send(to: string, text: string): Promise<boolean>;
  /** False when nobody can receive the answer; the message then goes on to the agent. */
  sendResult(req: Request, result: ToolResult): boolean;
  classify(text: string): Promise<ClassifyReply>;
  isAwaiting(to: string): boolean;
  settings(): GateSettings;
  cache: { get(key: string): Promise<ToolResult | null>; set(key: string, result: ToolResult): Promise<void> };
  tools: Tool[];
  log(entry: LogEntry): void;
  block(entry: BlockedEntry): Promise<void>;
  canCall(): boolean;
  hash(text: string): Promise<string>;
  id(): string;
  now?: () => number;
  warn?: (message: string) => void;
}

const tokensOf = (text: string): number => Math.ceil(text.length / 4);

const llmOf = (usage: LlmUsage | undefined, called: boolean): LogEntry["llm"] =>
  called ? { called: true, in: usage?.in ?? 0, out: usage?.out ?? 0, model: usage?.model } : undefined;

export class Gate {
  private readonly seen = new Map<string, number>();
  /** The last message in flight per agent, so one waiting on the provider is not overtaken. */
  private readonly tails = new Map<string, Promise<unknown>>();
  private readonly now: () => number;

  constructor(private readonly deps: GateDeps) {
    this.now = deps.now ?? Date.now;
  }

  send(req: Request): Promise<Outcome> {
    // Off means off: the same synchronous hand-off as a plain sendMessage.
    if (!this.deps.settings().enabled) return this.pass(req, this.now());
    const before = this.tails.get(req.to) ?? Promise.resolve();
    const run = before.then(() => this.run(req));
    const tail: Promise<unknown> = run.catch(() => {}).finally(() => {
      if (this.tails.get(req.to) === tail) this.tails.delete(req.to);
    });
    this.tails.set(req.to, tail);
    return run;
  }

  private async run(req: Request): Promise<Outcome> {
    const started = this.now();
    const settings = this.deps.settings();
    if (this.deps.isAwaiting(req.to)) return this.pass(req, started);

    const text = normalize(req.text);
    const hash = await this.deps.hash(text);

    const hit = await this.deps.cache.get(hash);
    if (hit) {
      if (!this.resultTo(req, hit)) return this.pass(req, started, hash);
      this.log(req, hash, "cache-hit", started, { tokensSavedEstimate: tokensOf(req.text) });
      return { kind: "cache-hit", result: hit };
    }

    if (this.isLoop(req, hash, settings.loopWindowMin)) return this.block(req, hash, "loop", started);
    if (!text) return this.block(req, hash, "empty", started);

    let fromRules = true;
    let classified: Classified | null = rules(text);
    let usage: LlmUsage | undefined;
    let classifyMs: number | undefined;

    if (!classified) {
      if (!this.deps.canCall()) {
        return this.pass(req, started, hash, { error: "provider.capped" });
      }
      fromRules = false;
      const at = this.now();
      let error: string | undefined;
      try {
        const reply: ClassifyReply = await this.deps.classify(text);
        classified = reply.result;
        usage = reply.usage;
        error = reply.error;
      } catch (e) {
        error = String(e);
      }
      classifyMs = this.now() - at;
      if (!classified) {
        return this.pass(req, started, hash, {
          error: error ?? "classifier",
          llm: llmOf(usage, true),
          ms: { total: this.now() - started, classify: classifyMs },
        });
      }
    }

    if (!classified.is_actionable) {
      return this.block(req, hash, fromRules ? "smalltalk" : "guardrail", started, {
        llm: llmOf(usage, !fromRules),
        ms: { total: this.now() - started, classify: classifyMs },
      });
    }

    const intent = typeof classified.data?.intent === "string" ? classified.data.intent : "";
    const tool = intent ? this.deps.tools.find((t) => t.name === intent) : undefined;
    if (classified.data && tool) {
      const at = this.now();
      let result: ToolResult;
      try {
        result = await tool.run(classified.data);
      } catch (e) {
        result = {
          status: "error",
          values: {},
          timestamp: this.now(),
          metadata: { tool: tool.name, version: tool.version, cacheable: tool.cacheable },
          error: String(e),
        };
      }
      const executeMs = this.now() - at;
      if (result.status === "ok" && tool.cacheable) {
        try {
          await this.deps.cache.set(hash, result);
        } catch {
          /* the answer is still correct; the next one just runs the tool again */
        }
      }
      const ms = { total: this.now() - started, classify: classifyMs, execute: executeMs };
      if (!this.resultTo(req, result)) return this.pass(req, started, hash, { llm: llmOf(usage, !fromRules), ms });
      this.log(req, hash, fromRules ? "rule" : "api", started, { llm: llmOf(usage, !fromRules), ms });
      return result.status === "ok" ? { kind: "answered", via: fromRules ? "rule" : "tool", result } : { kind: "failed", error: result.error ?? "tool" };
    }

    return this.pass(req, started, hash, {
      llm: llmOf(usage, !fromRules),
      ms: { total: this.now() - started, classify: classifyMs },
    });
  }

  private async pass(req: Request, started: number, hash?: string, extra: Partial<LogEntry> = {}): Promise<Outcome> {
    let sent = false;
    try {
      sent = await this.deps.send(req.to, req.text);
    } catch (e) {
      this.log(req, hash ?? "", "failed", started, { ...extra, error: String(e) });
      return { kind: "failed", error: String(e) };
    }
    if (!sent) {
      this.log(req, hash ?? "", "failed", started, { ...extra, error: "pane is not running" });
      return { kind: "failed", error: "pane is not running" };
    }
    if (hash) this.log(req, hash, "pass", started, extra);
    return { kind: "sent", via: "pass" };
  }

  private async block(req: Request, hash: string, reason: string, started: number, extra: Partial<LogEntry> = {}): Promise<Outcome> {
    const id = this.deps.id();
    try {
      await this.deps.block({ id, hash, source: req.source, from: req.from, to: req.to, text: req.text, reason, at: this.now() });
    } catch (e) {
      this.warn("could not keep the blocked message: " + String(e));
    }
    this.log(req, hash, "blocked", started, { tokensSavedEstimate: tokensOf(req.text), ...extra });
    return { kind: "blocked", id, reason };
  }

  private resultTo(req: Request, result: ToolResult): boolean {
    try {
      return this.deps.sendResult(req, result);
    } catch (e) {
      this.warn("could not deliver the result: " + String(e));
      return false;
    }
  }

  private isLoop(req: Request, hash: string, minutes: number): boolean {
    if (req.source !== "fleet" && req.source !== "founder") return false;
    const now = this.now();
    const window = Math.max(1, minutes) * 60_000;
    const key = `${req.from ?? ""}\u0000${req.to}\u0000${hash}`;
    const at = this.seen.get(key);
    if (at !== undefined && now - at <= window) return true;
    this.seen.set(key, now);
    if (this.seen.size > 1000) {
      for (const [k, v] of this.seen) if (now - v > window) this.seen.delete(k);
    }
    return false;
  }

  private log(req: Request, hash: string, path: LogEntry["path"], started: number, extra: Partial<LogEntry> = {}): void {
    const total = this.now() - started;
    try {
      this.deps.log({
        ts: this.now(),
        hash,
        source: req.source,
        from: req.from,
        to: req.to,
        path,
        ...extra,
        ms: { total, ...extra.ms },
      });
    } catch {
      /* telemetry must never break a send */
    }
  }

  private warn(message: string): void {
    (this.deps.warn ?? console.warn)(message);
  }
}
