/* Ingest: what the gate held back and what it answered. A held message has one
 * action, Send anyway; old ones clear themselves and the day's numbers stay in
 * the log. Rows share Flow's anatomy and order, oldest first, so a new one never
 * moves a row under the pointer. Nothing polls: the panel redraws when it opens
 * and when the gate reports a change, keeping focus and opened text. */

import type { DockContext } from "../dockstore";
import { blockedList, blockedResend, heldReachable, ingestRecent, onIngest, type RecentResult } from "./index";
import type { BlockedList } from "./blocked";
import type { BlockedEntry } from "./types";
import { TOOLS, answerText, senderName } from "./words";

export interface PanelApi {
  list(dir: string): Promise<BlockedList>;
  recent(dir: string): readonly RecentResult[];
  resend(dir: string, entry: BlockedEntry): Promise<boolean>;
  /** False when the agent isn't running, so Send anyway would not reach it now. */
  reachable(dir: string, entry: BlockedEntry): boolean;
  subscribe(fn: () => void): () => void;
}

const REASONS: Record<string, string> = {
  smalltalk: "Small talk",
  loop: "Same message again",
  empty: "Empty message",
  guardrail: "Not a task, says the classifier",
};

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

/** Who, why and when, on one quiet line under the message. */
function meta(from: string, to: string, at: number, why?: string): HTMLElement {
  const line = el("p", "igp-meta");
  line.append(`${from} → ${to}`);
  if (why) line.append(" · ", el("span", "igp-why", why));
  line.append(" · ", el("time", "", stamp(at)));
  return line;
}

const hm = (d: Date): string => `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;

/** Held messages outlive the day they were held, so an old one shows its date. */
export function stamp(ts: number, now = Date.now()): string {
  const d = new Date(ts);
  if (d.toDateString() === new Date(now).toDateString()) return hm(d);
  return `${d.toLocaleDateString("en-US", { month: "short", day: "numeric" })} ${hm(d)}`;
}

const defaultApi = (): PanelApi => ({
  list: blockedList,
  recent: ingestRecent,
  resend: blockedResend,
  reachable: heldReachable,
  subscribe: onIngest,
});

export function createIngestPanel(api: PanelApi = defaultApi()) {
  let root: HTMLElement | null = null;
  let dir: string | null = null;
  let open = false;
  let drawn = 0;
  /** Rows with a send under way: pressing again must not repeat it. */
  const busy = new Set<string>();
  const opened = new Set<string>();
  const status = el("p", "igp-status");
  status.setAttribute("role", "status");

  function say(text: string): void {
    status.replaceChildren(text);
  }

  function marked(mark: string): HTMLElement | undefined {
    return [...(root?.querySelectorAll<HTMLElement>("[data-mark]") ?? [])].find((e) => e.dataset.mark === mark);
  }

  function heldRow(entry: BlockedEntry, index: number): HTMLElement {
    const row = el("div", "flow-row");
    row.dataset.id = entry.id;

    const send = button("igp-btn igp-send", "Send anyway", () => void resend(entry, index), `send:${entry.id}`);
    send.disabled = busy.has(entry.id);

    const foot = el("div", "igp-foot");
    const line = meta(senderName(entry.from, entry.source), entry.toName ?? "agent", entry.at, REASONS[entry.reason] ?? entry.reason);
    if (dir && !api.reachable(dir, entry)) line.append(" · ", el("span", "igp-stopped", `${entry.toName ?? "The agent"} is stopped`));
    foot.append(line, send);

    row.append(foldable(entry.gist ?? entry.text, entry.id, opened), foot);
    return row;
  }

  function answerRow(r: RecentResult, index: number): HTMLElement {
    const row = el("div", "flow-row");
    const line = el("p", "igp-answer");
    line.append(el("b", "", TOOLS[r.result.metadata.tool] ?? r.result.metadata.tool), ` ${answerText(r.result)}`);
    row.append(line, foldable(r.gist ?? r.text, `answer:${r.at}:${index}`, opened), meta(senderName(r.from, r.source), r.toName ?? "agent", r.at, r.cached ? "from cache, 0 tokens" : undefined));
    return row;
  }

  /** The Send anyway on the row now at `index`, else the panel. */
  function focusRow(index: number): void {
    const rows = root?.querySelectorAll<HTMLElement>(".igp-held .igp-send");
    const next = rows?.length ? rows[Math.min(index, rows.length - 1)] : undefined;
    (next ?? root)?.focus();
  }

  function setBusy(id: string, on: boolean): void {
    if (on) busy.add(id);
    else busy.delete(id);
    const b = marked(`send:${id}`) as HTMLButtonElement | undefined;
    if (b) b.disabled = on;
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
    if (ok) focusRow(index);
    else marked(`send:${entry.id}`)?.focus();
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
    try {
      held = await api.list(here);
    } catch {
      if (ticket !== drawn) return;
      const failed = el("div", "flow-empty", "Couldn't read this project's gate files.");
      failed.append(button("igp-btn", "Try again", () => void draw()));
      root.replaceChildren(failed);
      return;
    }
    if (ticket !== drawn) return;

    const parts: HTMLElement[] = [status];

    const heldSec = el("section", "igp-sec igp-held");
    heldSec.setAttribute("aria-label", "Held back");
    if (held.damaged || held.entries.length > 0) heldSec.append(el("h3", "igp-h", held.damaged ? "Held back" : `Held back (${held.entries.length})`));
    if (held.entries.length > 0) heldSec.append(el("p", "igp-note", "Each one leaves this list by itself after 7 days."));
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
      sec.setAttribute("aria-label", "Answered by ByteHive");
      const list = el("div", "flow-list");
      answers.forEach((r, i) => list.append(answerRow(r, i)));
      sec.append(el("h3", "igp-h", `Answered by ByteHive (${answers.length})`), list);
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
      say("");
    },
    setContext(ctx: DockContext | null) {
      const next = ctx?.dir ?? null;
      if (next !== dir) say("");
      dir = next;
      void draw();
    },
  };
}
