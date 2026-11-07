/* Ingest: the dock panel for what the gate did today. Messages it held back can
 * be sent anyway or deleted; answers it computed are listed so nothing it did is
 * invisible. Rows share Flow's anatomy, so the two logs read the same. Nothing
 * polls: the panel redraws when it opens and when the gate reports a change. */

import { fmtTime } from "../flow";
import { getPrefs } from "../prefs";
import { money } from "../pricing";
import type { DockContext } from "../dockstore";
import { blockedList, blockedRemove, blockedResend, blockedRestore, ingestRecent, ingestStats, onIngest, type RecentResult } from "./index";
import type { BlockedList } from "./blocked";
import type { LogStats } from "./log";
import type { BlockedEntry, Source, ToolResult } from "./types";

export interface PanelApi {
  enabled(): boolean;
  list(dir: string): Promise<BlockedList>;
  stats(dir: string): Promise<LogStats>;
  recent(dir: string): readonly RecentResult[];
  resend(dir: string, entry: BlockedEntry): Promise<boolean>;
  remove(dir: string, id: string): Promise<BlockedEntry | null>;
  restore(dir: string, entry: BlockedEntry): Promise<void>;
  subscribe(fn: () => void): () => void;
  openSettings(): void;
}

const REASONS: Record<string, string> = {
  smalltalk: "Small talk",
  loop: "Same message again",
  empty: "Empty message",
  guardrail: "Not a task, says the classifier",
};

const TOOLS: Record<string, string> = {
  estimate_cost: "Cost estimate",
  convert_currency: "Currency",
  remaining_budget: "Budget left",
};

const count = (n: number, one: string, many = one + "s"): string => `${n.toLocaleString("en-US")} ${n === 1 ? one : many}`;

/** The day in one sentence. Tokens saved stay out: they are only an estimate. */
export function todayLine(s: LogStats): string {
  if (s.messages === 0) return "No messages through the gate today.";
  return (
    `Today: ${count(s.messages, "message")}, ${s.blocked} held back, ${s.cacheHits} from cache, ` +
    `${count(s.calls, "classifier call")} using ${count(s.tokens, "token")}.`
  );
}

/** What a computed answer says, in the words a person would read it. */
export function answerText(r: ToolResult): string {
  if (r.status === "error") return `Failed: ${r.error ?? "no answer"}`;
  if (r.metadata.tool === "estimate_cost" && typeof r.values.usd === "number") return money(r.values.usd);
  return Object.entries(r.values)
    .map(([k, v]) => `${k} ${typeof v === "number" ? Math.round(v * 1e6) / 1e6 : v}`)
    .join(", ");
}

function el<K extends keyof HTMLElementTagNameMap>(tag: K, cls: string, text?: string): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  node.className = cls;
  if (text !== undefined) node.textContent = text;
  return node;
}

function button(cls: string, text: string, onClick: () => void): HTMLButtonElement {
  const b = el("button", cls, text);
  b.type = "button";
  b.addEventListener("click", onClick);
  return b;
}

/** Long text folds to two lines; pressing it opens the whole message. */
function foldable(text: string): HTMLButtonElement {
  const b = el("button", "flow-text", text);
  b.type = "button";
  b.setAttribute("aria-expanded", "false");
  b.title = "Show the full message";
  b.addEventListener("click", () => {
    b.setAttribute("aria-expanded", String(b.classList.toggle("full")));
  });
  return b;
}

/** Who sent it: Kanban and the phone are you; an unnamed agent is not. */
export function senderName(from: string | undefined, source: Source): string {
  if (from) return from;
  return source === "kanban" || source === "phone" ? "You" : "An agent";
}

function who(from: string, to: string): HTMLElement {
  const line = el("span", "flow-who");
  line.append(el("b", "", from), el("span", "flow-arw", "→"), el("span", "flow-to", to));
  return line;
}

/** Held messages outlive the day they were held, so an old one shows its date. */
export function stamp(ts: number, now = Date.now()): string {
  const d = new Date(ts);
  if (d.toDateString() === new Date(now).toDateString()) return fmtTime(ts);
  return `${d.toLocaleDateString("en-US", { month: "short", day: "numeric" })} ${fmtTime(ts)}`;
}

const defaultApi = (openSettings: () => void): PanelApi => ({
  enabled: () => getPrefs().ingest,
  list: blockedList,
  stats: ingestStats,
  recent: ingestRecent,
  resend: blockedResend,
  remove: blockedRemove,
  restore: blockedRestore,
  subscribe: onIngest,
  openSettings,
});

export function createIngestPanel(openSettings: () => void, api: PanelApi = defaultApi(openSettings)) {
  let root: HTMLElement | null = null;
  let dir: string | null = null;
  let open = false;
  let drawn = 0;
  const status = el("p", "igp-status");
  status.setAttribute("role", "status");

  function say(text: string, undo?: () => void): void {
    status.replaceChildren(text);
    if (undo) status.append(" ", button("igp-link", "Undo", undo));
  }

  function heldRow(entry: BlockedEntry, index: number): HTMLElement {
    const row = el("div", "flow-row");
    row.dataset.id = entry.id;

    const head = el("div", "igp-head");
    head.append(who(senderName(entry.from, entry.source), entry.toName ?? "agent"), el("time", "flow-time", stamp(entry.at)));

    const actions = el("div", "igp-actions");
    actions.append(
      el("span", "igp-why", REASONS[entry.reason] ?? entry.reason),
      button("igp-btn", "Delete", () => void remove(entry, index)),
      button("igp-btn igp-send", "Send anyway", () => void resend(entry, index)),
    );

    row.append(head, foldable(entry.text), actions);
    return row;
  }

  function answerRow(r: RecentResult): HTMLElement {
    const row = el("div", "flow-row");
    const head = el("div", "igp-head");
    head.append(who(senderName(r.from, r.source), r.toName ?? "agent"), el("time", "flow-time", stamp(r.at)));
    const line = el("p", "igp-answer");
    line.append(el("b", "", TOOLS[r.result.metadata.tool] ?? r.result.metadata.tool), ` ${answerText(r.result)}`);
    if (r.cached) line.append(el("span", "igp-why", " from cache, 0 tokens"));
    row.append(head, line, foldable(r.text));
    return row;
  }

  /** Focus stays where the row was: the next one down, else the panel. */
  function focusRow(index: number): void {
    const rows = root?.querySelectorAll<HTMLElement>(".igp-held .flow-row .igp-send");
    const next = rows?.[Math.min(index, (rows?.length ?? 1) - 1)];
    (next ?? root)?.focus();
  }

  async function resend(entry: BlockedEntry, index: number): Promise<void> {
    if (!dir) return;
    const ok = await api.resend(dir, entry).catch(() => false);
    say(ok ? `Sent to ${entry.toName ?? "the agent"}.` : `Couldn't reach ${entry.toName ?? "the agent"}, so the message is still here.`);
    await draw();
    if (ok) focusRow(index);
  }

  async function remove(entry: BlockedEntry, index: number): Promise<void> {
    if (!dir) return;
    const here = dir;
    try {
      const gone = await api.remove(here, entry.id);
      say(
        "Message deleted.",
        gone
          ? () => {
              void api.restore(here, gone).then(() => {
                say("Message restored.");
                return draw();
              });
            }
          : undefined,
      );
    } catch {
      say("Couldn't delete it, so the message is still here.");
    }
    await draw();
    focusRow(index);
  }

  /** Redraw from the files. A slower read that started earlier never wins. */
  async function draw(): Promise<void> {
    if (!root || !open) return;
    const ticket = ++drawn;
    if (!dir) {
      root.replaceChildren(el("div", "flow-empty", "Open a project to see its messages."));
      return;
    }
    const here = dir;
    let held: BlockedList;
    let today: LogStats;
    try {
      [held, today] = await Promise.all([api.list(here), api.stats(here)]);
    } catch {
      if (ticket !== drawn) return;
      const failed = el("div", "flow-empty", "Couldn't read this project's gate files.");
      failed.append(button("igp-btn", "Try again", () => void draw()));
      root.replaceChildren(failed);
      return;
    }
    if (ticket !== drawn) return;

    const parts: HTMLElement[] = [];
    if (api.enabled()) {
      parts.push(el("p", "igp-today", todayLine(today)));
    } else {
      const off = el("p", "igp-today", "The gate is off, so every message goes straight to its agent. ");
      off.append(button("igp-link", "Open Settings", () => api.openSettings()));
      parts.push(off);
    }
    parts.push(status);

    const heldSec = el("section", "igp-sec igp-held");
    heldSec.setAttribute("aria-label", "Held back");
    heldSec.append(el("h3", "igp-h", `Held back (${held.entries.length})`));
    if (held.damaged) {
      heldSec.append(el("p", "igp-none", "The list of held messages is damaged, so it can't be shown. New ones start a fresh list."));
    } else if (held.entries.length === 0) {
      heldSec.append(el("p", "igp-none", "Nothing held back."));
    } else {
      const list = el("div", "flow-list");
      [...held.entries].reverse().forEach((e, i) => list.append(heldRow(e, i)));
      heldSec.append(list);
    }
    parts.push(heldSec);

    const answers = api.recent(here);
    if (answers.length > 0) {
      const sec = el("section", "igp-sec");
      sec.setAttribute("aria-label", "Answered by code");
      const list = el("div", "flow-list");
      [...answers].reverse().forEach((r) => list.append(answerRow(r)));
      sec.append(el("h3", "igp-h", `Answered by code (${answers.length})`), list);
      parts.push(sec);
    }
    root.replaceChildren(...parts);
  }

  return {
    mount(body: HTMLElement) {
      root = el("div", "flow-root igp-root");
      root.tabIndex = -1;
      body.appendChild(root);
      api.subscribe(() => void draw());
    },
    show() {
      open = true;
      void draw();
    },
    hide() {
      open = false;
      say("");
    },
    setContext(ctx: DockContext | null) {
      dir = ctx?.dir ?? null;
      void draw();
    },
  };
}
