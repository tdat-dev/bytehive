/* The recorder: an offscreen document, because a service worker has no
 * VideoEncoder. The service worker sends it the tab's frames (JPEG) with the
 * time each one came; each is encoded at that time (WebCodecs H.264, VP9 when
 * this Chrome has no H.264 encoder) and muxed into an MP4.
 *
 * Not a canvas stream with MediaRecorder: in a hidden document Chrome takes a
 * canvas frame only when it renders the page, which is seldom, so the video
 * came out at 3 fps or empty. Here every frame is encoded and stop flushes
 * them all. A still page needs no repeated frames: an MP4 frame lasts until
 * the next one. */

import { Muxer, ArrayBufferTarget } from "../vendor/mp4-muxer.mjs";

const CODECS = [
  { codec: "avc1.640028", mux: "avc" }, // H.264 High 4.0: up to 1920x1080
  { codec: "avc1.4d0028", mux: "avc" },
  { codec: "vp09.00.40.08", mux: "vp9" },
];
const BITRATE = 2_500_000;
const KEY_EVERY_US = 2_000_000;
const MAX_QUEUE = 8; // frames waiting for the encoder; more are dropped

const recs = new Map(); // id -> recording

chrome.runtime.onMessage.addListener((m, _sender, reply) => {
  if (m?.target !== "bytehive-offscreen") return;
  ops[m.op]?.(m).then(reply, (e) => reply({ error: String(e?.message ?? e) }));
  return true;
});

const ops = {
  /** Answered once the frame is encoded: the worker acks the screencast only
   *  then, so Chrome sends frames as fast as they are encoded and no faster. */
  async frame(m) {
    const r = recs.get(m.id);
    if (!r) return false;
    r.chain = r.chain.then(() => frame(r, m.data, m.at)).catch((e) => (r.error ??= String(e?.message ?? e)));
    await r.chain;
    return true;
  },
  async start(m) {
    await ops.drop(m);
    recs.set(m.id, { id: m.id, chain: Promise.resolve(), t0: m.started, want: m.width > 0 && m.height > 0 ? [m.width, m.height] : null });
    return { ok: true };
  },
  async stop(m) {
    const r = recs.get(m.id);
    if (!r) throw new Error("No such recording.");
    r.stopped = true; // a frame still on its way is not encoded
    await r.chain;
    if (!r.enc) {
      recs.delete(m.id);
      return { size: 0, error: r.error };
    }
    // The last picture lasts until the moment stop was asked.
    if (r.shown && !r.error) encode(r, r.shown, m.at, true);
    await r.enc.flush().catch((e) => (r.error ??= String(e?.message ?? e)));
    if (r.error || !r.chunks) {
      const why = r.error ?? "the encoder gave nothing";
      await ops.drop(m);
      return { size: 0, error: why };
    }
    r.enc.close();
    r.muxer.finalize();
    r.shown?.close();
    r.blob = new Blob([r.target.buffer], { type: "video/mp4" });
    r.enc = r.muxer = r.target = r.shown = r.canvas = r.g = null;
    return { size: r.blob.size, mime: "video/mp4", width: r.width, height: r.height, codec: r.codec, encoded: r.encoded ?? 0 };
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
      try { if (r.enc?.state === "configured") r.enc.close(); } catch {}
      r.shown?.close();
      recs.delete(m.id);
    }
    return { left: recs.size };
  },
};

async function frame(r, b64, at) {
  const bmp = await createImageBitmap(await (await fetch(`data:image/jpeg;base64,${b64}`)).blob());
  if (!recs.has(r.id) || r.stopped) return bmp.close();
  if (!r.enc) await begin(r, ...(r.want ?? [bmp.width, bmp.height]), at);
  r.shown?.close();
  r.shown = bmp;
  // A slow machine: let frames go rather than pile up in memory.
  if (r.enc.encodeQueueSize > MAX_QUEUE) return;
  encode(r, bmp, at, false);
  r.encoded = (r.encoded ?? 0) + 1;
}

function encode(r, bmp, at, last) {
  const { width: W, height: H } = r;
  // A frame of another size (the window was resized) is fitted, not stretched.
  // Nearly the same shape (a pixel cut off for even sides) fills the frame.
  const fx = W / bmp.width, fy = H / bmp.height;
  const s = Math.min(fx, fy);
  const fill = Math.abs(fx - fy) < 0.01;
  const w = fill ? W : Math.round(bmp.width * s), h = fill ? H : Math.round(bmp.height * s);
  if (w !== W || h !== H) { r.g.fillStyle = "#000"; r.g.fillRect(0, 0, W, H); }
  r.g.drawImage(bmp, (W - w) >> 1, (H - h) >> 1, w, h);
  // Time 0 is when recording started, as on the timeline; the first frame
  // shows from there.
  let ts = r.lastTs < 0 ? 0 : Math.round((at - r.t0) * 1000);
  if (ts <= r.lastTs) ts = r.lastTs + (last ? 1000 : 1);
  // The muxer wants a duration; it spaces frames by their timestamps anyway.
  const vf = new VideoFrame(r.canvas, { timestamp: ts, duration: 33_333 });
  const key = r.lastKey < 0 || ts - r.lastKey >= KEY_EVERY_US;
  if (key) r.lastKey = ts;
  r.enc.encode(vf, { keyFrame: key });
  vf.close();
  r.lastTs = ts;
}

async function begin(r, w, h, at) {
  // H.264 wants even sides.
  r.width = w - (w % 2);
  r.height = h - (h % 2);
  r.canvas = new OffscreenCanvas(r.width, r.height);
  r.g = r.canvas.getContext("2d", { alpha: false });
  let pick = null;
  // The software encoder first: measured on a busy machine, the GPU one
  // (Windows Media Foundation) held frames back and kept up with 6 a second,
  // the software one with 16 and more.
  for (const c of CODECS.flatMap((c) => [{ ...c, hw: "prefer-software" }, { ...c, hw: "no-preference" }])) {
    const config = { codec: c.codec, hardwareAcceleration: c.hw, width: r.width, height: r.height, bitrate: BITRATE, bitrateMode: "variable", framerate: 30, latencyMode: "realtime", ...(c.mux === "avc" ? { avc: { format: "avc" } } : {}) };
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
  r.t0 ??= at;
  r.lastTs = -1;
  r.lastKey = -1;
}

function base64(bytes) {
  let bin = "";
  for (let i = 0; i < bytes.length; i += 0x8000) bin += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return btoa(bin);
}
