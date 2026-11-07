// Screenshots of the office demo, from a throwaway headless Chrome. It starts no server:
//   npx vite --port 17631 --strictPort
//   node tools/office_shots.mjs http://localhost:17631/office-demo.html <out folder> [only]
// `only` keeps the shots whose name or width holds that text. Prints each file and its scene stats.
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const CHROME = "C:/Program Files/Google/Chrome/Application/chrome.exe";
const PORT = 9555;
const SIZES = [[1280, 800], [1920, 1080]];
const SHOTS = [
  ["building", "level=building"],
  ["floor-bytehive", "level=floor:bytehive"],
  ["desk-linh", "level=desk:linh"],
  ["desk-hai", "level=desk:hai"],
  ["desk-founder", "level=desk:founder"],
  ["quiet", "level=building&quiet=1"],
  ["empty", "level=building&empty=1"],
  ["floors-8", "level=building&floors=8"],
  ["flight-landed", "level=building&flight=1"],
  ["flight-air", "level=building&flight=1&air=0.5"],
];

const [base, out, only] = process.argv.slice(2);
if (!base || !out) {
  console.error("usage: node tools/office_shots.mjs <demo url> <out folder> [only]");
  process.exit(2);
}
fs.mkdirSync(out, { recursive: true });

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const profile = fs.mkdtempSync(path.join(os.tmpdir(), "office-shots-"));
const chrome = spawn(CHROME, ["--headless=new", `--remote-debugging-port=${PORT}`, "--enable-unsafe-swiftshader", `--user-data-dir=${profile}`, "--hide-scrollbars", "--no-first-run", "about:blank"], { stdio: "ignore" });

let ws = null;
async function closeChrome() {
  try {
    if (ws) await Promise.race([send("Browser.close"), sleep(3000)]);
  } catch { /* it may be gone already */ }
  await sleep(500);
  if (chrome.exitCode === null) chrome.kill();
  await sleep(300);
  try { fs.rmSync(profile, { recursive: true, force: true }); } catch { /* a file still held open: the temp folder keeps it */ }
}

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
  return r.result.value;
}

async function waitForFrames() {
  for (let i = 0; i < 300; i += 1) {
    const frames = await value("window.__office ? window.__office.stats().frames : 0");
    if (frames > 0) return true;
    await sleep(200);
  }
  return false;
}

async function shoot(name, query, w, h) {
  await send("Emulation.setDeviceMetricsOverride", { width: w, height: h, deviceScaleFactor: 1, mobile: false });
  // `air` holds a plane halfway, which reduced motion would put at its destination.
  await send("Page.navigate", { url: `${base}?${query}&software=1${query.includes("air=") ? "" : "&still=1"}` });
  await sleep(400);
  if (!(await waitForFrames())) {
    const text = await value("document.body.innerText.slice(0, 300)");
    console.error(`${name}: no frame drawn. Page says: ${text}`);
    return;
  }
  await sleep(300);
  const { data } = await send("Page.captureScreenshot", { format: "png" });
  const file = path.join(out, `${name}-${w}x${h}.png`);
  fs.writeFileSync(file, Buffer.from(data, "base64"));
  console.log(file, await value("JSON.stringify(window.__office.stats())"));
}

try {
  await connect();
  await send("Page.enable");
  await send("Runtime.enable");
  for (const [w, h] of SIZES) {
    for (const [name, query] of SHOTS) if (!only || name.includes(only) || `${w}`.includes(only)) await shoot(name, query, w, h);
  }
} catch (e) {
  console.error(String(e));
  process.exitCode = 1;
} finally {
  await closeChrome();
}
