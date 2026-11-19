/* The recorder: an offscreen document, because a service worker has no
 * canvas stream and no MediaRecorder. The service worker sends it the tab's
 * screencast frames (JPEG); it paints them on a canvas and records the canvas.
 * Frames arrive only when the page paints, so a still page costs nothing; a
 * beat every half second keeps the video's clock going through it. */

const TYPES = ["video/mp4;codecs=avc1.640028", "video/mp4;codecs=avc1", "video/mp4", "video/webm;codecs=vp9", "video/webm"];
const BITRATE = 4_000_000;
const BEAT_MS = 500;

const recs = new Map(); // id -> { canvas, g, track, recorder, chunks, mime, chain, last, beat, blob }

chrome.runtime.onMessage.addListener((m, _sender, reply) => {
  if (m?.target !== "bytehive-offscreen") return;
  if (m.op === "frame") {
    // Answered at once: the worker never waits on a frame.
    reply(true);
    const r = recs.get(m.id);
    if (r) r.chain = r.chain.then(() => draw(r, m.data)).catch(() => {});
    return;
  }
  ops[m.op]?.(m).then(reply, (e) => reply({ error: String(e?.message ?? e) }));
  return true;
});

const ops = {
  async start(m) {
    recs.get(m.id)?.recorder?.stop();
    recs.set(m.id, { chunks: [], chain: Promise.resolve(), last: 0 });
    return { ok: true };
  },
  async stop(m) {
    const r = recs.get(m.id);
    if (!r) throw new Error("No such recording.");
    await r.chain;
    clearInterval(r.beat);
    if (!r.recorder) { recs.delete(m.id); return { size: 0 }; }
    if (r.recorder.state !== "inactive") {
      const done = new Promise((ok) => (r.recorder.onstop = ok));
      r.recorder.stop();
      await done;
    }
    r.track.stop();
    r.blob = new Blob(r.chunks, { type: r.mime });
    r.chunks = [];
    r.canvas = r.g = null;
    return { size: r.blob.size, mime: r.mime.split(";")[0], width: r.width, height: r.height };
  },
  async part(m) {
    const blob = recs.get(m.id)?.blob;
    if (!blob) throw new Error("The video is gone.");
    const bytes = new Uint8Array(await blob.slice(m.offset, m.offset + m.size).arrayBuffer());
    return { data: base64(bytes) };
  },
  async drop(m) {
    const r = recs.get(m.id);
    if (r?.recorder?.state === "recording") r.recorder.stop();
    clearInterval(r?.beat);
    recs.delete(m.id);
    return { left: recs.size };
  },
};

async function draw(r, b64) {
  const blob = await (await fetch(`data:image/jpeg;base64,${b64}`)).blob();
  const bmp = await createImageBitmap(blob);
  try {
    if (!r.recorder) begin(r, bmp.width, bmp.height);
    const { width: W, height: H } = r.canvas;
    // A frame of another size (the window was resized) is fitted, not stretched.
    const s = Math.min(W / bmp.width, H / bmp.height);
    const w = Math.round(bmp.width * s), h = Math.round(bmp.height * s);
    if (w !== W || h !== H) { r.g.fillStyle = "#000"; r.g.fillRect(0, 0, W, H); }
    r.g.drawImage(bmp, (W - w) >> 1, (H - h) >> 1, w, h);
  } finally {
    bmp.close();
  }
  r.track.requestFrame();
  r.last = performance.now();
}

function begin(r, w, h) {
  // H.264 wants even sides.
  r.width = w - (w % 2);
  r.height = h - (h % 2);
  r.canvas = document.createElement("canvas");
  r.canvas.width = r.width;
  r.canvas.height = r.height;
  r.g = r.canvas.getContext("2d", { alpha: false });
  const stream = r.canvas.captureStream(0);
  r.track = stream.getVideoTracks()[0];
  r.mime = TYPES.find((t) => MediaRecorder.isTypeSupported(t)) ?? "";
  r.recorder = new MediaRecorder(stream, { ...(r.mime ? { mimeType: r.mime } : {}), videoBitsPerSecond: BITRATE });
  r.mime = r.recorder.mimeType || r.mime || "video/webm";
  r.recorder.ondataavailable = (e) => { if (e.data.size) r.chunks.push(e.data); };
  r.recorder.start(1000);
  r.beat = setInterval(() => { if (r.track && performance.now() - r.last > BEAT_MS - 50) r.track.requestFrame(); }, BEAT_MS);
}

function base64(bytes) {
  let bin = "";
  for (let i = 0; i < bytes.length; i += 0x8000) bin += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return btoa(bin);
}
