// The key probe from the UX audit: with a terminal focused, every combo reaches
// the terminal and fires no app action. Real key events in a throwaway headless
// Chrome, the real app from a dev server, a fake Tauri backend. It starts no server:
//   node node_modules/vite/bin/vite.js --port 17641 --strictPort
//   node tools/ux_probe.mjs http://localhost:17641 [folder for shots]
// Exits 1 when a check fails. Prints one line per combo and a summary.
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const CHROME = "C:/Program Files/Google/Chrome/Application/chrome.exe";
const PORT = 9561;

const [base, shots] = process.argv.slice(2);
if (!base) {
  console.error("usage: node tools/ux_probe.mjs <dev server url> [folder for shots]");
  process.exit(2);
}
if (shots) fs.mkdirSync(shots, { recursive: true });

const ALT = 1, CTRL = 2, SHIFT = 8;
const letter = (c) => ({ key: c, code: `Key${c.toUpperCase()}`, vk: c.toUpperCase().charCodeAt(0) });
const digit = (d) => ({ key: d, code: `Digit${d}`, vk: d.charCodeAt(0) });
const named = (key, code, vk) => ({ key, code, vk });

// What the audit measured, the brief's list, and the keys no CLI should lose.
const COMBOS = [
  ["Ctrl+K", CTRL, letter("k")],
  ["Ctrl+,", CTRL, named(",", "Comma", 188)],
  ["Ctrl+/", CTRL, named("/", "Slash", 191)],
  ["Ctrl+Tab", CTRL, named("Tab", "Tab", 9)],
  ["Ctrl+Shift+Tab", CTRL | SHIFT, named("Tab", "Tab", 9)],
  ...["K", "J", "D", "O", "T", "B", "L", "M", "E"].map((c) => [`Ctrl+Shift+${c}`, CTRL | SHIFT, letter(c)]),
  ...["s", "r", "p", "o", "h"].map((c) => [`Alt+${c.toUpperCase()}`, ALT, letter(c)]),
  ["Alt+Up", ALT, named("ArrowUp", "ArrowUp", 38)],
  ["Alt+Down", ALT, named("ArrowDown", "ArrowDown", 40)],
  ...["1", "5", "9"].map((d) => [`Alt+${d}`, ALT, digit(d)]),
  ["Ctrl+=", CTRL, named("=", "Equal", 187)],
  ["Ctrl+-", CTRL, named("-", "Minus", 189)],
  ["Ctrl+0", CTRL, digit("0")],
  ["?", SHIFT, named("?", "Slash", 191)],
  ["Esc", 0, named("Escape", "Escape", 27)],
];

// A fake backend, in the page before the app loads. `__probe` is what the probe reads.
const FAKE = String.raw`
(() => {
  const callbacks = new Map();
  let nextCb = 1;
  const probe = window.__probe = { input: [], channels: [], keys: 0 };
  window.addEventListener("keydown", () => { probe.keys++; }, true);
  const lists = new Set(["programs_on_path", "git_repos_under", "workspace_files", "claude_live_sessions", "claude_sessions", "agent_changes", "git_changed_files", "browser_profiles", "fs_read_dir"]);
  const bytes = (s) => new TextEncoder().encode(s).buffer;
  probe.write = (s) => { for (const ch of probe.channels) callbacks.get(ch.id)?.({ index: ch.n++, message: bytes(s) }); };
  window.__TAURI_INTERNALS__ = {
    metadata: { currentWindow: { label: "main" }, currentWebview: { windowLabel: "main", label: "main" } },
    transformCallback(cb) { const id = nextCb++; callbacks.set(id, cb); return id; },
    unregisterCallback(id) { callbacks.delete(id); },
    callbacks,
    async invoke(cmd, args) {
      if (cmd === "pty_spawn") {
        const ch = { id: args.onBytes.id, n: 0 };
        probe.channels.push(ch);
        setTimeout(() => callbacks.get(ch.id)?.({ index: ch.n++, message: bytes("probe> ") }), 50);
        return probe.channels.length;
      }
      if (cmd === "pty_input") { probe.input.push(args.data); return null; }
      if (cmd === "plugin:app|version") return "0.0.0";
      if (cmd === "folder_state") return { head: "h1", files: { "notes.txt": "1:1" }, committed: [], jumped: false };
      if (cmd === "hotkey_set" && args.combo === "Win+D") throw { Failed: "Windows would not give ByteHive Win+D: another app or another ByteHive window has it." };
      if (lists.has(cmd)) return [];
      return null;
    },
  };
  localStorage.setItem("maestro.session", JSON.stringify([{ name: "probe", dir: "D:/probe", agents: [
    { program: "claude", args: [], cwd: "D:/probe", name: "Ana", badge: "claude", color: "#d97757", mono: "A" },
    { program: "codex", args: [], cwd: "D:/probe", name: "Bo", badge: "codex", color: "#10a37f", mono: "B" },
  ] }]));
})();
`;

// Everything an app shortcut could do, as one comparable string.
const ACTIONS = `JSON.stringify({
  palette: !!document.querySelector(".inbox-pal"),
  modal: [...document.querySelectorAll(".inbox-modal-back, .backdrop.open")].map((e) => e.className),
  dock: document.querySelector(".dock-panel.open, [data-tool].open")?.dataset.tool ?? null,
  office: !!document.querySelector(".of:not([hidden])"),
  grid: document.body.classList.contains("ib-grid") || !!document.querySelector(".split-pin"),
  review: !!document.querySelector(".inbox-review"),
  stage: document.querySelector(".pane.focused")?.dataset.id ?? [...document.querySelectorAll(".pane")].findIndex((p) => p.classList.contains("focused")),
  code: document.getElementById("app")?.classList.contains("code-hidden"),
  zoom: document.querySelector(".pane .xterm-rows")?.style.fontSize ?? "",
  find: [...document.querySelectorAll("[data-find]")].some((e) => !e.hidden),
  terminal: !!document.activeElement?.closest(".xterm"),
})`;

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const profile = fs.mkdtempSync(path.join(os.tmpdir(), "ux-probe-"));
const chrome = spawn(CHROME, ["--headless=new", `--remote-debugging-port=${PORT}`, "--enable-unsafe-swiftshader", `--user-data-dir=${profile}`, "--hide-scrollbars", "--no-first-run", "--window-size=1440,900", "about:blank"], { stdio: "ignore" });

let ws = null;
let nextId = 0;
const pending = new Map();
function send(method, params = {}) {
  return new Promise((resolve, reject) => {
    const id = ++nextId;
    pending.set(id, { resolve, reject });
    ws.send(JSON.stringify({ id, method, params }));
  });
}

async function connect() {
  let target = null;
  for (let i = 0; i < 100 && !target; i += 1) {
    await sleep(200);
    try { target = (await (await fetch(`http://127.0.0.1:${PORT}/json`)).json()).find((t) => t.type === "page"); } catch { /* not up yet */ }
  }
  if (!target) throw new Error(`Chrome did not open port ${PORT}`);
  ws = new WebSocket(target.webSocketDebuggerUrl);
  await new Promise((r) => ws.addEventListener("open", r, { once: true }));
  ws.addEventListener("message", (e) => {
    const m = JSON.parse(e.data);
    const p = m.id && pending.get(m.id);
    if (!p) return;
    pending.delete(m.id);
    if (m.error) p.reject(new Error(m.error.message));
    else p.resolve(m.result);
  });
}

async function value(expression) {
  const r = await send("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true });
  if (r.exceptionDetails) throw new Error(r.exceptionDetails.exception?.description ?? r.exceptionDetails.text);
  return r.result.value;
}

async function until(expression, what, ms = 15000) {
  for (let t = 0; t < ms; t += 100) {
    if (await value(expression)) return;
    await sleep(100);
  }
  throw new Error(`timed out waiting for ${what}`);
}

async function press(mods, k) {
  const text = k.key.length === 1 && !(mods & (CTRL | ALT)) ? k.key : undefined;
  const ev = { modifiers: mods, key: k.key, code: k.code, windowsVirtualKeyCode: k.vk, nativeVirtualKeyCode: k.vk };
  await send("Input.dispatchKeyEvent", { type: text ? "keyDown" : "rawKeyDown", ...ev, text });
  await send("Input.dispatchKeyEvent", { type: "keyUp", ...ev });
}

async function click(x, y) {
  for (const type of ["mousePressed", "mouseReleased"]) await send("Input.dispatchMouseEvent", { type, x, y, button: "left", clickCount: 1 });
}

async function clickOn(selector) {
  const at = JSON.parse(await value(`(() => { const r = document.querySelector(${JSON.stringify(selector)}).getBoundingClientRect(); return JSON.stringify({ x: r.left + r.width / 2, y: r.top + r.height / 2 }); })()`));
  await click(at.x, at.y);
}

async function shot(name) {
  if (!shots) return;
  const { data } = await send("Page.captureScreenshot", { format: "png" });
  fs.writeFileSync(path.join(shots, `${name}.png`), Buffer.from(data, "base64"));
}

const focusTerminal = async () => {
  // Its left edge: an open tool stands over the right of it.
  const at = JSON.parse(await value(`(() => { const r = document.querySelector(".pane.focused .xterm-screen").getBoundingClientRect(); return JSON.stringify({ x: r.left + 30, y: r.top + 30 }); })()`));
  await click(at.x, at.y);
  await until(`!!document.activeElement?.closest(".xterm")`, "the terminal to take focus");
};

const failures = [];
const check = (ok, line) => { console.log(`${ok ? "ok  " : "FAIL"} ${line}`); if (!ok) failures.push(line); };

/** Press each combo over the terminal; nothing in the app may move. */
async function combos(label, mustGiveBytes) {
  const before = await value(ACTIONS);
  for (const [name, mods, k] of COMBOS) {
    await value("__probe.input.length = 0; __probe.keys = 0");
    await press(mods, k);
    await sleep(60);
    const sent = JSON.parse(await value("JSON.stringify(__probe.input)"));
    const after = await value(ACTIONS);
    const quiet = after === before;
    // Chrome keeps Ctrl+Tab for its own tabs and the page never sees it: only the real window can say.
    if (!(await value("__probe.keys"))) { console.log(`skip ${label} ${name}: this browser keeps it, the page saw no key`); continue; }
    const reached = sent.length > 0;
    check(quiet && (reached || !mustGiveBytes), `${label} ${name}: ${quiet ? "no app action" : `APP MOVED ${after}`}, bytes ${JSON.stringify(sent)}`);
    if (!quiet) { await focusTerminal(); }
  }
}

async function closeChrome() {
  try { if (ws) await Promise.race([send("Browser.close"), sleep(3000)]); } catch { /* it may be gone already */ }
  await sleep(500);
  if (chrome.exitCode === null) chrome.kill();
  await sleep(300);
  try { fs.rmSync(profile, { recursive: true, force: true }); } catch { /* a file still held open: the temp folder keeps it */ }
}

try {
  await connect();
  await send("Page.enable");
  await send("Runtime.enable");
  await send("Emulation.setDeviceMetricsOverride", { width: 1440, height: 900, deviceScaleFactor: 1, mobile: false });
  await send("Page.addScriptToEvaluateOnNewDocument", { source: FAKE });
  await send("Page.navigate", { url: base });
  await until(`!!document.querySelector("#btnResumeAll") && !document.querySelector("#btnResumeAll").hidden`, "the restored agents");
  await sleep(2400); // the splash takes clicks until it has gone
  await value(`document.querySelector("#btnResumeAll").click()`);
  await until(`__probe.channels.length >= 2 && !!document.querySelector(".pane.focused .xterm-screen")`, "the agents to start");
  await sleep(400);

  // The fake folder is dirty from the first look, so it is nobody's, and the project says so.
  await until(`!!document.querySelector(".iq-ph")?.textContent.includes("Uncommitted changes")`, "the project header to name the folder's changes");
  check(true, "the project header says the folder has uncommitted changes");

  await focusTerminal();
  await shot("terminal-focused");
  await send("Emulation.setDeviceMetricsOverride", { width: 900, height: 620, deviceScaleFactor: 1, mobile: false });
  await sleep(300);
  await shot("terminal-focused-900");
  await send("Emulation.setDeviceMetricsOverride", { width: 1440, height: 900, deviceScaleFactor: 1, mobile: false });
  await sleep(300);
  await combos("plain:", false);

  // ConPTY asks for win32-input-mode: from then on every key is a key event.
  await value(`__probe.write("\\x1b[?9001h")`);
  await sleep(200);
  await combos("win32:", true);
  await value(`__probe.write("\\x1b[?9001l")`);
  await sleep(100);

  // What a terminal keeps for itself.
  await press(CTRL | SHIFT, letter("F"));
  await sleep(120);
  check(await value(`[...document.querySelectorAll("[data-find]")].some((e) => !e.hidden)`), "Ctrl+Shift+F opens the find bar");
  await press(0, named("Escape", "Escape", 27));
  await sleep(120);

  // An open tool stays open when Esc goes to the terminal.
  await value(`document.querySelector("#doorBoard").click()`);
  await sleep(200);
  await focusTerminal();
  await value("__probe.input.length = 0");
  await press(0, named("Escape", "Escape", 27));
  await sleep(120);
  check(await value(`__probe.input.includes("\\x1b") && !!document.querySelector(".open[data-tool]")`), "Esc over a terminal reaches it and leaves the open dock alone");
  await value(`document.querySelector("#doorBoard").click()`);
  await sleep(200);

  // The gate isn't just dead: outside a terminal the same keys still work.
  await clickOn("#doorBoard"); // a real click, which also leaves the terminal
  await sleep(200);
  check(await value(`!document.activeElement?.closest(".xterm")`), "a click outside the terminal gives the keys back to the app");
  await shot("board-open");
  await press(CTRL | SHIFT, letter("K"));
  await sleep(200);
  await press(CTRL, letter("k"));
  await sleep(200);
  check(await value(`!!document.querySelector(".inbox-pal")`), "Ctrl+K opens the palette with focus outside");
  await shot("palette-open");
  await press(CTRL | SHIFT, letter("K"));
  await sleep(200);
  check(await value(`!document.querySelector(".open[data-tool]")`), "no app key acts behind the open palette");
  await press(0, named("Escape", "Escape", 27));
  await sleep(200);

  // The global key's row in Settings, and what it says when Windows refuses.
  await value(`[...document.querySelectorAll(".inbox-dock button")].slice(-1)[0].click()`);
  await sleep(300);
  await value(`document.querySelector('.sn[data-sec="terminal"]').click()`);
  await sleep(700);
  await value(`(() => { const i = document.getElementById("prefHotkey"); i.value = "Win+D"; i.dispatchEvent(new Event("change", { bubbles: true })); })()`);
  await sleep(300);
  check(await value(`document.getElementById("setHotkeyMsg").textContent.includes("Windows")`), "Settings says in words when Windows refuses the key");
  await shot("settings-hotkey");
  await value(`(() => { const i = document.getElementById("prefHotkey"); i.value = "Win+Alt+N"; i.dispatchEvent(new Event("change", { bubbles: true })); document.getElementById("setClose").click(); })()`);
  await sleep(300);

  // Each door does what it says, with real clicks.
  await clickOn("#doorJump");
  await sleep(200);
  check(await value(`!!document.querySelector(".inbox-pal")`), "Jump opens the palette");
  await press(0, named("Escape", "Escape", 27));
  await sleep(200);
  await clickOn("#doorOffice");
  await until(`!!document.querySelector(".of:not([hidden])")`, "the office");
  await sleep(600);
  await shot("office-bar");
  for (const [sel, name, open] of [
    ["[data-jump]", "Jump", `!!document.querySelector(".inbox-pal")`],
    ["[data-new-agent]", "New agent", `!!document.querySelector(".inbox-modal-back:not(.pal-back)")`],
  ]) {
    await clickOn(`.of ${sel}`);
    await sleep(300);
    check(await value(open), `the office's ${name} opens over it`);
    await shot(`office-${name.replace(" ", "-").toLowerCase()}`);
    await press(0, named("Escape", "Escape", 27));
    await sleep(250);
  }
  await clickOn(".of [data-board-door]");
  await sleep(300);
  check(await value(`!!document.querySelector(".open[data-tool]")`), "the office's Board opens over it");
  await shot("office-board");
} catch (e) {
  failures.push(String(e));
  console.error(e);
} finally {
  await closeChrome();
}

console.log(failures.length ? `FAILED: ${failures.length} check(s)` : "All checks passed.");
process.exit(failures.length ? 1 : 0);
