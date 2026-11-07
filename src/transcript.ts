// A conversation read back from an agent's transcript (chatmodel.ts), drawn
// the way the old chat view drew it: messages, steps that open, questions,
// screenshots. Sessions (History) shows one with it; nothing here is live.

import { marked } from "marked";
import DOMPurify from "dompurify";
import { turnsOf, type ChatImage, type ChatItem, type StepItem } from "./chatmodel";
import { ICON_CLOSE } from "./icons";
import { drawBlocks, type Block } from "./keyeddom";

/** "3m ago", "2h ago", "Sep 20". */
export function ago(at: number, now = Date.now()): string {
  const m = Math.round((now - at) / 60_000);
  if (m < 1) return "just now";
  if (m < 60) return `${m}m ago`;
  const h = Math.round(m / 60);
  if (h < 24) return `${h}h ago`;
  return new Date(at).toLocaleDateString([], { month: "short", day: "numeric" });
}

/** 950 → "950", 45_200 → "45k", 1_300_000 → "1.3M". */
export function tokens(n: number): string {
  if (n < 1000) return String(n);
  if (n < 1_000_000) return `${n < 10_000 ? (n / 1000).toFixed(1).replace(/\.0$/, "") : Math.round(n / 1000)}k`;
  return `${(n / 1_000_000).toFixed(1).replace(/\.0$/, "")}M`;
}

/** 12_000 → "12s", 72_000 → "1m 12s", 3_780_000 → "1h 3m". */
export function took(ms: number): string {
  const s = Math.round(ms / 1000);
  if (s < 60) return `${Math.max(1, s)}s`;
  const m = Math.floor(s / 60);
  if (m < 60) return s % 60 ? `${m}m ${s % 60}s` : `${m}m`;
  return `${Math.floor(m / 60)}h ${m % 60}m`;
}

const clock = (at: number) => new Date(at).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
/** The time for today, the day and time before that. */
export function when(at: number, now = Date.now()): string {
  const d = new Date(at);
  if (d.toDateString() === new Date(now).toDateString()) return clock(at);
  return `${d.toLocaleDateString([], { month: "short", day: "numeric" })}, ${clock(at)}`;
}
/** The same folder, however its slashes and letter case are written. */
export function samePath(a: string, b: string): boolean {
  const norm = (p: string) => p.replace(/\\/g, "/").replace(/\/+$/, "").toLowerCase();
  return norm(a) === norm(b);
}
/** A tooltip's worth of a long value: its first line, cut short. */
const brief = (s: string) => { const line = s.split(/\r?\n/)[0]; return line.length > 200 ? `${line.slice(0, 199)}…` : line; };

const esc = (s: string) => s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]!);

const ICON: Record<string, string> = {
  Ran: `<path d="M4 6l4 4-4 4M10 14h6" />`,
  Read: `<path d="M5 3h7l3 3v11H5z" /><path d="M8 9h5M8 12h5" />`,
  Edited: `<path d="M4 16l1-4 8-8 3 3-8 8z" />`,
  Wrote: `<path d="M5 3h7l3 3v11H5z" /><path d="M10 8v6M7 11h6" />`,
  search: `<circle cx="9" cy="9" r="5" /><path d="M13 13l4 4" />`,
  plan: `<path d="M4 6h2M4 10h2M4 14h2M9 6h7M9 10h7M9 14h7" />`,
  agent: `<circle cx="10" cy="7" r="3" /><path d="M4 17c1-3 3.5-4 6-4s5 1 6 4" />`,
  tool: `<path d="M12 4a4 4 0 0 0-4 5l-4 4 3 3 4-4a4 4 0 0 0 5-4l-2 2-2-1-1-2z" />`,
  shot: `<path d="M3 7h3l1.5-2h5L14 7h3v9H3z" /><circle cx="10" cy="11.5" r="2.8" />`,
  image: `<rect x="3" y="4" width="14" height="12" rx="2" /><circle cx="7.5" cy="8.5" r="1.3" /><path d="M3 14l4-3.5 3 2.5 3-2.5 4 3.5" />`,
  ask: `<path d="M4 5h12v8H9l-3 3v-3H4z" /><path d="M8.5 7.8a1.6 1.6 0 1 1 2.2 1.5c-.5.2-.7.5-.7 1" /><path d="M10 11.6v.1" />`,
};
function iconFor(s: StepItem): string {
  const k = s.verb in ICON ? s.verb
    : s.verb === "Took a screenshot" ? "shot"
    : s.verb === "Looked at" ? "image"
    : s.tool === "AskUserQuestion" ? "ask"
    : /Search|Looked for|Listed/.test(s.verb) ? "search"
    : s.tool === "TodoWrite" ? "plan"
    : /helper agent/.test(s.verb) ? "agent" : "tool";
  return `<svg class="cv-ic" viewBox="0 0 20 20" aria-hidden="true">${ICON[k]}</svg>`;
}

// Parsing markdown is the costly part of a redraw; a message never changes once
// written, so each is parsed once.
const mdCache = new Map<string, string>();
function markdown(id: string, text: string): string {
  const key = `${id}:${text.length}`;
  let html = mdCache.get(key);
  if (html === undefined) {
    if (mdCache.size > 4000) mdCache.clear();
    html = renderMarkdown(text);
    mdCache.set(key, html);
  }
  return html;
}

/** A reply's markdown as the chat draws it: links, Copy on code, files that open. */
function renderMarkdown(text: string): string {
  const t = document.createElement("template");
  // breaks: a line the agent ends is a line in the chat, as in the terminal
  // (one number per line was drawn as "1 2 3 … 40").
  t.innerHTML = DOMPurify.sanitize(marked.parse(text, { async: false, gfm: true, breaks: true }) as string);
  // Links say where they go; a click opens them in the browser (see mount),
  // never in the app's own window.
  for (const a of t.content.querySelectorAll("a[href]")) { a.setAttribute("rel", "noopener noreferrer"); a.setAttribute("title", a.getAttribute("href")!); }
  // A code block copies with one click.
  for (const pre of t.content.querySelectorAll("pre")) pre.insertAdjacentHTML("beforeend", `<button type="button" class="cv-cc" data-copy-code aria-label="Copy this code">Copy</button>`);
  // A file named in the answer (`src/app.ts:42`) opens in the code panel.
  for (const code of t.content.querySelectorAll("code")) {
    if (code.closest("pre")) continue;
    const ref = fileRef(code.textContent ?? "");
    if (ref) { code.classList.add("cv-fref"); code.setAttribute("data-file-ref", ref.path); code.setAttribute("title", `Open ${ref.path}`); code.setAttribute("tabindex", "0"); code.setAttribute("role", "link"); }
  }
  return t.innerHTML;
}

/** A file path in an answer's inline code: `src/app.ts`, `D:\x\y.rs:12`, `chatview.ts:40:3`. */
export function fileRef(text: string): { path: string; line?: number } | null {
  const m = /^((?:[A-Za-z]:[\\/])?[\w.@()[\]\-\\/]*[\w\-]\.[A-Za-z][A-Za-z0-9]{0,7})(?::(\d+)(?::\d+)?)?$/.exec(text.trim());
  if (!m) return null;
  const path = m[1];
  // a version (1.2.3), a domain (example.com) or a method call (x.map) is not a file
  if (/^\d/.test(path) || /^(?:www\.|[a-z]+\.(?:com|org|net|io|dev|app)$)/i.test(path)) return null;
  if (!/[\\/]/.test(path) && !/\.(?:[jt]sx?|mjs|cjs|rs|py|go|rb|java|kt|swift|c|cc|cpp|h|hpp|cs|css|scss|html|vue|svelte|json|toml|ya?ml|md|txt|sh|ps1|sql|lock|env)$/i.test(path)) return null;
  return { path, line: m[2] ? Number(m[2]) : undefined };
}

const urls = new WeakMap<ChatImage, string>();

/** A URL an <img> can show this picture from. */
function urlOf(img: ChatImage): string {
  let u = urls.get(img);
  if (u) return u;
  try {
    const bin = atob(img.data.replace(/\s+/g, ""));
    const bytes = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
    u = URL.createObjectURL(new Blob([bytes], { type: img.media }));
  } catch {
    u = `data:${img.media};base64,${img.data}`;
  }
  urls.set(img, u);
  return u;
}
/** Let go of the pictures of a conversation the view no longer shows. */
export function forgetImages(items: ChatItem[]): void {
  for (const it of items) {
    const pics = it.kind === "step" ? it.images : it.kind === "user" ? it.pics : undefined;
    for (const p of pics ?? []) {
      const u = urls.get(p);
      if (u?.startsWith("blob:")) URL.revokeObjectURL(u);
      urls.delete(p);
    }
  }
}

/** Most a small picture is blown up: past this it is only blur. */
const MAX_GROW = 3;
/** The size to show a w×h picture at so a small one fills its box (up to
 *  MAX_GROW times its size); null when it is big enough already (CSS shrinks
 *  big ones to fit). */
export function grownSize(w: number, h: number, boxW: number, boxH: number): { w: number; h: number } | null {
  if (!(w > 0 && h > 0 && boxW > 0 && boxH > 0)) return null;
  const s = Math.min(boxW / w, boxH / h, MAX_GROW);
  return s > 1.05 ? { w: Math.round(w * s), h: Math.round(h * s) } : null;
}

/** What a step saw (its screenshots), or what you pasted: thumbnails that open full size. */
function shotsHtml(owner: string, pics: ChatImage[], what: string): string {
  return `<div class="cv-shots">${pics.map((p, k) => `<button type="button" class="cv-shot" data-shot="${owner}" data-k="${k}" aria-label="${what} ${k + 1} of ${pics.length}, open full size"><img src="${urlOf(p)}" alt="" decoding="async"></button>`).join("")}</div>`;
}

function stepBody(s: StepItem): string {
  if (s.todos?.length) {
    return `<ul class="cv-todos">${s.todos.map((t) => `<li class="${t.state}"><i aria-hidden="true"></i>${esc(t.text)}</li>`).join("")}</ul>`;
  }
  if (s.diff?.length) {
    // Each line is its own block: no newline between them, or every line doubles.
    return `<pre class="cv-diff">${s.diff.map((l) => `<span class="${l.sign === "+" ? "a" : l.sign === "-" ? "d" : ""}">${esc(l.sign)} ${esc(l.text)}</span>`).join("")}</pre>`;
  }
  const head = s.full && s.full !== s.target ? `<code class="cv-full">${esc(s.full)}</code>` : "";
  const out = s.output ? `<pre class="cv-out${s.error ? " err" : ""}">${esc(s.output)}</pre>` : s.done && !s.images?.length ? `<p class="cv-none">No output</p>` : "";
  return head + out;
}

function hasBody(s: StepItem): boolean {
  return !!(s.todos?.length || s.diff?.length || s.output || (s.full && s.full !== s.target));
}

function stepHtml(s: StepItem, open: boolean): string {
  const counts = s.added || s.removed ? `<span class="cv-n"><b class="a">+${s.added ?? 0}</b> <b class="d">−${s.removed ?? 0}</b></span>` : "";
  const state = !s.done ? `<span class="cv-spin" role="img" aria-label="Running"></span>` : s.error ? `<span class="cv-err">Failed</span>` : "";
  const body = hasBody(s);
  const inner = `${iconFor(s)}<span class="cv-verb">${esc(s.verb)}</span><span class="cv-tgt${s.code ? " code" : ""}">${esc(s.target)}</span>${counts}${state}`;
  const title = esc(brief(s.full ?? s.target));
  // A step with nothing more to show is a plain line, not a dead button.
  return `<div class="cv-step${s.error ? " err" : ""}${open ? " open" : ""}" data-id="${s.id}">${body
    ? `<button type="button" class="cv-sh" aria-expanded="${open}" title="${title}">${inner}<svg class="i cv-chev" aria-hidden="true"><use href="#i-chevron-right"/></svg></button>`
    : `<div class="cv-sh cv-flat" title="${title}">${inner}</div>`}${open && body ? `<div class="cv-sb">${stepBody(s)}</div>` : ""}${s.images?.length ? shotsHtml(s.id, s.images, s.verb === "Took a screenshot" ? "Screenshot" : "Image") : ""}</div>`;
}

function itemHtml(it: ChatItem, open: Set<string>): string {
  switch (it.kind) {
    case "user":
      return `<div class="cv-u" data-id="${it.id}"><div class="cv-bubble">${it.pics?.length ? shotsHtml(it.id, it.pics, "Image") : ""}${esc(it.text)}${it.images && !it.pics?.length ? `<span class="cv-img">${it.images} image${it.images === 1 ? "" : "s"}</span>` : ""}</div>${it.text
        ? `<div class="cv-uacts"><button type="button" data-edit-msg="${it.id}" title="Put it back in the message box to change and send again">Edit</button><button type="button" data-copy-msg="${it.id}" title="Copy this message">Copy</button></div>` : ""}</div>`;
    case "text":
      return `<div class="cv-a" data-id="${it.id}">${markdown(it.id, it.text)}</div>`;
    case "note":
      return `<div class="cv-note" data-id="${it.id}"><span>${esc(it.text)}</span></div>`;
    case "step":
      return stepHtml(it, open.has(it.id) || (it.tool === "TodoWrite" && !open.has(`!${it.id}`)));
  }
}

/** The conversation by turns: each of your messages, what followed, and for a
 *  finished turn a footer (how long it worked, when, Copy). */
function threadBlocks(items: ChatItem[], open: Set<string>, expanded: Set<string>, working: boolean, live: LiveOption[] = [], stopped = false): Block[] {
  const turns = turnsOf(items);
  return turns.flatMap((t, k) => {
    const said = t.items.some((i) => i.kind === "text");
    const done = !(working && k === turns.length - 1);
    const lastAt = t.items[t.items.length - 1]?.at ?? 0;
    const foot = said && done
      ? `<div class="cv-tf">${t.took ? `<span>Worked for ${took(t.took)}</span>` : ""}${lastAt ? `<time>${when(lastAt)}</time>` : ""}<button type="button" class="cv-copy" data-copy="${t.items[0].id}" aria-label="Copy this answer">Copy</button></div>`
      : "";
    return [...runsBlocks(t.items, open, expanded, live, stopped), { key: `tf:${t.items[0].id}`, html: foot }];
  });
}

/** A conversation read back, not live (Sessions): drawn as the chat draws
 *  it, after `before` (notes, "Show earlier"), with nothing waiting on you. */
export function drawTranscript(host: HTMLElement, before: Block[], items: ChatItem[], open: Set<string>, expanded: Set<string>): void {
  drawBlocks(host, [...before, ...threadBlocks(items, open, expanded, false, [], true)]);
}

/** Steps between two messages sit together; a long run folds its start. Each
 *  message and step is its own block, so a new step redraws nothing before it. */
function runsBlocks(items: ChatItem[], open: Set<string>, expanded: Set<string>, live: LiveOption[] = [], stopped = false): Block[] {
  const out: Block[] = [];
  let run: StepItem[] = [];
  const flush = () => {
    if (!run.length) return;
    const key = run[0].id;
    const long = run.length > 5;
    const fold = long && !expanded.has(key);
    const shown = fold ? run.slice(-3) : run;
    const toggle = !long ? ""
      : fold ? `<button type="button" class="cv-more" data-expand="${key}" aria-expanded="false">${run.length - 3} earlier steps</button>`
      : `<button type="button" class="cv-more" data-fold="${key}" aria-expanded="true">Fold ${run.length - 3} earlier steps</button>`;
    out.push({
      key: `steps:${key}`,
      html: `<div class="cv-steps"></div>`,
      kids: [{ key: "more", html: toggle }, ...shown.map((s) => ({ key: s.id, html: stepHtml(s, open.has(s.id) || (s.tool === "TodoWrite" && !open.has(`!${s.id}`))) }))],
    });
    run = [];
  };
  for (const it of items) {
    // A question to you is part of the conversation, never folded away with the steps.
    if (it.kind === "step" && it.questions?.length) { flush(); out.push({ key: it.id, html: askHtml(it, live, stopped) }); continue; }
    if (it.kind === "step") { run.push(it); continue; }
    flush();
    out.push({ key: it.id, html: itemHtml(it, open) });
  }
  flush();
  return out;
}

/** An option of the question on the agent's screen right now, and the number that picks it. */
export interface LiveOption { n: number; label: string }

/** The option on screen that is this one of the transcript's: the same words,
 *  or one cut short of the other ("Chat left + work panel…"). */
export function liveMatch(label: string, live: LiveOption[]): LiveOption | undefined {
  const norm = (t: string) => t.toLowerCase().replace(/[…]|\.\.\.$/g, "").replace(/\s+/g, " ").trim();
  const want = norm(label);
  if (!want) return undefined;
  return live.find((o) => norm(o.label) === want)
    ?? live.find((o) => { const got = norm(o.label); return got.length >= 4 && (want.startsWith(got) || got.startsWith(want)); });
}

/** What the agent asked you (AskUserQuestion): the question, its options with
 *  what each means, and what you picked, or that it waits for you. While it
 *  waits, the options of the question on its screen pick with a click. */
function askHtml(s: StepItem, live: LiveOption[] = [], stopped = false): string {
  // An agent that stopped is not waiting any more, whatever its last question.
  const lapsed = !s.done && stopped;
  const waiting = !s.done && !stopped;
  const turnedDown = s.done && s.error;
  let clickable = false;
  const qs = (s.questions ?? []).map((q) => {
    const picked = new Set((q.answer ?? "").split(/,\s*/).filter(Boolean));
    const ownWords = q.answer && !q.options.some((o) => picked.has(o.label)) ? q.answer : "";
    return `<section class="cv-q">${q.header ? `<span class="cv-qh">${esc(q.header)}</span>` : ""}<p class="cv-qt">${esc(q.question)}</p>
      <ul class="cv-qo${q.multi ? " multi" : ""}">${q.options.map((o) => {
        const on = picked.has(o.label);
        const inner = `<span class="cv-qm" aria-hidden="true"></span><span class="cv-ql"><b>${esc(o.label)}</b>${o.description ? `<span>${esc(o.description)}</span>` : ""}</span>${on ? `<span class="ia-sr">(your answer)</span>` : ""}`;
        const pick = waiting && !q.multi ? liveMatch(o.label, live) : undefined;
        if (pick) clickable = true;
        return pick
          ? `<li class="pick"><button type="button" class="cv-qb" data-answer="${pick.n}" title="Answer “${esc(o.label)}”">${inner}</button></li>`
          : `<li${on ? ` class="on"` : ""}>${inner}</li>`;
      }).join("")}</ul>${ownWords ? `<p class="cv-qa">You answered: ${esc(ownWords)}</p>` : ""}</section>`;
  }).join("");
  const note = waiting ? `<p class="cv-qs wait"><span class="cv-dots" aria-hidden="true"><i></i><i></i><i></i></span>${clickable
    ? "Waiting for your answer: pick one here, or answer in your own words in the terminal."
    : "Waiting for your answer: pick it in the terminal."}</p>`
    : turnedDown ? `<p class="cv-qs">You chose to talk it over instead of picking.</p>`
    : lapsed ? `<p class="cv-qs">It stopped before you answered.</p>` : "";
  return `<div class="cv-ask${waiting ? " waiting" : ""}" data-id="${s.id}" role="group" aria-label="${esc(s.verb)} ${esc(s.target)}">
    <div class="cv-ask-h">${iconFor(s)}<span>${waiting ? "Asks you" : "Asked you"}</span></div>${qs}${note}</div>`;
}

/** A picture full size over everything: ← → between the pictures of the same
 *  step, Esc or a click outside closes it, and the keyboard goes back. */
export function openImage(pics: ChatImage[], start: number, what: string): void {
  document.querySelector(".cv-lb")?.remove();
  const back = document.activeElement as HTMLElement | null;
  let k = Math.max(0, Math.min(pics.length - 1, start));
  const el = document.createElement("div");
  el.className = "inbox-modal-back cv-lb";
  el.innerHTML = `<div class="cv-lb-box" role="dialog" aria-modal="true" aria-label="${esc(what)}">
      <header class="cv-lb-bar"><span class="cv-lb-t">${esc(what)}</span><span class="cv-lb-n"></span>
        <button type="button" class="im-x" data-lb-close aria-label="Close" title="Close (Esc)">${ICON_CLOSE}</button></header>
      <div class="cv-lb-stage">
        <button type="button" class="cv-lb-nav prev" data-lb="-1" aria-label="Previous picture"><svg class="i" aria-hidden="true"><use href="#i-chevron-left"/></svg></button>
        <img class="cv-lb-img" src="${urlOf(pics[k])}" alt="${esc(what)}">
        <button type="button" class="cv-lb-nav next" data-lb="1" aria-label="Next picture"><svg class="i" aria-hidden="true"><use href="#i-chevron-right"/></svg></button>
      </div></div>`;
  document.body.appendChild(el);
  const img = el.querySelector<HTMLImageElement>(".cv-lb-img")!;
  const count = el.querySelector<HTMLElement>(".cv-lb-n")!;
  // Full size means at least filling the screen's box, not a 160-pixel crop in the middle.
  img.addEventListener("load", () => {
    const size = grownSize(img.naturalWidth, img.naturalHeight, window.innerWidth - 112, window.innerHeight - 120);
    img.style.width = size ? `${size.w}px` : "";
    img.style.height = size ? `${size.h}px` : "";
  });
  const show = () => {
    img.src = urlOf(pics[k]);
    img.alt = `${what}, picture ${k + 1} of ${pics.length}`;
    count.textContent = pics.length > 1 ? `${k + 1} / ${pics.length}` : "";
    el.querySelectorAll<HTMLButtonElement>(".cv-lb-nav").forEach((b) => { b.hidden = pics.length < 2; });
  };
  const close = () => { el.remove(); if (back?.isConnected) back.focus(); };
  const step = (d: number) => { k = (k + d + pics.length) % pics.length; show(); };
  el.addEventListener("click", (e) => {
    const t = e.target as HTMLElement;
    if (t.closest("[data-lb-close]") || t === el || t.classList.contains("cv-lb-stage")) { close(); return; }
    const nav = t.closest<HTMLElement>("[data-lb]");
    if (nav) step(Number(nav.dataset.lb));
  });
  el.addEventListener("keydown", (e) => {
    if (e.key === "Escape") { e.preventDefault(); e.stopPropagation(); close(); }
    else if (e.key === "ArrowRight" && pics.length > 1) { e.preventDefault(); step(1); }
    else if (e.key === "ArrowLeft" && pics.length > 1) { e.preventDefault(); step(-1); }
    e.stopPropagation();
  });
  show();
  el.querySelector<HTMLElement>("[data-lb-close]")!.focus();
}
