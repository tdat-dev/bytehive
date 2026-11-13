// A session as a conversation. What an agent wrote is untrusted: it can quote a
// web page, so markdown goes through DOMPurify and everything else through
// textContent. This page holds device keys and never trusts text as markup.

import DOMPurify from "dompurify";
import { Marked } from "marked";
import type { ChatItem, StepItem } from "../../../src/chatmodel";
import type { PromptOption, ToWeb } from "../../../src/remote/protocol";
import type { MachineSession } from "../session";
import { el, mark } from "./ui";

const PAGE = 40;

// A link out of an agent's text opens on its own and takes nothing of this page along.
DOMPurify.addHook("afterSanitizeAttributes", (node) => {
  if (node.tagName === "A") {
    node.setAttribute("target", "_blank");
    node.setAttribute("rel", "noopener noreferrer");
  }
});

const escapeHtml = (t: string) => t.replace(/[&<>"']/g, (c) => `&#${c.charCodeAt(0)};`);

// Raw HTML in an agent's text is shown as the characters it is made of and never becomes
// markup: besides being the safer reading, an element with an event handler in it makes
// the browser report a CSP violation even when the sanitizer is about to remove it.
const md = new Marked({ renderer: { html: ({ text }) => escapeHtml(text) } });

export function renderMarkdown(text: string): string {
  // Without a sanitizer the text is kept as text: DOMPurify would hand the markup back untouched.
  if (!DOMPurify.isSupported) return escapeHtml(text);
  return DOMPurify.sanitize(md.parse(text, { async: false }), {
    // The markdown itself can still make a link or a picture: no pictures (a remote one
    // would tell its server this page was opened, and the page doesn't show them anyway),
    // and DOMPurify drops any link that isn't a plain address.
    FORBID_TAGS: ["style", "form", "iframe", "object", "embed", "img", "input", "button"],
    FORBID_ATTR: ["style"],
  });
}

/** `incoming` over `have`: items that moved on are replaced, new ones are added, an older page goes in front. */
export function mergeItems(have: ChatItem[], incoming: ChatItem[], older: boolean): ChatItem[] {
  if (older) {
    const known = new Set(have.map((i) => i.id));
    return [...incoming.filter((i) => !known.has(i.id)), ...have];
  }
  const out = [...have];
  const place = new Map(have.map((x, n) => [x.id, n]));
  for (const i of incoming) {
    const at = place.get(i.id);
    if (at !== undefined) out[at] = i;
    else place.set(i.id, out.push(i) - 1);
  }
  return out;
}

export type PendingState = "unsent" | "waiting" | "typed";
/** `why`: what the computer said about a message it refused, shown in place of "not sent". */
export interface Pending { id: string; text: string; state: PendingState; why?: string }

/**
 * Messages typed on this page and not yet seen in the conversation. Nothing is
 * ever sent again without a click: a resend uses the same id, so the computer
 * answers the first try's result instead of typing the message twice.
 */
export class PendingList {
  items: Pending[] = [];
  private line: unknown = null;
  constructor(private send: (id: string, text: string) => void) {}

  /**
   * Points sending at the session that is open now. On a new one the old line is
   * gone with whatever it carried. On the same one (a tab switch) nothing was lost:
   * what is waiting is still on its way, and a typed one is in the conversation just asked for.
   */
  bind(send: (id: string, text: string) => void, line: unknown): void {
    if (line !== this.line) this.park();
    else this.items = this.items.filter((x) => x.state !== "typed");
    this.line = line;
    this.send = send;
  }

  add(text: string, online: boolean): string {
    const id = crypto.randomUUID();
    this.items.push({ id, text, state: online ? "waiting" : "unsent" });
    if (online) this.send(id, text);
    return id;
  }

  retry(id: string): void {
    const p = this.items.find((x) => x.id === id);
    if (!p) return;
    p.state = "waiting";
    p.why = undefined;
    this.send(p.id, p.text);
  }

  ack(id: string, state: "typed" | "seen" | "failed"): void {
    const p = this.items.find((x) => x.id === id);
    if (!p) return;
    if (state === "seen") this.items = this.items.filter((x) => x !== p);
    else p.state = state === "failed" ? "unsent" : "typed";
  }

  /** The computer turned down what is on its way, saying `why`. Answers whether anything was. */
  refuse(why: string): boolean {
    const hit = this.items.filter((x) => x.state === "waiting");
    for (const p of hit) {
      p.state = "unsent";
      p.why = why;
    }
    return hit.length > 0;
  }

  /** Nobody is listening any more: a message still waiting may or may not have arrived, so it is offered again; a typed one will show in the conversation. */
  park(): void {
    this.items = this.items.filter((x) => x.state !== "typed");
    for (const p of this.items) p.state = "unsent";
  }
}

/** What was typed here and not yet seen outlives the page of one agent: a dropped line closes it. */
const kept = new Map<string, PendingList>();

// An answer can come while the Terminal tab is open: the lists hear it either way. A no-agent error
// names no agent, so it answers what is on its way on that computer. Saying so of a message that did
// get there costs nothing: Send again uses the same id, and the computer answers the first try's result.
window.addEventListener("remote-message", (e) => {
  const { peer, msg } = (e as CustomEvent<{ peer: string; msg: ToWeb }>).detail;
  if (msg.type === "chat.ack") for (const list of kept.values()) list.ack(msg.id, msg.state);
  if (msg.type === "error" && msg.code === "no-agent") {
    for (const [key, list] of kept) if (key.startsWith(`${peer}/`)) list.refuse(msg.message);
  }
});

const LABEL: Record<PendingState, string> = { unsent: "not sent", waiting: "waiting", typed: "typed" };

/** How far the log is from its end, in pixels. */
const fromEnd = (log: HTMLElement) => log.scrollHeight - log.scrollTop - log.clientHeight;

/** One word for a step that isn't simply done, so colour never says it alone. */
const stepWord = (i: StepItem) => (i.error ? "Failed" : i.done ? "" : "Running");

function stepRow(i: StepItem): HTMLElement {
  const word = stepWord(i);
  const row = el(i.output ? "summary" : "div", undefined, "step-row");
  const what = el("span", undefined, "step-what");
  what.append(el("span", i.verb, "step-verb"), " ", el("span", i.target, i.code ? "step-target code" : "step-target"));
  if (i.full) what.title = i.full;
  row.append(mark(i.error ? "failed" : i.done ? "done" : "working"), what);
  if (word) row.append(el("span", word, "step-word"));
  if (!i.output) return row;
  const more = el("details");
  more.append(row, el("pre", i.output));
  return more;
}

/** One item of a conversation as the log shows it; the saved sessions page draws the same. */
export function chatLine(i: ChatItem): HTMLElement {
  const d = el("div", undefined, `item ${i.kind}`);
  if (i.kind === "text") d.innerHTML = renderMarkdown(i.text);
  else if (i.kind === "user" || i.kind === "note") d.textContent = i.text;
  else d.append(stepRow(i));
  return d;
}

const ENTRY = /^[\s│┃|]*(?:[❯>›]\s*)?\d\.\s/;

/**
 * The screen up to the list of answers: the list is what it ends with (see prompts.ts), and the
 * buttons show it already. Anything else stays: this text is what the person says yes to, so a cut
 * happens only where the numbered lines from there are exactly the answers.
 */
export function questionOf(screen: string, options: PromptOption[]): string {
  const lines = screen.split(/\r?\n/);
  const first = lines.map((l) => /^[\s│┃|]*(?:[❯>›]\s*)?1\.\s/.test(l)).lastIndexOf(true);
  const entries = lines.slice(Math.max(first, 0)).filter((l) => ENTRY.test(l)).length;
  return options.length && first > 0 && entries === options.length ? lines.slice(0, first).join("\n").trimEnd() : screen;
}

/**
 * Where the question starts in the text: just under the rule a CLI draws above it, or null when
 * there is none. The machine sends the last 24 lines of the screen, so what stands above the rule
 * is older output, and what is about to run comes right under it: that is what has to be in view
 * before anyone says yes. A rule in the last lines closes the question instead of opening it.
 */
export function promptStart(text: string): number | null {
  const lines = text.split("\n");
  let at = 0;
  let start: number | null = null;
  lines.forEach((line, n) => {
    at += line.length + 1;
    if (lines.length - n > 2 && /^[\s│┃|]*[╭┌─━═]{3,}/.test(line)) start = at;
  });
  return start;
}

/** Opens the conversation of `agent` in `el`; answers the function that closes it. */
export function openChat(root: HTMLElement, s: MachineSession, peer: string, agent: string, onTerminal: () => void): () => void {
  let items: ChatItem[] = [];
  let loaded = false;
  let older = false;
  let partial = false;
  let note = "";
  let asking: { id: string; screen: string; options: PromptOption[] } | null = null;

  const send = (id: string, text: string) => s.send({ type: "chat.send", agent, id, text });
  const key = `${peer}/${agent}`;
  const pending = kept.get(key) ?? new PendingList(send);
  pending.bind(send, s);
  kept.set(key, pending);

  const log = el("div", undefined, "log");
  log.setAttribute("role", "log");
  log.setAttribute("aria-live", "polite");
  log.setAttribute("aria-label", "Conversation");
  log.tabIndex = 0;
  const jump = el("button", "Jump to latest", "btn raised small jump");
  jump.type = "button";
  jump.hidden = true;
  jump.onclick = () => { log.scrollTop = log.scrollHeight; };
  log.onscroll = () => { jump.hidden = fromEnd(log) < 200; };
  const wrap = el("div", undefined, "log-wrap");
  wrap.append(log, jump);
  const ask = el("div", undefined, "ask");
  ask.setAttribute("aria-live", "assertive");
  const queue = el("div", undefined, "queue");
  const form = el("form", undefined, "composer");
  const box = el("textarea");
  box.rows = 1;
  box.placeholder = "Message the agent";
  box.setAttribute("aria-label", "Message to the agent");
  const go = el("button", "Send", "btn primary");
  go.type = "submit";
  form.append(box, go);
  root.append(wrap, queue, ask, form);

  /** The box grows with what is typed, up to the cap in the page's styles, then scrolls. */
  const fitBox = () => {
    box.style.height = "auto";
    box.style.height = `${box.scrollHeight}px`;
  };
  box.oninput = fitBox;
  // A phone's Enter is a new line; a keyboard's sends. Never while an input method is composing a word.
  const realKeyboard = window.matchMedia?.("(hover: hover) and (pointer: fine)").matches ?? false;
  box.onkeydown = (e) => {
    if (e.key === "Enter" && !e.shiftKey && !e.isComposing && realKeyboard) {
      e.preventDefault();
      form.requestSubmit();
    }
  };

  const line = chatLine;

  /** What stands above the items. Built again only when it changes: the log is read aloud as it grows. */
  let head: Element[] = [];
  let headIs = "";

  function drawHead(): void {
    const now = JSON.stringify([note, older, partial, loaded, items.length === 0]);
    if (now === headIs) return;
    headIs = now;
    const held = head.some((e) => e === document.activeElement);
    for (const e of head) e.remove();
    head = [];
    if (note) head.push(el("p", note, "log-note"));
    else if (!items.length) head.push(el("p", loaded ? "No messages yet. What you send here is typed into the agent's terminal." : "Loading the conversation.", "log-note"));
    if (older || partial) {
      const b = el("button", older ? "Show earlier" : "Load the whole conversation", "btn quiet small");
      b.type = "button";
      b.onclick = () => s.send(older
        ? { type: "chat.page", agent, before: items[0]?.id ?? null, limit: PAGE }
        : { type: "chat.page", agent, before: null, limit: PAGE, whole: true });
      head.push(b);
    }
    log.prepend(...head);
    if (held) log.focus();
  }

  /** The items on screen, by id: only what is new or changed is drawn, and an agent's markdown is the slow part. */
  const drawn = new Map<string, { item: ChatItem; node: HTMLElement }>();

  function drawLog(): void {
    const stuck = fromEnd(log) < 40;
    const end = log.scrollHeight - log.scrollTop;
    const firstBefore = log.querySelector(".item");
    drawHead();
    const left = new Set(drawn.keys());
    const above = head.at(-1);
    let next: ChildNode | null = above ? above.nextSibling : log.firstChild;
    for (const i of items) {
      left.delete(i.id);
      let shown = drawn.get(i.id);
      if (shown && shown.item !== i && JSON.stringify(shown.item) !== JSON.stringify(i)) {
        const node = line(i);
        shown.node.replaceWith(node);
        if (next === shown.node) next = node;
        shown = { item: i, node };
        drawn.set(i.id, shown);
      } else if (!shown) {
        shown = { item: i, node: line(i) };
        drawn.set(i.id, shown);
      }
      if (shown.node === next) next = next.nextSibling;
      else log.insertBefore(shown.node, next);
    }
    for (const id of left) {
      drawn.get(id)!.node.remove();
      drawn.delete(id);
    }
    if (stuck) log.scrollTop = log.scrollHeight;
    // An older page went in above: what was being read stays where it was.
    else if (firstBefore?.isConnected && firstBefore !== log.querySelector(".item")) log.scrollTop = log.scrollHeight - end;
    jump.hidden = fromEnd(log) < 200;
  }

  function drawAsk(): void {
    ask.replaceChildren();
    // One thing to press at a time: while a question is open its answer is the main action, not Send.
    go.classList.toggle("primary", !asking);
    go.classList.toggle("raised", !!asking);
    drawQueue();
    if (!asking) return;
    const title = el("h2", undefined, "ask-title");
    title.append(mark("needs"), el("span", "The agent is asking"));
    const text = questionOf(asking.screen, asking.options);
    const start = promptStart(text);
    const prompt = el("span", text.slice(start ?? text.length));
    const pre = el("pre", text.slice(0, start ?? text.length));
    pre.append(prompt);
    const choices = el("div", undefined, "ask-choices");
    // The first answer is the one the CLI itself has highlighted.
    for (const [n, o] of asking.options.entries()) {
      const b = el("button", o.label, n ? "btn raised" : "btn primary");
      b.type = "button";
      b.onclick = () => {
        s.send({ type: "prompt.answer", agent, id: asking!.id, keys: o.keys });
        asking = null;
        drawAsk();
        // The button is gone. On a phone the box would open the keyboard for nothing.
        if (realKeyboard) box.focus({ preventScroll: true });
      };
      choices.append(b);
    }
    // A question that isn't recognised is answered where it can be read whole.
    if (!asking.options.length) {
      const b = el("button", "Open the terminal to answer", "btn raised");
      b.type = "button";
      b.onclick = onTerminal;
      choices.append(b);
    }
    ask.append(title, pre, choices);
    // It gives way on a short screen, but never below a few lines, and a short question stays short.
    pre.style.minHeight = `${Math.min(100, pre.scrollHeight)}px`;
    // With no rule to go by, the end: a screen with a question ends with it.
    pre.scrollTop = start === null ? pre.scrollHeight : prompt.offsetTop - pre.offsetTop;
  }

  function drawQueue(): void {
    // A question comes first, and a row cut in half would look sent: one whole line until it is answered.
    if (asking && pending.items.length) {
      const stuck = pending.items.filter((p) => p.state === "unsent").length;
      const n = stuck || pending.items.length;
      const held = stuck ? "not sent. Shown again after you answer." : "on the way.";
      queue.replaceChildren(el("p", `${n} message${n === 1 ? "" : "s"} ${held}`, "queue-note"));
      return;
    }
    queue.replaceChildren(...pending.items.map((p) => {
      const row = el("div", undefined, "pending");
      const state = el("span", undefined, "pending-state");
      // The brackets are for a screen reader's pause: on screen the state stands on its own.
      state.append(el("span", " (", "sr"), mark(p.state), p.why ?? LABEL[p.state], el("span", ")", "sr"));
      row.append(el("span", p.text, "pending-text"), state);
      if (p.state === "unsent") {
        const b = el("button", "Send again", "btn raised small");
        b.type = "button";
        b.onclick = () => {
          pending.retry(p.id);
          drawQueue();
        };
        row.append(b);
      }
      return row;
    }));
    // The newest is the one being watched.
    queue.scrollTop = queue.scrollHeight;
  }

  const listener = (e: Event) => {
    const { peer: from, msg } = (e as CustomEvent<{ peer: string; msg: ToWeb }>).detail;
    if (from !== peer) return;
    if (msg.type === "chat.items" && msg.agent === agent) {
      if (msg.reset) {
        // A new parse (a whole load, a new conversation): its ids mean other items than ours.
        items = msg.items;
        older = msg.older;
      } else {
        items = mergeItems(items, msg.items, !msg.live && items.length > 0);
        // A pushed message says nothing about what comes before the items we hold.
        if (!msg.live) older = msg.older;
      }
      partial = msg.partial;
      note = "";
      loaded = true;
      drawLog();
    } else if (msg.type === "chat.ack") {
      drawQueue();
    } else if (msg.type === "prompt.ask" && msg.agent === agent) {
      asking = { id: msg.id, screen: msg.screen, options: msg.options };
      drawAsk();
    } else if (msg.type === "prompt.clear" && msg.agent === agent) {
      asking = null;
      drawAsk();
    } else if (msg.type === "fleet.state") {
      // The agent moved on from its question, on the computer or from another device.
      const now = msg.agents.find((a) => a.id === agent);
      if (asking && now && now.status !== "needs") {
        asking = null;
        drawAsk();
      }
    } else if (msg.type === "error" && msg.code === "no-chat") {
      note = msg.message;
      drawLog();
    } else if (msg.type === "error" && msg.code === "no-agent") {
      // The list has heard it already: it shows on the message when there was one.
      if (pending.items.some((p) => p.why === msg.message)) drawQueue();
      else {
        note = msg.message;
        drawLog();
      }
    }
  };
  window.addEventListener("remote-message", listener);

  form.onsubmit = (e) => {
    e.preventDefault();
    const text = box.value.trim();
    if (!text) return;
    pending.add(text, s.open);
    box.value = "";
    fitBox();
    log.scrollTop = log.scrollHeight;
    drawQueue();
  };

  drawLog();
  drawAsk();
  drawQueue();
  s.send({ type: "chat.page", agent, before: null, limit: PAGE });
  return () => {
    // Leaving closes the page on the computer; a line that dropped has no one left to close.
    if (s.open) s.send({ type: "chat.close", agent });
    window.removeEventListener("remote-message", listener);
    root.replaceChildren();
  };
}
