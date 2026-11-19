/* The two ends of a video recording that talk to Chrome: the tab's CDP
 * screencast (frames each time the page paints, covered tab or not) and the
 * offscreen document that encodes them into an MP4 (offscreen.js). */

const OFFSCREEN = "record/offscreen.html";
let opening = null;

export async function ensureOffscreen() {
  if (await chrome.offscreen.hasDocument()) return;
  // Two recordings starting at once must not both create it.
  opening ??= chrome.offscreen
    .createDocument({ url: OFFSCREEN, reasons: ["BLOBS"], justification: "Encode an agent's tab recording into a video file." })
    .catch((e) => { if (!/single offscreen/i.test(String(e?.message ?? e))) throw e; })
    .finally(() => (opening = null));
  await opening;
}

export async function closeOffscreen() {
  try { if (await chrome.offscreen.hasDocument()) await chrome.offscreen.closeDocument(); } catch {}
}

export async function toOffscreen(op, extra = {}) {
  return chrome.runtime.sendMessage({ target: "bytehive-offscreen", op, ...extra });
}

/** JPEG frames of what the tab paints, at most full HD. Chrome sends the next
 *  one only after the last is acked, so a slow machine gets fewer frames, not
 *  a backlog. */
export function startCast(cdp, tabId) {
  return cdp(tabId, "Page.startScreencast", { format: "jpeg", quality: 80, maxWidth: 1920, maxHeight: 1080, everyNthFrame: 1 });
}

export function stopCast(cdp, tabId) {
  return cdp(tabId, "Page.stopScreencast");
}
