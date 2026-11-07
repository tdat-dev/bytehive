// Notification card over the agent list, bottom left (clear of the stage's
// header and composer) — the mockup's #hint. `topNote(msg, ms)` fades a
// short message in and out; used for the first-run tip and the Tidy / background
// confirmations. Self-contained: injects its own markup + style, no app imports.

const STYLE = `
.hint{position:fixed;left:var(--s6);bottom:calc(var(--ib-dock-h, 84px) + var(--s3));z-index:var(--z-sheet);display:block;
  width:max-content;max-width:min(calc(var(--ib-q, 320px) - 48px), calc(100vw - 48px));font:var(--t-body);color:var(--ink);background:var(--raised);
  border:0;box-shadow:var(--lift),var(--ring-line);padding:var(--s2) var(--s3);border-radius:var(--r-dialog);
  opacity:0;visibility:hidden;transform:translateY(8px);pointer-events:none;
  transition:opacity var(--m-swap) var(--ease-in),transform var(--m-swap) var(--ease-in),visibility var(--m-swap) var(--ease-in)}
.hint.on{opacity:1;visibility:visible;transform:translateY(0)}
.hint b{color:var(--ink)}
.hint kbd{font:var(--t-mono);background:transparent;
  border:0;box-shadow:var(--ring-line);border-radius:var(--r-control);padding:1px var(--s1);color:var(--ink-3)}
`;

let hintEl: HTMLElement | null = null;
let textEl: HTMLElement | null = null;
let timer = 0;

/** Flash a message in the top-center pill for `ms`, then fade it out. */
export function topNote(msg: string, ms = 2600): void {
  if (!hintEl || !textEl) return;
  textEl.innerHTML = msg; // callers pass trusted, app-authored strings only
  hintEl.classList.add("on");
  window.clearTimeout(timer);
  timer = window.setTimeout(() => hintEl?.classList.remove("on"), ms);
}

/** Inject the pill and show the first-run tip once. Call once at startup. */
export function initHint(): void {
  if (document.getElementById("hint")) return;
  const style = document.createElement("style");
  style.textContent = STYLE;
  document.head.appendChild(style);

  hintEl = document.createElement("div");
  hintEl.className = "hint";
  hintEl.id = "hint";
  hintEl.setAttribute("role", "status");
  textEl = document.createElement("span");
  textEl.id = "hintText";
  hintEl.appendChild(textEl);
  document.body.appendChild(hintEl);

  // The first-run tip lives in tips.ts now (welcome), with the others.
}
