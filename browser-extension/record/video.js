/* A video recording of an agent's tab (browser_record): the screencast feeds
 * the offscreen recorder, and alongside it a timeline of what the agent did
 * and what broke, with keyframes before and after each action. Stop leaves a
 * set of files the MCP side pulls in parts (video, timeline.md, frames/). */

import { addEvent, describe, consoleLine, networkLine, toMarkdown, counts, clock } from "./timeline.js";
import { ensureOffscreen, closeOffscreen, toOffscreen, startCast, stopCast } from "./screencast.js";

const MAX_MS = 10 * 60_000; // a recording stops by itself after this
const KEEP_MS = 10 * 60_000; // a finished one nobody fetched is dropped
const MAX_KEYS = 60;
const AFTER_MS = 700;
const IN_FLIGHT = 8; // pictures on their way to the recorder at once
const PULL_MS = 1000; // a frame is asked for after this long without one
const SETTLE_MS = 1500; // after the debugger attaches, before the view is measured
export const PART = 4 * 1024 * 1024;

const live = new Map(); // agent -> recording
const done = new Map(); // agent -> { id, files: Map(name -> { size, video?, bytes? }), result, claimed, drop }
let deps; // { cdp, dbg, cursor } from background.js

export function initVideo(d) { deps = d; }
export const isRecording = (agent) => live.has(agent);

export async function startVideo(agent, tab) {
  await stopVideo(agent, { discard: true }).catch(() => {});
  dropDone(agent);
  await deps.dbg(tab.id);
  await ensureOffscreen();
  const id = `${agent}-${Date.now()}`;
  const size = await viewSize(tab.id);
  const started = Date.now();
  await toOffscreen("start", { id, started, ...size });
  const rec = { id, agent, tabId: tab.id, started, events: [], keys: [], latest: null, frames: 0, reqs: new Map() };
  rec.auto = setTimeout(() => stopVideo(agent, { auto: true }).catch(() => {}), MAX_MS);
  rec.lastFrame = 0;
  rec.pull = setInterval(() => pullFrame(rec), PULL_MS);
  live.set(agent, rec);
  try {
    await startCast(deps.cdp, tab.id);
  } catch (e) {
    live.delete(agent);
    clearTimeout(rec.auto);
    clearInterval(rec.pull);
    await toOffscreen("drop", { id }).catch(() => {});
    throw e;
  }
  // The cursor turns plain: just an arrow, the way a person's mouse looks.
  void deps.cursor(tab.id, agent, "show");
}

/** The video's size: the tab's view in device pixels, as the screencast sends
 *  it (at most 1920x1080). Taken from the tab, not the first frame, which can
 *  come from a window still settling. Empty when the page can't be asked. */
async function viewSize(tabId) {
  // Chrome shows its "started debugging" bar a moment after the debugger
  // attaches, and the view gets shorter: measure after it.
  const wait = deps.attachedAt(tabId) + SETTLE_MS - Date.now();
  if (wait > 0) await new Promise((r) => setTimeout(r, wait));
  try {
    const r = await deps.cdp(tabId, "Runtime.evaluate", { expression: "[innerWidth, innerHeight, devicePixelRatio]", returnByValue: true });
    const [w, h, d] = r.result.value;
    const s = Math.min(1, 1920 / (w * d), 1080 / (h * d));
    return { width: Math.round(w * d * s), height: Math.round(h * d * s) };
  } catch {
    return {};
  }
}

/** A frame to the encoder. Chrome sends the next screencast frame only
 *  after this one is acked: ack at once while fewer than IN_FLIGHT frames
 *  are being encoded, else when one is done. One at a time gave 16 fps;
 *  a few at once keeps capture and encoding busy together, and Chrome still
 *  slows down instead of piling frames up. */
function takeFrame(rec, data, at = Date.now(), sessionId = null) {
  rec.latest = data;
  rec.lastFrame = Date.now();
  if (++rec.frames === 1) addKey(rec, "start");
  const ack = () => deps.cdp(rec.tabId, "Page.screencastFrameAck", { sessionId }).catch(() => {});
  rec.inFlight = (rec.inFlight ?? 0) + 1;
  if (sessionId != null) {
    if (rec.inFlight < IN_FLIGHT) ack();
    else rec.owed = ack;
  }
  const done = () => {
    rec.inFlight--;
    const owed = rec.owed;
    rec.owed = null;
    owed?.();
  };
  toOffscreen("frame", { id: rec.id, data, at }).then(done, done);
}

/** Chrome paints a covered window only while something animates, so a still
 *  page (or one changed by script) can send no screencast frame at all. After
 *  a quiet second, ask for a picture: a screenshot makes Chrome paint. */
async function pullFrame(rec, force = false) {
  if (!force && (rec.pulling || Date.now() - rec.lastFrame < PULL_MS)) return;
  rec.pulling = true;
  try {
    const shot = await deps.cdp(rec.tabId, "Page.captureScreenshot", { format: "jpeg", quality: 80, optimizeForSpeed: true });
    if ((force || live.get(rec.agent) === rec) && shot?.data && Date.now() - rec.lastFrame >= PULL_MS) takeFrame(rec, shot.data);
  } catch {} finally { rec.pulling = false; }
}

/** CDP events of every tab: frames and trouble for the one being recorded. */
export function onCdpEvent(tabId, method, p) {
  let rec = null;
  for (const r of live.values()) if (r.tabId === tabId) rec = r;
  if (!rec) return;
  const at = Date.now() - rec.started;
  if (method === "Page.screencastFrame") {
    // When Chrome painted it: the worker gets frames in bursts, so its own
    // clock would bunch them up. Chrome's clock is not quite ours, so its
    // times are moved onto ours by the gap seen on the first frame, and never
    // later than now.
    const painted = (p.metadata?.timestamp ?? 0) * 1000;
    const now = Date.now();
    rec.skew ??= now - painted;
    const at = Math.min(now, painted + rec.skew);
    takeFrame(rec, p.data, Math.abs(at - now) < 2000 ? at : now, p.sessionId);
  } else if (method === "Page.loadEventFired") {
    // A new document: make sure the screencast carries on with it.
    startCast(deps.cdp, tabId).catch(() => {});
  } else if (method === "Runtime.consoleAPICalled") {
    addEvent(rec.events, at, "error", consoleLine(p.type, p.args.map((a) => a.value ?? a.description ?? "").join(" ")));
  } else if (method === "Runtime.exceptionThrown") {
    addEvent(rec.events, at, "error", consoleLine("exception", p.exceptionDetails.exception?.description ?? p.exceptionDetails.text));
  } else if (method === "Network.requestWillBeSent") {
    if (rec.reqs.size > 2000) rec.reqs.clear();
    rec.reqs.set(p.requestId, { method: p.request.method, url: p.request.url });
  } else if (method === "Network.responseReceived") {
    const q = rec.reqs.get(p.requestId);
    addEvent(rec.events, at, "net", networkLine(q?.method ?? "GET", p.response.url, p.response.status));
  } else if (method === "Network.loadingFailed") {
    const q = rec.reqs.get(p.requestId);
    if (q) addEvent(rec.events, at, "net", networkLine(q.method, q.url, 0, p.errorText));
  }
}

export function onTabClosed(tabId) {
  for (const [agent, r] of live) if (r.tabId === tabId) stopVideo(agent, { auto: true }).catch(() => {});
}

/** Before an agent's call: a timeline line and the frame just before it. */
export function noteBefore(agent, tool, args) {
  const rec = live.get(agent);
  if (!rec) return null;
  const e = addEvent(rec.events, Date.now() - rec.started, "action", describe(tool, args));
  if (e) e.frames = addKey(rec, "before");
  return e;
}

/** After it: whether it failed, and the frame once the page has reacted. */
export function noteAfter(agent, e, failed) {
  const rec = live.get(agent);
  if (!e || !rec) return;
  if (failed) e.text += ` (failed: ${String(failed).slice(0, 120)})`;
  setTimeout(() => {
    if (live.get(agent) !== rec) return;
    const k = addKey(rec, "after");
    if (k && k !== e.frames) e.frames = e.frames ? `${e.frames} ${k}` : k;
  }, AFTER_MS);
}

function addKey(rec, label) {
  if (!rec.latest) return "";
  const last = rec.keys[rec.keys.length - 1];
  if (last?.data === rec.latest) return last.num;
  if (rec.keys.length >= MAX_KEYS) return "";
  const num = String(rec.keys.length + 1).padStart(3, "0");
  rec.keys.push({ num, name: `frames/${num}-${clock(Date.now() - rec.started).replace(":", "m")}s-${label}.jpg`, data: rec.latest });
  return num;
}

export async function stopVideo(agent, { discard = false, auto = false } = {}) {
  const rec = live.get(agent);
  if (!rec) {
    const d = done.get(agent);
    if (d && !d.claimed && !discard) { d.claimed = true; return d.result; }
    throw new Error("Nothing is being recorded. Start with action start.");
  }
  live.delete(agent);
  clearTimeout(rec.auto);
  clearInterval(rec.pull);
  await stopCast(deps.cdp, rec.tabId).catch(() => {});
  void deps.cursor(rec.tabId, agent, "show");
  if (discard) { await toOffscreen("drop", { id: rec.id }).catch(() => {}); return null; }
  // A page that never painted still gets its one picture.
  if (!rec.frames) await pullFrame(rec, true);
  addKey(rec, "end");
  const v = await toOffscreen("stop", { id: rec.id, at: Date.now() });
  if (v?.error && !v.size) {
    await toOffscreen("drop", { id: rec.id }).catch(() => {});
    throw new Error(`The video could not be made: ${v.error}`);
  }
  if (!v?.size) {
    await toOffscreen("drop", { id: rec.id }).catch(() => {});
    throw new Error(rec.frames ? `The tab sent ${rec.frames} frame(s) but the video came out empty.` : "No frames came from the tab, so there is no video.");
  }
  const ms = Date.now() - rec.started;
  const { actions, errors } = counts(rec.events);
  const md = toMarkdown(`${agent}: ${clock(ms)} of video, ${v.width}x${v.height}, ${v.ticks} frames at 30 fps, ${v.real} of them new pictures, ${new Date(rec.started).toISOString()}`, rec.events) +
    (rec.keys.length ? "\nNumbers in [brackets] are frames/NNN-*.jpg.\n" : "");
  const files = new Map([["video.mp4", { size: v.size, video: true }]]);
  const put = (name, bytes) => files.set(name, { size: bytes.length, bytes });
  put("timeline.md", new TextEncoder().encode(md));
  for (const k of rec.keys) put(k.name, Uint8Array.from(atob(k.data), (c) => c.charCodeAt(0)));
  const mb = (v.size / 1048576).toFixed(1);
  const result = {
    content: [{ type: "text", text: `Recorded ${clock(ms)} (${mb} MB, ${v.width}x${v.height}${auto ? ", stopped by itself" : ""}): ${actions} actions, ${errors} errors, ${rec.keys.length} frames.` }],
    record: { files: [...files].map(([name, f]) => ({ name, size: f.size })) },
  };
  done.set(agent, { id: rec.id, files, result, claimed: !auto, drop: setTimeout(() => dropDone(agent), KEEP_MS) });
  return result;
}

/** One part of a finished recording's file, base64. */
export async function readPart(agent, name, offset = 0) {
  const d = done.get(agent);
  const f = d?.files.get(name);
  if (!f) throw new Error(`No recorded file ${name}.`);
  if (f.video) {
    const r = await toOffscreen("part", { id: d.id, offset, size: PART });
    if (!r || r.error) throw new Error(r?.error ?? "The recorder is gone.");
    return r.data;
  }
  const b = f.bytes.subarray(offset, offset + PART);
  let bin = "";
  for (let i = 0; i < b.length; i += 0x8000) bin += String.fromCharCode(...b.subarray(i, i + 0x8000));
  return btoa(bin);
}

export function dropDone(agent) {
  const d = done.get(agent);
  if (!d) return;
  clearTimeout(d.drop);
  done.delete(agent);
  toOffscreen("drop", { id: d.id })
    .then((r) => { if (r?.left === 0 && !live.size && !done.size) return closeOffscreen(); })
    .catch(() => {});
}
