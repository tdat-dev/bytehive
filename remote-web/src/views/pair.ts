import type { App } from "../main";
import { bar, el } from "./ui";

export function parsePairLink(text: string): { machine: string; code: string } | null {
  const hash = text.includes("#") ? text.slice(text.indexOf("#") + 1) : text;
  const dot = hash.indexOf(".");
  if (dot < 1) return null;
  return { machine: hash.slice(0, dot), code: hash.slice(dot + 1) };
}

/** The form on screen. A peers message must not rebuild it: a half-typed code and a running scan would go. */
let shown: { page: HTMLElement; draw(app: App): void; stop(): void } | null = null;

/** Called whenever the page is on another route: the camera goes off with it. */
export function leavePair(): void {
  shown?.stop();
  shown = null;
}

export function showPair(root: HTMLElement, app: App): void {
  if (shown && root.contains(shown.page)) return shown.draw(app);
  leavePair();

  const page = el("div", undefined, "sheet");
  const form = el("form", undefined, "fields");
  form.noValidate = true;

  const computerField = el("label", "Computer");
  const computer = el("select");
  computer.id = "m";
  computerField.append(computer);
  const none = el("p", undefined, "machine-note");

  const codeField = el("label", "Code");
  const code = el("input");
  code.id = "c";
  code.autocomplete = "off";
  code.spellcheck = false;
  code.setAttribute("autocapitalize", "characters");
  codeField.append(code);

  const status = el("p", undefined, "form-note");
  status.setAttribute("role", "status");
  const go = el("button", "Pair", "btn primary wide");
  go.id = "go";
  go.type = "submit";
  form.append(computerField, none, codeField, status, go);

  const scan = el("button", "Scan the QR code", "btn quiet wide");
  scan.id = "scan";
  scan.type = "button";
  const video = el("video", undefined, "scanner");
  video.id = "v";
  video.hidden = true;
  video.muted = true;
  video.setAttribute("playsinline", "");

  page.append(
    bar({ back: { href: "#/", label: "All computers" } }),
    el("h1", "Pair this device"),
    el("p", "On your computer, open ByteHive's settings, go to Remote and press Show code.", "machine-note"),
    form, scan, video,
  );
  root.replaceChildren(page);

  let waiting = false;
  let scanner: { destroy(): void } | null = null;
  /** Bumped when a scan stops, so one still starting finds out it is not wanted. */
  let scanTurn = 0;

  const stopScan = () => {
    scanTurn++;
    scanner?.destroy();
    scanner = null;
    video.hidden = true;
    scan.textContent = "Scan the QR code";
  };

  const pair = (machine: string, text: string, name: string) => {
    waiting = true;
    go.disabled = true;
    app.pairNote = "";
    status.textContent = `Pairing with ${name}.`;
    app.pair(machine, text);
  };

  const draw = (now: App) => {
    // Ids and names come from the relay: set as text and values, never as markup.
    const list = now.relay.peers.filter((p) => p.id.startsWith("m:") && p.online);
    const chosen = computer.value;
    computer.replaceChildren(...list.map((p) => {
      const o = el("option", p.name);
      o.value = p.id.slice(2);
      return o;
    }));
    computer.value = list.some((p) => p.id === `m:${chosen}`) ? chosen : list[0]?.id.slice(2) ?? "";
    computerField.hidden = !list.length;
    none.textContent = list.length ? "" : "No computer is online. Keep ByteHive open with Remote on and it shows up here. Scanning its QR code works without this list.";
    none.hidden = !!list.length;
    if (now.pairNote) {
      waiting = false;
      go.disabled = false;
      status.textContent = now.pairNote;
      now.pairNote = "";
    }
  };

  form.onsubmit = (e) => {
    e.preventDefault();
    if (waiting) return;
    const text = code.value.trim();
    if (!computer.value) status.textContent = "No computer to pair with yet.";
    else if (!text) {
      status.textContent = "Type the code your computer shows.";
      code.focus();
    } else pair(computer.value, text, computer.selectedOptions[0]?.textContent ?? "the computer");
  };

  scan.onclick = async () => {
    if (!video.hidden) return stopScan();
    const mine = ++scanTurn;
    video.hidden = false;
    scan.textContent = "Stop the camera";
    status.textContent = "";
    const { default: QrScanner } = await import("qr-scanner");
    if (mine !== scanTurn) return;
    const s = new QrScanner(video, (r) => {
      const p = parsePairLink(r.data);
      if (!p) return;
      stopScan();
      pair(p.machine, p.code, "the computer");
    }, { returnDetailedScanResult: true });
    scanner = s;
    await s.start().catch(() => {
      if (mine !== scanTurn) return;
      stopScan();
      status.textContent = "This browser didn't let the page use the camera. Type the code instead.";
    });
  };

  draw(app);
  shown = { page, draw, stop: stopScan };
}
