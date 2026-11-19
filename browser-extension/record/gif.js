/* GIF recording (browser_record format "gif", and the older browser_gif):
 * frames copied from the agent's tab twice a second, only while it records,
 * only while the tab is showing (the copy is free then). Moved here unchanged
 * from background.js. */

import { GIFEncoder, quantize, applyPalette } from "../vendor/gifenc.esm.js";

const recordings = new Map(); // agent -> { tabId, frames: [{ data, at, until }], timer, busy }
const GIF_FPS_MS = 500;
const GIF_MAX_FRAMES = 240; // two minutes
const GIF_WIDTH = 640;

/** deps: { grab(tab, maxW, quality), awake(tabId) } from background.js. */
export function startGif(agent, tabId, deps) {
  stopGif(agent);
  const rec = { tabId, frames: [], busy: false, started: Date.now() };
  rec.timer = setInterval(async () => {
    if (rec.busy) return;
    if (rec.frames.length >= GIF_MAX_FRAMES) return stopGif(agent, true);
    rec.busy = true;
    try {
      const tab = await chrome.tabs.get(rec.tabId);
      if (tab.active) {
        await deps.awake(tab.id);
        const img = await deps.grab(tab, GIF_WIDTH, 70);
        const last = rec.frames[rec.frames.length - 1];
        // A still page adds time to the last frame instead of another frame.
        if (last && last.data === img.data) last.until = Date.now();
        else rec.frames.push({ data: img.data, at: Date.now(), until: Date.now() });
      }
    } catch {} finally { rec.busy = false; }
  }, GIF_FPS_MS);
  recordings.set(agent, rec);
  return rec;
}

export const gifLive = (agent) => recordings.has(agent);

export function stopGif(agent, keep = false) {
  const rec = recordings.get(agent);
  if (!rec) return null;
  clearInterval(rec.timer);
  if (!keep) recordings.delete(agent);
  return rec;
}

/** Stop and encode: { gif (base64), frames, secs }. */
export async function finishGif(agent) {
  const rec = stopGif(agent);
  if (!rec) throw new Error("Nothing is being recorded. Start with action start.");
  if (!rec.frames.length) throw new Error("No frames were recorded: the tab was not showing.");
  return { gif: await encodeGif(rec.frames), frames: rec.frames.length, secs: Math.round((Date.now() - rec.started) / 1000) };
}

/** Encode the frames as an animated GIF (base64), one frame at a time so
 *  memory stays small. */
async function encodeGif(frames) {
  const gif = GIFEncoder();
  for (let i = 0; i < frames.length; i++) {
    const f = frames[i];
    const bmp = await createImageBitmap(await (await fetch(`data:image/jpeg;base64,${f.data}`)).blob());
    const c = new OffscreenCanvas(bmp.width, bmp.height);
    const g = c.getContext("2d", { willReadFrequently: true });
    g.drawImage(bmp, 0, 0);
    bmp.close();
    const { data, width, height } = g.getImageData(0, 0, c.width, c.height);
    const palette = quantize(data, 256);
    const index = applyPalette(data, palette);
    const next = frames[i + 1]?.at ?? f.until + GIF_FPS_MS;
    gif.writeFrame(index, width, height, { palette, delay: Math.max(GIF_FPS_MS, next - f.at) });
  }
  gif.finish();
  const bytes = gif.bytes();
  let bin = "";
  for (let i = 0; i < bytes.length; i += 0x8000) bin += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return btoa(bin);
}
