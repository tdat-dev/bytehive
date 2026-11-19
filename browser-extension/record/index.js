/* Recording an agent's tab: the hub tools "record" (browser_record) and
 * "gif" (the older browser_gif, kept for an older bytehive-mcp).
 *
 * record {action: "start", format: "video" | "gif", tabId}
 *        {action: "stop"}  -> video: { content, record: { files: [{name, size}] } }
 *                             gif:   { content, gif }
 *        {action: "part", name, offset} -> { content: [], part } (base64, 4 MB at most)
 *        {action: "done"}  frees a finished recording */

import { startGif, finishGif, gifLive, stopGif } from "./gif.js";
import { initVideo, isRecording, startVideo, stopVideo, readPart, dropDone, onCdpEvent, onTabClosed, noteBefore, noteAfter } from "./video.js";

export { isRecording, onCdpEvent, onTabClosed, noteBefore, noteAfter };

let deps; // { cdp, dbg, cursor, grab, awake, tabFor } from background.js

export function initRecord(d) {
  deps = d;
  initVideo(d);
}

const text = (s) => ({ content: [{ type: "text", text: s }] });

async function gifResult(agent) {
  const g = await finishGif(agent);
  const r = text(`Recorded ${g.frames} frames over ${g.secs}s.`);
  r.gif = g.gif;
  return r;
}

export async function recordTool(agent, a) {
  switch (a.action) {
    case "start": {
      const t = await deps.tabFor(agent, a.tabId);
      if (a.format === "gif") {
        await stopVideo(agent, { discard: true }).catch(() => {});
        startGif(agent, t.id, deps);
        return text("Recording a GIF. Do the steps, then stop. Frames are taken while the tab is showing.");
      }
      stopGif(agent);
      await startVideo(agent, t);
      return text("Recording. Do the steps, then stop.");
    }
    case "stop":
      return gifLive(agent) ? gifResult(agent) : stopVideo(agent);
    case "part":
      return { content: [], part: await readPart(agent, String(a.name ?? ""), Math.max(0, Number(a.offset) || 0)) };
    case "done":
      dropDone(agent);
      return text("Done.");
    default:
      throw new Error(`Unknown record action "${a.action}".`);
  }
}

/** The older hub tool "gif": start/stop a GIF only. */
export async function gifTool(agent, a) {
  if (a.action === "start") {
    const t = await deps.tabFor(agent, a.tabId);
    startGif(agent, t.id, deps);
    return text("Recording. Do the steps, then call browser_gif with action stop. Frames are taken while the tab is showing.");
  }
  return gifResult(agent);
}
