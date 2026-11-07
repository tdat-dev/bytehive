/** The message gate's vocabulary. A message is handed whole to `sendGated`;
 *  the gate decides whether it reaches a pane, gets answered by code, or is
 *  held back for a human. */

export type Source = "fleet" | "kanban" | "phone" | "founder" | "retry";

export interface Request {
  source: Source;
  /** Display name of the sender, when there is one (fleet, founder). */
  from?: string;
  /** Pane id of the sender, when the sender is an agent, so an answer can go back. */
  fromPane?: string;
  /** Pane id of the agent that would receive the message. */
  to: string;
  /** That agent's name, for the panel and the toast. */
  toName?: string;
  text: string;
  /** What a person wrote, when `text` wraps it in instructions (a Kanban
   *  dispatch). The gate judges this and delivers `text`. */
  gist?: string;
  /** The board card this delivers, so Send anyway can finish the dispatch. */
  card?: CardRef;
}

export interface ToolResult {
  status: "ok" | "error";
  values: Record<string, number | string>;
  timestamp: number;
  metadata: { tool: string; version: string; cacheable: boolean };
  error?: string;
}

export interface Tool {
  name: string;
  version: string;
  cacheable: boolean;
  run(input: Record<string, unknown>): Promise<ToolResult>;
}

/** The classifier's two fields. `data` rather than `payload`: 9Router's cx
 *  route rewrites that word, even in prose. */
export interface Classified {
  is_actionable: boolean;
  data: Record<string, unknown> | null;
}

export interface LlmUsage {
  in: number;
  out: number;
  model?: string;
}

export interface ClassifyReply {
  result: Classified | null;
  usage?: LlmUsage;
  error?: string;
}

/** A board card on its way to an agent. `stage` is the list it belongs in once
 *  the agent has it; without one that is Doing. */
export interface CardRef {
  id: string;
  stage?: string;
}

/** What became of a board dispatch. `stopped` means the agent did not get it and
 *  sending it again would change nothing: the gate held or answered it, or
 *  delivery failed. `no-agent` means the pane is gone, so another may take it. */
export type Delivery = "sent" | "stopped" | "no-agent";

export type Outcome =
  | { kind: "answered"; via: "rule" | "tool"; result: ToolResult }
  | { kind: "sent"; via: "pass" }
  | { kind: "cache-hit"; result: ToolResult }
  | { kind: "blocked"; id: string; reason: string }
  | { kind: "failed"; error: string };

export interface BlockedEntry {
  id: string;
  hash: string;
  source: Source;
  from?: string;
  to: string;
  toName?: string;
  text: string;
  gist?: string;
  card?: CardRef;
  reason: string;
  at: number;
}

export interface LogEntry {
  ts: number;
  hash: string;
  source: Source;
  from?: string;
  to: string;
  path: "rule" | "api" | "cache-hit" | "blocked" | "pass" | "failed";
  ms: { total: number; classify?: number; execute?: number };
  llm?: LlmUsage & { called: boolean };
  tokensSavedEstimate?: number;
  error?: string;
}

export interface GateSettings {
  enabled: boolean;
  ttlDays: number;
  dailyCallCap: number;
  dailyTokenCap: number;
  loopWindowMin: number;
}
