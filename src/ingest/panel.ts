/* Ingest: the dock panel for what the gate did today. Messages it held back can
 * be sent anyway or deleted; answers it computed stay listed until the app
 * closes. Rows share Flow's anatomy and order, oldest first, so a new one never
 * moves a row under the pointer. Nothing polls: the panel redraws when it opens
 * and when the gate reports a change, keeping focus and opened text. */

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
    `Today: ${count(s.messages, "message")}, ${s.blocked} ${s.blocked === 1 ? "was" : "were"} held, ${s.cacheHits} from cache, ` +
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

/** `mark` names a control across redraws, so focus can find its way back. */
function button(cls: string, text: string, onClick: () => void, mark?: string): HTMLButtonElement {
  const b = el("button", cls, text);
  b.type = "button";
  if (mark) b.dataset.mark = mark;
  b.addEventListener("click", onClick);
  return b;
}

/** Long text folds to two lines; pressing it opens the whole message. `opened`
 *  remembers which, so a redraw doesn't fold what you are reading. */
function foldable(text: string, key: string, opened: Set<string>): HTMLButtonElement {
  const b = el("button", "flow-text", text);
  b.type = "button";
  b.dataset.mark = `text:${key}`;
  b.classList.toggle("full", opened.has(key));
  b.setAttribute("aria-expanded", String(opened.has(key)));
  b.title = "Show the full message";
  b.addEventListener("click", () => {
    const full = b.classList.toggle("full");
    b.setAttribute("aria-expanded", String(full));
    if (full) opened.add(key);
    else opened.delete(key);
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
  /** Rows with a send or delete under way: pressing again must not repeat it. */
  const busy = new Set<string>();
  const opened = new Set<string>();
  /** Deleted and still restorable, until Undo or a change of project. */
  let deleted: BlockedEntry[] = [];
  const status = el("p", "igp-status");
  status.setAttribute("role", "status");

  function say(text: string, undo?: () => void): void {
    status.replaceChildren(text);
    if (undo) status.append(" ", button("igp-link", "Undo", undo, "undo"));
  }

  function marked(mark: string): HTMLElement | undefined {
    return [...(root?.querySelectorAll<HTMLElement>("[data-mark]") ?? [])].find((e) => e.dataset.mark === mark);
  }

  function heldRow(entry: BlockedEntry, index: number): HTMLElement {
    const row = el("div", "flow-row");
    row.dataset.id = entry.id;

    const head = el("div", "igp-head");
    head.append(who(senderName(entry.from, entry.source), entry.toName ?? "agent"), el("time", "flow-time", stamp(entry.at)));

    const del = button("igp-btn igp-del", "Delete", () => void remove(entry, index), `del:${entry.id}`);
    const send = button("igp-btn igp-send", "Send anyway", () => void resend(entry, index), `send:${entry.id}`);
    del.disabled = send.disabled = busy.has(entry.id);

    const actions = el("div", "igp-actions");
    actions.append(el("span", "igp-why", REASONS[entry.reason] ?? entry.reason), del, send);

    row.append(head, foldable(entry.gist ?? entry.text, entry.id, opened), actions);
    return row;
  }

  function answerRow(r: RecentResult, index: number): HTMLElement {
    const row = el("div", "flow-row");
    const head = el("div", "igp-head");
    head.append(who(senderName(r.from, r.source), r.toName ?? "agent"), el("time", "flow-time", stamp(r.at)));
    const line = el("p", "igp-answer");
    line.append(el("b", "", TOOLS[r.result.metadata.tool] ?? r.result.metadata.tool), ` ${answerText(r.result)}`);
    if (r.cached) line.append(el("span", "igp-why", " from cache, 0 tokens"));
    row.append(head, line, foldable(r.gist ?? r.text, `answer:${r.at}:${index}`, opened));
    return row;
  }

  /** The same control on the row now at `index`, else Undo, else the panel. */
  function focusRow(index: number, selector: string): void {
    const rows = root?.querySelectorAll<HTMLElement>(`.igp-held .flow-row ${selector}`);
    const next = rows?.length ? rows[Math.min(index, rows.length - 1)] : undefined;
    (next ?? marked("undo") ?? root)?.focus();
  }

  function setBusy(id: string, on: boolean): void {
    if (on) busy.add(id);
    else busy.delete(id);
    for (const mark of [`del:${id}`, `send:${id}`]) {
      const b = marked(mark) as HTMLButtonElement | undefined;
      if (b) b.disabled = on;
    }
  }

  async function resend(entry: BlockedEntry, index: number): Promise<void> {
    if (!dir || busy.has(entry.id)) return;
    const name = entry.toName ?? "the agent";
    setBusy(entry.id, true);
    say(`Sending to ${name}...`);
    const ok = await api.resend(dir, entry).catch(() => false);
    setBusy(entry.id, false);
    say(ok ? `Sent to ${name}.` : `Couldn't reach ${name}, so the message is still here.`);
    await draw();
    if (ok) focusRow(index, ".igp-send");
    else marked(`send:${entry.id}`)?.focus();
  }

  function undo(): void {
    if (!dir) return;
    const here = dir;
    const items = deleted;
    deleted = [];
    void Promise.all(items.map((e) => api.restore(here, e))).then(
      async () => {
        say(items.length === 1 ? "Message restored." : `${items.length} messages restored.`);
        await draw();
        marked(`del:${items[items.length - 1].id}`)?.focus();
      },
      () => {
        deleted = items;
        say("Couldn't restore it.", undo);
        marked("undo")?.focus();
      },
    );
  }

  async function remove(entry: BlockedEntry, index: number): Promise<void> {
    if (!dir || busy.has(entry.id)) return;
    setBusy(entry.id, true);
    try {
      const gone = await api.remove(dir, entry.id);
      if (gone) deleted.push(gone);
      say(deleted.length === 1 ? "Message deleted." : `${deleted.length} messages deleted.`, undo);
    } catch {
      say("Couldn't delete it, so the message is still here.");
    }
    setBusy(entry.id, false);
    await draw();
    focusRow(index, ".igp-del");
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

    const parts: HTMLElement[] = [status];
    if (api.enabled()) {
      parts.push(el("p", "igp-today", todayLine(today)));
    } else {
      const off = el("p", "igp-today", "The gate is off, so every message goes straight to its agent. ");
      off.append(button("igp-link", "Open Settings", () => api.openSettings()));
      parts.push(off);
    }

    const heldSec = el("section", "igp-sec igp-held");
    heldSec.setAttribute("aria-label", "Held back");
    heldSec.append(el("h3", "igp-h", held.damaged ? "Held back" : `Held back (${held.entries.length})`));
    if (held.damaged) {
      heldSec.append(el("p", "igp-none", "The list of held messages is damaged, so it can't be shown. New ones start a fresh list."));
    } else if (held.entries.length === 0) {
      heldSec.append(el("p", "igp-none", "Nothing held back."));
    } else {
      const list = el("div", "flow-list");
      held.entries.forEach((e, i) => list.append(heldRow(e, i)));
      heldSec.append(list);
    }
    parts.push(heldSec);

    const answers = api.recent(here);
    if (answers.length > 0) {
      const sec = el("section", "igp-sec");
      sec.setAttribute("aria-label", "Answered by code");
      const list = el("div", "flow-list");
      answers.forEach((r, i) => list.append(answerRow(r, i)));
      sec.append(el("h3", "igp-h", `Answered by code (${answers.length})`), list);
      parts.push(sec);
    }

    const active = document.activeElement as HTMLElement | null;
    const mark = active && root.contains(active) ? active.dataset.mark : undefined;
    root.replaceChildren(...parts);
    if (mark) marked(mark)?.focus();
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
      if (deleted.length === 0) say("");
    },
    setContext(ctx: DockContext | null) {
      const next = ctx?.dir ?? null;
      if (next !== dir) {
        deleted = [];
        say("");
      }
      dir = next;
      void draw();
    },
  };
}
