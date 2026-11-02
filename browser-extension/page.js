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

/** Wait in the page, without screenshots: until `text` or `selector` shows
 *  (or with `gone`, is no longer there), or with `settle`, until the page has
 *  been still for 500 ms. A MutationObserver wakes it; checks are debounced,
 *  so a busy page costs little. Gives up after `ms`. */
export async function waitInPage(o) {
  const t0 = performance.now();
  const took = () => Math.round(performance.now() - t0);
  if (o.settle) {
    let last = performance.now();
    const mo = new MutationObserver(() => { last = performance.now(); });
    mo.observe(document, { subtree: true, childList: true, characterData: true, attributes: true });
    await new Promise((done) => {
      const tick = () => { const n = performance.now(); if (n - last >= 500 || n - t0 >= o.ms) done(); else setTimeout(tick, 100); };
      tick();
    });
    mo.disconnect();
    return { ok: performance.now() - t0 < o.ms, ms: took() };
  }
  const low = (o.text ?? "").toLowerCase();
  const there = () => {
    if (o.selector) { const el = document.querySelector(o.selector); return !!el && el.getClientRects().length > 0; }
    return !!low && (document.body?.innerText ?? "").toLowerCase().includes(low);
  };
  const want = () => (o.gone ? !there() : there());
  if (want()) return { ok: true, ms: 0 };
  return new Promise((done) => {
    let timer = 0, over = false;
    const finish = (ok) => { if (over) return; over = true; mo.disconnect(); clearTimeout(timer); clearTimeout(end); clearInterval(poll); done({ ok, ms: took() }); };
    const check = () => { timer = 0; if (want()) finish(true); };
    const mo = new MutationObserver(() => { if (!timer) timer = setTimeout(check, 80); });
    mo.observe(document, { subtree: true, childList: true, characterData: true });
    // Style changes (shown, hidden) do not always touch the DOM tree.
    const poll = setInterval(check, 500);
    const end = setTimeout(() => finish(false), o.ms);
  });
}

/** Watch part of the page for `ms` and say what text appears in it, line by
 *  line with the second it came: a chat reply, a price, a status. Stops early
 *  when a line holds `until`. Text only: no screenshots. */
export async function observeText(o) {
  const S = globalThis.__bytehive ?? {};
  const el = o.ref ? S.refs?.get(String(o.ref).replace(/^ref_/, "e"))?.deref() : o.selector ? document.querySelector(o.selector) : document.body;
  if (!el || !el.isConnected) return { error: "Nothing to watch: no element matches. Give a selector, or a ref from browser_read_page." };
  const lines = () => (el.innerText ?? "").split("\n").map((s) => s.trim()).filter(Boolean);
  const seen = new Set(lines());
  const until = (o.until ?? "").toLowerCase();
  const t0 = performance.now();
  const events = [];
  let chars = 0, more = 0;
  return new Promise((done) => {
    let timer = 0, over = false;
    const finish = (why) => { if (over) return; over = true; mo.disconnect(); clearTimeout(timer); clearTimeout(end); done({ events, more, why, ms: Math.round(performance.now() - t0) }); };
    const take = () => {
      timer = 0;
      for (const l of lines()) {
        if (seen.has(l)) continue;
        seen.add(l);
        if (chars < 4000) { events.push(`+${((performance.now() - t0) / 1000).toFixed(1)}s ${l.slice(0, 200)}`); chars += Math.min(200, l.length); } else more++;
        if (until && l.toLowerCase().includes(until)) return finish("until");
      }
    };
    // Changes are read at most four times a second, however busy the page.
    const mo = new MutationObserver(() => { if (!timer) timer = setTimeout(take, 250); });
    mo.observe(el, { subtree: true, childList: true, characterData: true });
    const end = setTimeout(() => { take(); finish("time"); }, o.ms);
  });
}

/** Frames of a video as one contact sheet (a grid with each frame's time),
 *  drawn in the page from the decoded video: one small image, no screenshots,
 *  nothing heavy. `times` in seconds, or `count` spread over the video, or
 *  one `every` seconds from `from`. The video's place and play state come back
 *  as they were. A video another site serves without CORS cannot be read
 *  here: then `tainted` says so and the extension photographs it instead. */
export async function videoFrames(o) {
  // The ref or selector given (or a video in it), else the largest video.
  const S0 = globalThis.__bytehive ?? {};
  let v = o.ref ? S0.refs?.get(String(o.ref).replace(/^ref_/, "e"))?.deref() : o.selector ? document.querySelector(o.selector) : null;
  if (v && v.tagName !== "VIDEO") v = v.querySelector("video");
  v ??= [...document.querySelectorAll("video")].sort((a, b) => b.clientWidth * b.clientHeight - a.clientWidth * a.clientHeight)[0] ?? null;
  if (!v) return { error: "No video on this page." };
  if (v.readyState < 1) await new Promise((r) => { v.addEventListener("loadedmetadata", r, { once: true }); setTimeout(r, 4000); });
  const dur = Number.isFinite(v.duration) ? v.duration : 0;
  const n = Math.min(16, Math.max(1, Number(o.count) || 6));
  let times = (o.times ?? []).map(Number).filter((t) => t >= 0 && (!dur || t <= dur));
  if (!times.length && o.every && dur) for (let t = Number(o.from) || 0; t <= dur && times.length < n; t += Number(o.every)) times.push(t);
  if (!times.length) times = dur ? Array.from({ length: n }, (_, i) => (dur * (i + 0.5)) / n) : [v.currentTime];
  times = times.slice(0, 16);
  const was = { t: v.currentTime, paused: v.paused };
  v.pause();
  const seek = (t) => new Promise((r) => {
    if (Math.abs(v.currentTime - t) < 0.01 && v.readyState >= 2) return r();
    const ok = () => { clearTimeout(slow); r(); };
    const slow = setTimeout(ok, 3000);
    v.addEventListener("seeked", ok, { once: true });
    v.currentTime = t;
  });
  const w = Math.min(480, Math.max(120, Number(o.width) || 320));
  const h = Math.round((w * (v.videoHeight || 9)) / (v.videoWidth || 16));
  const cols = Math.ceil(Math.sqrt(times.length)), rows = Math.ceil(times.length / cols);
  const c = document.createElement("canvas");
  c.width = cols * w;
  c.height = rows * h;
  const g = c.getContext("2d");
  // A short video in tenths of a second, so two frames never share a label.
  const stamp = (t) => (dur && dur < 60 ? `${t.toFixed(1)}s` : `${Math.floor(t / 60)}:${String(Math.floor(t % 60)).padStart(2, "0")}`);
  let data = null, tainted = false;
  try {
    for (let i = 0; i < times.length; i++) {
      await seek(times[i]);
      const x = (i % cols) * w, y = Math.floor(i / cols) * h;
      g.drawImage(v, x, y, w, h);
      g.fillStyle = "rgba(0,0,0,.6)";
      g.fillRect(x + 4, y + 4, 46, 18);
      g.fillStyle = "#fff";
      g.font = "12px system-ui, sans-serif";
      g.fillText(stamp(times[i]), x + 9, y + 17);
    }
    data = c.toDataURL("image/jpeg", 0.72);
  } catch {
    tainted = true;
  }
  v.currentTime = was.t;
  if (!was.paused) v.play().catch(() => {});
  return { data, tainted, times, stamps: times.map(stamp), duration: dur, size: `${c.width}x${c.height}` };
}

/** For a video that can't be read in the page: show the frame at `t` and say
 *  where the video sits, so the extension can photograph that part. */
export async function seekVideo(o, t) {
  const S0 = globalThis.__bytehive ?? {};
  let v = o.ref ? S0.refs?.get(String(o.ref).replace(/^ref_/, "e"))?.deref() : o.selector ? document.querySelector(o.selector) : null;
  if (v && v.tagName !== "VIDEO") v = v.querySelector("video");
  v ??= [...document.querySelectorAll("video")].sort((a, b) => b.clientWidth * b.clientHeight - a.clientWidth * a.clientHeight)[0] ?? null;
  if (!v) return { error: "No video on this page." };
  v.pause();
  await new Promise((r) => { const ok = () => { clearTimeout(s); r(); }; const s = setTimeout(ok, 3000); v.addEventListener("seeked", ok, { once: true }); v.currentTime = t; });
  await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
  const b = v.getBoundingClientRect();
  return { x: b.left, y: b.top, w: b.width, h: b.height };
}
