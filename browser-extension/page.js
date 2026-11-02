/* Functions injected into pages with chrome.scripting.executeScript. Each one
 * must be self-contained: it runs in the page's isolated world, not here.
 * Refs live in that isolated world (globalThis.__bytehive), so the page's own
 * scripts never see them and the DOM is never marked. */

/** A readable outline of the page, small enough to be cheap for the model:
 *  interactive elements (or everything meaningful with filter "all"), each
 *  with a ref the other tools accept, one short line each.
 *  opts.scope "view" (default) lists what is on screen, "page" everything.
 *  Unless opts.full, a second read of the same page sends only what changed.
 *  opts.k divides coordinates into screenshot pixels. A time budget keeps a
 *  huge page from stalling the tab (and the machine). */
export function snapshotPage(filter, maxItems, opts = {}) {
  const S = (globalThis.__bytehive ??= { refs: new Map(), next: 1, byEl: new WeakMap() });
  const W = innerWidth, H = innerHeight, k = opts.k || 1, onlyView = opts.scope !== "page";
  const t0 = performance.now(), BUDGET = 150;
  const INTERACTIVE = new Set(["a", "button", "input", "select", "textarea", "summary", "details", "option"]);
  const ROLES = /^(button|link|checkbox|radio|switch|tab|menuitem|menuitemcheckbox|menuitemradio|option|combobox|textbox|searchbox|slider|spinbutton|treeitem)$/;
  const clean = (s) => (s ?? "").replace(/\s+/g, " ").trim();
  const cut = (s, n = 60) => (s.length > n ? s.slice(0, n - 1) + "…" : s);
  const shown = (el) => {
    const cs = getComputedStyle(el);
    return cs.visibility !== "hidden" && cs.display !== "none" && Number(cs.opacity) > 0.05;
  };
  const nameOf = (el) => {
    const label = el.getAttribute("aria-label");
    if (label) return clean(label);
    const by = el.getAttribute("aria-labelledby");
    if (by) {
      const t = by.split(/\s+/).map((id) => el.ownerDocument.getElementById(id)?.textContent ?? "").join(" ");
      if (clean(t)) return clean(t);
    }
    if (el.labels?.length) return clean([...el.labels].map((l) => l.textContent).join(" "));
    const tag = el.tagName.toLowerCase();
    if (tag === "input" || tag === "textarea") return clean(el.placeholder || el.title || el.name || "");
    if (tag === "img") return clean(el.alt || el.title || "");
    // textContent, not innerText: no layout pass per element.
    return clean(el.textContent || el.title || el.getAttribute("alt") || "");
  };
  const roleOf = (el) => {
    const r = el.getAttribute("role");
    if (r) return r;
    const tag = el.tagName.toLowerCase();
    if (tag === "a") return el.hasAttribute("href") ? "link" : "text";
    if (tag === "input") {
      const t = (el.type || "text").toLowerCase();
      return t === "checkbox" || t === "radio" ? t : t === "submit" || t === "button" ? "button" : t === "search" ? "searchbox" : "textbox";
    }
    if (tag === "textarea") return "textbox";
    if (tag === "select") return "combobox";
    if (/^h[1-6]$/.test(tag)) return "heading";
    if (el.isContentEditable) return "textbox";
    return tag === "summary" ? "button" : tag;
  };
  // Cheap first: tag, role, attributes. Only then the computed cursor.
  const plainInteractive = (el) => {
    const tag = el.tagName.toLowerCase();
    if (INTERACTIVE.has(tag)) return !(tag === "a" && !el.hasAttribute("href") && !el.onclick);
    const role = el.getAttribute("role");
    if (role && ROLES.test(role)) return true;
    if (el.isContentEditable && el.parentElement && !el.parentElement.isContentEditable) return true;
    return el.hasAttribute("onclick") || el.getAttribute("tabindex") === "0";
  };
  const pointerOnly = (el) => {
    if (getComputedStyle(el).cursor !== "pointer") return false;
    const up = el.parentElement;
    return !!up && getComputedStyle(up).cursor !== "pointer" && !up.closest("a,button,[role=button],[role=link]");
  };
  const isMeaningful = (el) => /^h[1-6]$/i.test(el.tagName) || el.tagName === "IMG" || el.getAttribute("role") === "dialog" || el.getAttribute("role") === "alert";
  const refFor = (el) => {
    let id = S.byEl.get(el);
    if (!id) { id = "e" + S.next++; S.byEl.set(el, id); S.refs.set(id, new WeakRef(el)); }
    return id;
  };
  // Refs of elements long gone are dropped, so the map never grows for ever.
  if (S.refs.size > 3000) for (const [id, w] of S.refs) { const el = w.deref(); if (!el || !el.isConnected) S.refs.delete(id); }
  const now = new Map();
  let below = 0, skipped = 0, late = false;
  const walk = (root) => {
    for (const el of root.querySelectorAll("*")) {
      if (late || performance.now() - t0 > BUDGET) { late = true; return; }
      if (el.shadowRoot) walk(el.shadowRoot);
      if (now.has(S.byEl.get(el)) || el.closest("[data-bytehive-overlay]")) continue;
      const plain = plainInteractive(el);
      const meaningful = filter === "all" && isMeaningful(el);
      if (!plain && !meaningful && !["DIV", "SPAN", "LI", "IMG", "SVG", "LABEL", "TD", "svg"].includes(el.tagName)) continue;
      const r = el.getBoundingClientRect();
      if (r.width < 1 || r.height < 1) continue;
      const inView = r.bottom > 0 && r.top < H && r.right > 0 && r.left < W;
      if (onlyView && !inView) { if (plain) below++; continue; }
      if (!plain && !meaningful && !pointerOnly(el)) continue;
      if (!shown(el)) continue;
      if (now.size >= maxItems) { skipped++; continue; }
      const role = roleOf(el);
      let name = cut(nameOf(el));
      // A clickable box with no words says nothing: leave it out. An icon
      // link says where it goes.
      if (!name && !plain) continue;
      if (!name && el.tagName === "A") name = cut(el.getAttribute("href") ?? "", 40);
      let line = `${refFor(el)} ${role} "${name}"`;
      if ("value" in el && (role === "textbox" || role === "searchbox" || role === "combobox") && el.value) line += ` ="${cut(String(el.value), 40)}"`;
      if (el.checked) line += " checked";
      if (el.disabled || el.getAttribute("aria-disabled") === "true") line += " disabled";
      if (el.getAttribute("aria-expanded")) line += ` expanded=${el.getAttribute("aria-expanded")}`;
      line += inView ? ` @${Math.round((r.left + r.width / 2) / k)},${Math.round((r.top + r.height / 2) / k)}` : " (offscreen)";
      now.set(S.byEl.get(el), line);
    }
  };
  // An open dialog first: it is what the user sees, and modals sit at the end
  // of the page, where a time budget would run out before reaching them.
  for (const d of document.querySelectorAll("dialog[open],[role=dialog],[role=alertdialog],[aria-modal=true]")) walk(d);
  walk(document);
  const frames = document.querySelectorAll("iframe").length;
  const notes = [];
  if (skipped) notes.push(`${skipped} more not listed (raise max)`);
  if (below) notes.push(`${below} more controls off screen (scroll, or scope "page")`);
  if (late) notes.push("stopped early on a very large page: scroll, or use browser_find");
  if (frames) notes.push(`${frames} iframe(s) not included`);
  // Only what changed since the last read of this page, unless asked for all.
  const prev = S.prev;
  let lines = [...now.values()], diff = false;
  if (!opts.full && prev && prev.url === location.href && prev.scope === onlyView) {
    diff = true;
    const added = [...now].filter(([id, l]) => prev.lines.get(id) !== l).map(([, l]) => "+ " + l);
    const gone = [...prev.lines.keys()].filter((id) => !now.has(id)).map((id) => "- " + id);
    const same = now.size - added.length;
    lines = [...added, ...gone];
    notes.unshift(lines.length ? `changes since your last read; ${same} unchanged` : `no changes since your last read (${same} elements)`);
  }
  S.prev = { url: location.href, scope: onlyView, lines: now };
  return { title: document.title, url: location.href, viewport: `${Math.round(W / k)}x${Math.round(H / k)}`, lines, diff, note: notes.join("; ") };
}

/** Before an action: start counting page changes (a MutationObserver on
 *  nodes and text only, disconnected again by settleDelta). */
export function armDelta() {
  const S = (globalThis.__bytehive ??= { refs: new Map(), next: 1, byEl: new WeakMap() });
  S.muts = 0;
  S.lastMut = performance.now();
  S.mo?.disconnect();
  S.mo = new MutationObserver((list) => { S.muts += list.length; S.lastMut = performance.now(); });
  S.mo.observe(document, { subtree: true, childList: true, characterData: true });
  return true;
}

/** After an action: wait until the page has been still for `quiet` ms (at
 *  most `max`), then say how much changed and what has the focus. */
export async function settleDelta(quiet, max) {
  const S = globalThis.__bytehive ?? {};
  const t0 = performance.now();
  await new Promise((done) => {
    const tick = () => {
      const t = performance.now();
      if (t - t0 >= max || (t - t0 >= 80 && t - (S.lastMut ?? 0) >= quiet)) done();
      else setTimeout(tick, 40);
    };
    tick();
  });
  S.mo?.disconnect();
  S.mo = null;
  const a = document.activeElement;
  const focus = a && a !== document.body && a !== document.documentElement
    ? `${a.getAttribute("role") || a.tagName.toLowerCase()} "${(a.getAttribute("aria-label") || a.placeholder || a.value || a.textContent || "").replace(/\s+/g, " ").trim().slice(0, 40)}"`
    : "";
  return { changes: S.muts ?? 0, focus, url: location.href, title: document.title };
}

/** The element behind a ref: scrolled into view, with its centre. */
export function locateRef(ref) {
  ref = String(ref).replace(/^ref_/, "e");
  const el = globalThis.__bytehive?.refs.get(ref)?.deref();
  if (!el || !el.isConnected) return { error: `${ref} is gone from the page. Call browser_read_page again.` };
  el.scrollIntoView({ block: "center", inline: "center", behavior: "instant" });
  const r = el.getBoundingClientRect();
  return { x: Math.round(r.left + r.width / 2), y: Math.round(r.top + r.height / 2), rect: { x: r.left, y: r.top, w: r.width, h: r.height } };
}

/** Set a form control's value the way typing would, so frameworks notice. */
export function fillRef(ref, value) {
  ref = String(ref).replace(/^ref_/, "e");
  const el = globalThis.__bytehive?.refs.get(ref)?.deref();
  if (!el || !el.isConnected) return { error: `${ref} is gone from the page. Call browser_read_page again.` };
  const fire = (t) => el.dispatchEvent(new Event(t, { bubbles: true }));
  const tag = el.tagName.toLowerCase();
  if (tag === "select") {
    const opt = [...el.options].find((o) => o.value === value || o.text.trim() === value);
    if (!opt) return { error: `No option "${value}". Options: ${[...el.options].map((o) => o.text.trim()).join(", ")}` };
    el.value = opt.value;
  } else if (el.type === "checkbox" || el.type === "radio") {
    el.checked = value === true || value === "true" || value === "on" || value === "1";
  } else if (el.isContentEditable) {
    el.focus();
    el.textContent = String(value);
  } else {
    el.focus();
    const proto = tag === "textarea" ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype;
    Object.getOwnPropertyDescriptor(proto, "value").set.call(el, String(value));
  }
  fire("input");
  fire("change");
  return { ok: true };
}

/** Elements whose name or text contains every word of the query, on-screen
 *  ones first. Matching reads textContent (no layout); a time budget bounds
 *  the work on a huge page. Coordinates in screenshot pixels (divided by k). */
export function findInPage(query, maxItems, k = 1) {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  const S = (globalThis.__bytehive ??= { refs: new Map(), next: 1, byEl: new WeakMap() });
  const t0 = performance.now();
  const hits = [];
  const CONTROL = /^(A|BUTTON|INPUT|SELECT|TEXTAREA)$/;
  const has = (t) => { const low = t.toLowerCase(); return words.every((w) => low.includes(w)); };
  for (const el of document.querySelectorAll("a,button,input,select,textarea,[role],[aria-label],label,h1,h2,h3,h4,li,td,span,div,p")) {
    if (hits.length >= maxItems * 3 || performance.now() - t0 > 150) break;
    const text = (el.getAttribute("aria-label") || el.placeholder || el.labels?.[0]?.textContent || el.textContent || "").replace(/\s+/g, " ").trim();
    if (!text || text.length > 200 || !has(text)) continue;
    // keep the innermost match only
    if ([...el.children].some((c) => has(c.textContent || ""))) continue;
    const r = el.getBoundingClientRect();
    if (r.width < 1 || r.height < 1) continue;
    let id = S.byEl.get(el);
    if (!id) { id = "e" + S.next++; S.byEl.set(el, id); S.refs.set(id, new WeakRef(el)); }
    const control = CONTROL.test(el.tagName) || /^(button|link|textbox|checkbox|combobox|tab|menuitem)$/.test(el.getAttribute("role") ?? "");
    const inView = r.bottom > 0 && r.top < innerHeight && r.right > 0 && r.left < innerWidth;
    const at = inView ? `@${Math.round((r.left + r.width / 2) / k)},${Math.round((r.top + r.height / 2) / k)}` : "(offscreen)";
    hits.push({ rank: (control ? 0 : 2) + (inView ? 0 : 1), line: `${id} ${el.getAttribute("role") || el.tagName.toLowerCase()} "${text.slice(0, 60)}" ${at}` });
  }
  // Things you can act on, on screen, first: a field named "Email" before its label.
  return hits.sort((a, b) => a.rank - b.rank).slice(0, maxItems).map((h) => h.line);
}

/** The page's text from character `from`, at most `maxChars` of it. */
export function pageText(maxChars, from = 0) {
  const t = document.body?.innerText ?? "";
  const end = from + maxChars;
  const more = t.length > end ? `\n… ${t.length - end} more characters: call again with from=${end}` : "";
  return { title: document.title, url: location.href, text: t.slice(from, end) + more };
}

/** The name of what sits at a point: the button or link a click there hits. */
export function labelAt(x, y) {
  let el = document.elementsFromPoint(x, y).find((e) => !e.closest("[data-bytehive-overlay]"));
  el = el?.closest("button,a,[role=button],[role=link],[role=menuitem],input[type=submit],input[type=button],[onclick]") ?? el;
  if (!el) return "";
  return (el.getAttribute("aria-label") || el.value || el.innerText || el.title || "").replace(/\s+/g, " ").trim().slice(0, 80);
}

/** Does the page want a person? A visible password field (a login) or a
 *  CAPTCHA widget. Returns "login", "captcha" or "". */
export function blockerOf() {
  const shown = (el) => { const r = el.getBoundingClientRect(); return r.width > 1 && r.height > 1 && getComputedStyle(el).visibility !== "hidden"; };
  const captcha = [...document.querySelectorAll("iframe[src*='recaptcha'],iframe[src*='hcaptcha'],iframe[src*='turnstile'],iframe[src*='captcha'],.g-recaptcha,.h-captcha,.cf-turnstile,#captcha")];
  if (captcha.some(shown)) return "captcha";
  if ([...document.querySelectorAll("input[type=password]")].some(shown)) return "login";
  return "";
}

/** For an upload: mark the file input behind a ref so CDP can find it, or,
 *  when the ref is a button that opens a file chooser, say where to click. */
export function markForUpload(ref, mark) {
  ref = String(ref).replace(/^ref_/, "e");
  const el = globalThis.__bytehive?.refs.get(ref)?.deref();
  if (!el || !el.isConnected) return { error: `${ref} is gone from the page. Call browser_read_page again.` };
  const input = el.matches("input[type=file]")
    ? el
    : el.querySelector("input[type=file]") ?? (el.tagName === "LABEL" && el.control?.type === "file" ? el.control : null);
  el.scrollIntoView({ block: "center", inline: "center", behavior: "instant" });
  const r = el.getBoundingClientRect();
  const at = { x: Math.round(r.left + r.width / 2), y: Math.round(r.top + r.height / 2) };
  if (input) input.setAttribute("data-bytehive-upload", mark);
  return { input: !!input, ...at };
}

export function unmarkUpload(mark) {
  document.querySelector(`[data-bytehive-upload="${mark}"]`)?.removeAttribute("data-bytehive-upload");
  return true;
}
