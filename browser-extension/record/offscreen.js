/* The recorder: an offscreen document, because a service worker has no
 * VideoEncoder. The service worker sends it the tab's pictures (JPEG) with
 * the time Chrome painted each; they are encoded (WebCodecs H.264, VP9 when
 * this Chrome has no H.264 encoder) and muxed into an MP4.
 *
 * The video is a steady 30 frames a second. Taking a picture only decodes it
 * and queues it, so Chrome is never kept waiting; a separate loop encodes the
 * ticks of the clock (1/30 s each) a little behind real time, each with the
 * newest picture painted by then. A page that paints less often, or not at
 * all while still, repeats its picture, which costs almost nothing to store.
 *
 * Measured, so not done again: a canvas stream with MediaRecorder in this
 * hidden document gave 3 fps or empty files (Chrome takes a canvas frame only
 * when it renders the page); encoding inside the picture's own call made
 * Chrome wait for the encoder and sent 2 pictures a second. */

import { Muxer, ArrayBufferTarget } from "../vendor/mp4-muxer.mjs";

const CODECS = [
  { codec: "avc1.640028", mux: "avc" }, // H.264 High 4.0: up to 1920x1080
  { codec: "avc1.4d0028", mux: "avc" },
  { codec: "vp09.00.40.08", mux: "vp9" },
];
/** Which encoder to ask for first. Measured on a busy machine: the GPU one
 *  (Windows Media Foundation) fell 9 s behind a 30 fps clock, the software
 *  one stayed within half a second. */
const ENCODER = "prefer-software";
const BITRATE = 2_500_000;
const FPS = 30;
const KEY_EVERY = 2 * FPS; // a key frame every 2 s
const QUEUE = 4; // frames waiting in the encoder before the loop waits for it
const LAG_US = 250_000; // ticks are encoded this far behind real time
const MAX_WAITING = 12; // pictures queued for the loop; more and the middle ones go
const RUN_MS = 100; // how often the loop wakes up on its own
const at30 = (tick) => Math.round((tick * 1_000_000) / FPS);

const recs = new Map(); // id -> recording

chrome.runtime.onMessage.addListener((m, _sender, reply) => {
  if (m?.target !== "bytehive-offscreen") return;
  ops[m.op]?.(m).then(reply, (e) => reply({ error: String(e?.message ?? e) }));
  return true;
});

const ops = {
  async start(m) {
    await ops.drop(m);
    const r = { id: m.id, t0: m.started, want: m.width > 0 && m.height > 0 ? [m.width, m.height] : null, waiting: [], tick: 0, real: 0 };
    r.timer = setInterval(() => kick(r), RUN_MS);
    recs.set(m.id, r);
    return { ok: true };
  },
  /** A picture: decoded and queued, answered at once. */
  async frame(m) {
    const r = recs.get(m.id);
    if (!r || r.stopped) return false;
    const bmp = await createImageBitmap(await (await fetch(`data:image/jpeg;base64,${m.data}`)).blob());
    if (r.stopped || !recs.has(r.id)) { bmp.close(); return false; }
    r.ready ??= begin(r, ...(r.want ?? [bmp.width, bmp.height]));
    await r.ready;
    queue(r, { at: (m.at - r.t0) * 1000, bmp });
    kick(r);
    return true;
  },
  async stop(m) {
    const r = recs.get(m.id);
    if (!r) throw new Error("No such recording.");
    r.stopped = true;
    clearInterval(r.timer);
    if (!r.enc) {
      await ops.drop(m);
      return { size: 0, error: r.error };
    }
    // Every tick up to the moment stop was asked, with the last picture.
    r.until = Math.max(at30(r.tick) + 1, (m.at - r.t0) * 1000);
    await r.loop;
    await run(r);
    await r.enc.flush().catch((e) => (r.error ??= String(e?.message ?? e)));
    if (r.error || !r.chunks) {
      const why = r.error ?? "the encoder gave nothing";
      await ops.drop(m);
      return { size: 0, error: why };
    }
    r.enc.close();
    r.muxer.finalize();
    closeAll(r);
    r.blob = new Blob([r.target.buffer], { type: "video/mp4" });
    r.enc = r.muxer = r.target = r.canvas = r.g = null;
    return { size: r.blob.size, mime: "video/mp4", width: r.width, height: r.height, codec: r.codec, real: r.real, ticks: r.tick };
  },
  async part(m) {
    const blob = recs.get(m.id)?.blob;
    if (!blob) throw new Error("The video is gone.");
    const bytes = new Uint8Array(await blob.slice(m.offset, m.offset + m.size).arrayBuffer());
    return { data: base64(bytes) };
  },
  async drop(m) {
    const r = recs.get(m.id);
    if (r) {
      r.stopped = true;
      clearInterval(r.timer);
      try { if (r.enc?.state === "configured") r.enc.close(); } catch {}
      closeAll(r);
      recs.delete(m.id);
    }
    return { left: recs.size };
  },
};

/** In paint order, one per tick: only the newest picture painted by a tick
 *  is shown in it, so an older one of the same tick goes at once. A loop
 *  that is still behind keeps the oldest (needed next) and the newest, and
 *  lets the ones between go. */
function queue(r, p) {
  let i = r.waiting.length;
  while (i > 0 && r.waiting[i - 1].at > p.at) i--;
  const slot = (at) => Math.ceil((at * FPS) / 1_000_000);
  const before = r.waiting[i - 1];
  if (before && slot(before.at) === slot(p.at)) { before.bmp.close(); r.waiting[i - 1] = p; }
  else r.waiting.splice(i, 0, p);
  while (r.waiting.length > MAX_WAITING) r.waiting.splice(1, 1)[0].bmp.close();
}

function kick(r) {
  if (!r.running && r.enc && !r.error && !r.stopped) r.loop = run(r);
}

/** Encode the ticks that are due: up to LAG behind now, or to the end once
 *  stopped. */
async function run(r) {
  r.running = true;
  try {
    for (;;) {
      const horizon = r.until ?? (Date.now() - r.t0) * 1000 - LAG_US;
      const t = at30(r.tick);
      if (t >= horizon || r.error) break;
      // The newest picture painted by this tick (the very first one shows
      // from the start).
      while (r.waiting.length && (r.waiting[0].at <= t || !r.shown)) {
        r.shown?.close();
        r.shown = r.waiting.shift().bmp;
        r.fresh = true;
        r.real++;
      }
      if (!r.shown) break;
      if (r.enc.encodeQueueSize > QUEUE) await room(r);
      tick(r, t);
      // Let the next picture in: a loop that never yields keeps Chrome
      // waiting on its answer, and the tab sends fewer pictures.
      await new Promise((ok) => setTimeout(ok, 0));
    }
  } finally {
    r.running = false;
  }
}

function room(r) {
  return new Promise((ok) => {
    const t = setTimeout(ok, 1000);
    r.enc.addEventListener("dequeue", () => { clearTimeout(t); ok(); }, { once: true });
  });
}

function tick(r, t) {
  if (r.fresh) {
    r.base?.close();
    r.base = baseFrame(r, r.shown);
    r.fresh = false;
  }
  // A copy of the picture's frame with this tick's time: no pixels move.
  const vf = new VideoFrame(r.base, { timestamp: t, duration: at30(r.tick + 1) - t });
  r.enc.encode(vf, { keyFrame: r.tick % KEY_EVERY === 0 });
  vf.close();
  r.tick++;
}

/** The picture as a frame of the video's size: straight from the bitmap when
 *  it is that size (or a pixel more, cut for even sides), else drawn fitted
 *  on the canvas. Reading a canvas back for every tick is what kept the
 *  recorder at 13 pictures a second. */
function baseFrame(r, bmp) {
  const { width: W, height: H } = r;
  if (bmp.width - W >= 0 && bmp.width - W <= 1 && bmp.height - H >= 0 && bmp.height - H <= 1)
    return new VideoFrame(bmp, { timestamp: 0, visibleRect: { x: 0, y: 0, width: W, height: H } });
  paint(r, bmp);
  return new VideoFrame(r.canvas, { timestamp: 0 });
}

function paint(r, bmp) {
  const { width: W, height: H } = r;
  // A picture of another size (the window was resized) is fitted, not
  // stretched. Nearly the same shape (a pixel cut for even sides) fills it.
  const fx = W / bmp.width, fy = H / bmp.height;
  const s = Math.min(fx, fy);
  const full = Math.abs(fx - fy) < 0.01;
  const w = full ? W : Math.round(bmp.width * s), h = full ? H : Math.round(bmp.height * s);
  if (w !== W || h !== H) { r.g.fillStyle = "#000"; r.g.fillRect(0, 0, W, H); }
  r.g.drawImage(bmp, (W - w) >> 1, (H - h) >> 1, w, h);
}

function closeAll(r) {
  r.base?.close();
  r.base = null;
  r.shown?.close();
  r.shown = null;
  for (const p of r.waiting) p.bmp.close();
  r.waiting = [];
}

async function begin(r, w, h) {
  // H.264 wants even sides.
  r.width = w - (w % 2);
  r.height = h - (h % 2);
  r.canvas = new OffscreenCanvas(r.width, r.height);
  r.g = r.canvas.getContext("2d", { alpha: false });
  let pick = null;
  for (const c of CODECS.flatMap((c) => [{ ...c, hw: ENCODER }, { ...c, hw: "no-preference" }])) {
    const config = { codec: c.codec, hardwareAcceleration: c.hw, width: r.width, height: r.height, bitrate: BITRATE, bitrateMode: "variable", framerate: FPS, latencyMode: "realtime", ...(c.mux === "avc" ? { avc: { format: "avc" } } : {}) };
    try { if ((await VideoEncoder.isConfigSupported(config)).supported) { pick = { ...c, config }; break; } } catch {}
  }
  if (!pick) throw new Error("This Chrome cannot encode video.");
  r.codec = pick.codec;
  r.target = new ArrayBufferTarget();
  r.muxer = new Muxer({ target: r.target, video: { codec: pick.mux, width: r.width, height: r.height }, fastStart: "in-memory", firstTimestampBehavior: "offset" });
  r.enc = new VideoEncoder({
    output: (chunk, meta) => {
      try { r.muxer.addVideoChunk(chunk, meta); r.chunks = (r.chunks ?? 0) + 1; } catch (e) { r.error ??= `mux: ${e?.message ?? e}`; }
    },
    error: (e) => (r.error ??= String(e?.message ?? e)),
  });
  r.enc.configure(pick.config);
}

function base64(bytes) {
  let bin = "";
  for (let i = 0; i < bytes.length; i += 0x8000) bin += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return btoa(bin);
}
