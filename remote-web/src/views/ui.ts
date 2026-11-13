// What every page is built from. Names and text from the relay, a machine or an
// agent go in through textContent, never as markup: the only markup here is the
// fixed icons below.

import type { RemoteAgent } from "../../../src/remote/protocol";

export function el<K extends keyof HTMLElementTagNameMap>(tag: K, text?: string, cls?: string): HTMLElementTagNameMap[K] {
  const e = document.createElement(tag);
  if (text !== undefined) e.textContent = text;
  if (cls) e.className = cls;
  return e;
}

export function link(text: string, href: string, cls?: string): HTMLAnchorElement {
  const a = el("a", text, cls);
  a.href = href;
  return a;
}

const ICONS = {
  back: '<path d="M15 5l-7 7 7 7"/>',
  next: '<path d="M9 5l7 7-7 7"/>',
};

export function icon(name: keyof typeof ICONS): HTMLElement {
  const s = el("span", undefined, "icon");
  s.setAttribute("aria-hidden", "true");
  s.innerHTML = `<svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">${ICONS[name]}</svg>`;
  return s;
}

export function logo(size: number): HTMLImageElement {
  const i = el("img", undefined, "logo");
  i.src = "/icons/logo.svg";
  i.alt = "";
  i.width = size;
  i.height = size;
  return i;
}

// The word carries the state; the mark only repeats it, in shape first and colour second.
const STATUS: Record<string, string> = { needs: "Needs you", working: "Working", idle: "Idle", done: "Done", stopped: "Stopped" };
export const statusLabel = (s: string) => STATUS[s] ?? s;

export function mark(state: string): HTMLElement {
  const m = el("span", undefined, "mark");
  m.dataset.state = state;
  m.setAttribute("aria-hidden", "true");
  return m;
}

export const needsYouFirst = (a: RemoteAgent, b: RemoteAgent) => Number(b.status === "needs") - Number(a.status === "needs");

export type LinkState = "connecting" | "up" | "down";
const LINK: Record<LinkState, string> = { connecting: "Connecting", up: "Connected", down: "Reconnecting" };

// One element for every page: it moves into the bar of whichever page is on screen.
const pill = el("div", undefined, "link");
pill.setAttribute("role", "status");
const pillMark = mark("connecting");
const pillText = el("span", LINK.connecting);
pill.append(pillMark, pillText);

export function setLink(state: LinkState): void {
  pillMark.dataset.state = state;
  pillText.textContent = LINK[state];
}

/** The strip at the top of a page: the way back or the brand, what is passed in, and the connection. */
export function bar(o: { back?: { href: string; label: string }; middle?: HTMLElement }): HTMLElement {
  const b = el("header", undefined, "bar");
  if (o.back) {
    const a = link("", o.back.href, "bar-back");
    a.append(icon("back"), el("span", o.back.label, "sr"));
    b.append(a);
  } else {
    const brand = el("div", undefined, "brand");
    brand.append(logo(28), el("span", "ByteHive Remote"));
    b.append(brand);
  }
  b.append(o.middle ?? el("div", undefined, "grow"), pill);
  return b;
}

/** A full page of its own: signing in, a stop, a message. The first paragraph is the one that matters. */
export function gate(root: HTMLElement, title: string, notes: string[], action?: HTMLElement): void {
  const page = el("div", undefined, "gate");
  page.append(logo(72), el("h1", title), ...notes.map((n) => el("p", n)));
  if (action) page.append(action);
  root.replaceChildren(page);
}

const focusKey = (e: Element) => e.getAttribute("href") ?? e.id ?? "";

/** Puts `page` on screen in one step, with the keyboard where it was: a fleet update draws these pages again. */
export function swap(root: HTMLElement, page: HTMLElement): void {
  const was = document.activeElement;
  const key = was && was !== document.body && root.contains(was) ? focusKey(was) : "";
  root.replaceChildren(page);
  if (key) [...root.querySelectorAll<HTMLElement>("a, button")].find((e) => focusKey(e) === key)?.focus();
}

// Read by screen readers only, for what appears on a page that was drawn again.
const spoken = el("div", undefined, "sr");
spoken.setAttribute("role", "status");
export function say(text: string): void {
  if (!spoken.isConnected) document.body.append(spoken);
  spoken.textContent = text;
}

export type Theme = "dark" | "light";
const THEME_KEY = "bytehive.theme";

// Dark unless the person chose light here. Never read from the system: a phone
// that switches at sunset would flip the page under someone reading it.
function storedTheme(): Theme {
  try {
    return localStorage.getItem(THEME_KEY) === "light" ? "light" : "dark";
  } catch {
    return "dark";
  }
}

function applyTheme(t: Theme): void {
  document.documentElement.dataset.theme = t;
  document.querySelector('meta[name="theme-color"]')?.setAttribute("content", t === "light" ? "#f6f6f8" : "#050506");
}

export function themeButton(): HTMLButtonElement {
  const label = () => (document.documentElement.dataset.theme === "light" ? "Dark theme" : "Light theme");
  const b = el("button", label(), "btn quiet");
  b.type = "button";
  b.id = "theme";
  b.onclick = () => {
    const next: Theme = document.documentElement.dataset.theme === "light" ? "dark" : "light";
    applyTheme(next);
    try {
      localStorage.setItem(THEME_KEY, next);
    } catch {
      // Not kept for the next visit, and nothing else depends on it.
    }
    b.textContent = label();
  };
  return b;
}

applyTheme(storedTheme());
