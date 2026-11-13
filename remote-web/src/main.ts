import init, { Session, generateKeys } from "../../remote/noise-wasm/pkg/bytehive_noise_wasm.js";
import type { ToWeb } from "../../src/remote/protocol";
import "./styles.css";
import { el, gate, setLink, type LinkState } from "./views/ui";
import { takeTokenFromUrl, token } from "./auth";
import { isFramed } from "./frame";
import { forgetDevice, loadDevice, machines, saveMachine, saveToken, saveVault, storedDeviceId, type PairedMachine } from "./keys";
import { MachineSession, Relay, type CloseReason } from "./session";
import { restorePush } from "./push";
import { leavePair, showPair } from "./views/pair";
import { leaveHome, showHome, showMachine } from "./views/home";
import { showSignIn } from "./views/signin";
import { leaveAgent, showAgent } from "./views/agent";
import { leaveNewAgent, showNewAgent } from "./views/newagent";

const PAIR_KEY = "bytehive.pair";
const app = document.getElementById("app")!;

export type Fleet = Extract<ToWeb, { type: "fleet.state" }>;
export type Invite = Extract<ToWeb, { type: "pair.code" }>;

export interface App {
  relay: Relay;
  sessions: Map<string, MachineSession>;
  /** The last fleet each machine reported, by peer id. */
  fleet: Map<string, Fleet>;
  /** The last code a machine made for a new device, by peer id. */
  invites: Map<string, Invite>;
  /** Why a machine's session closed, by peer id, until one opens again. */
  trouble: Map<string, CloseReason>;
  /** This page's connection to the relay. */
  link: LinkState;
  /** Why the last pairing failed; the pair page clears it on the next try. */
  pairNote: string;
  render(): void;
  pair(machineId: string, code: string): void;
}

function holdPairLink(): void {
  if (location.pathname !== "/pair" || !location.hash) return;
  sessionStorage.setItem(PAIR_KEY, location.hash.slice(1));
  history.replaceState(null, "", "/");
}

/** Keeps the page to what can be seen: a phone's keyboard takes the bottom of the screen and the composer stays above it. */
function followKeyboard(): void {
  const view = window.visualViewport;
  if (!view) return;
  const fit = () => {
    document.documentElement.style.setProperty("--app-h", `${view.height}px`);
    document.documentElement.style.setProperty("--app-top", `${view.offsetTop}px`);
  };
  view.addEventListener("resize", fit);
  view.addEventListener("scroll", fit);
  fit();
}

/** Ends whatever a page holds open: a read in progress, a camera, a chat or a terminal. */
function leaveAll(): void {
  leaveHome();
  leavePair();
  leaveAgent();
  leaveNewAgent();
}

async function main(): Promise<void> {
  if (isFramed(window)) return gate(app, "ByteHive Remote", ["This page can't run inside another page. Open it on its own."]);
  followKeyboard();
  holdPairLink();
  await init();
  const device = await loadDevice({ generateKeys });
  await takeTokenFromUrl(device.id);
  const t = await token();
  if (!t) return showSignIn(app, device.id);
  const relay = new Relay({ token: t, deviceName: device.name });
  const sessions = new Map<string, MachineSession>();
  const fleet = new Map<string, Fleet>();
  const invites = new Map<string, Invite>();
  const trouble = new Map<string, CloseReason>();
  // Once the login is refused nothing is drawn over the way back in.
  let halted = false;
  const halt = (message: string, chooseDevice?: () => Promise<string | null>) => {
    halted = true;
    sessions.clear();
    leaveAll();
    showSignIn(app, device.id, message, chooseDevice);
  };

  const open = (m: PairedMachine | null, peer: string, code?: string) => {
    const s = new MachineSession({
      peer,
      noise: new Session(device.private, code),
      pairing: !!code,
      expectKey: m?.key,
      deviceName: device.name,
      sendFrame: (b) => relay.send(b),
    });
    sessions.set(peer, s);
    // A wrong code still finishes the handshake on this side (psk3 is checked by
    // the machine at the third message): only a hello that opens proves it.
    const giveUp = code ? window.setTimeout(() => {
      sessions.delete(peer);
      ctx.pairNote = "That code didn't work. Ask for a new one.";
      location.hash = "#/pair";
      ctx.render();
    }, 10_000) : 0;
    s.onOpen = () => { s.send({ type: "fleet.get" }); };
    s.onClosed = (why) => {
      sessions.delete(peer);
      trouble.set(peer, why);
      ctx.render();
    };
    // Reads of the machines wait for this, so a vault or the home list never sees it half written.
    let saved: Promise<void> = Promise.resolve();
    s.onMessage = (msg) => {
      if (msg.type === "hello") {
        window.clearTimeout(giveUp);
        trouble.delete(peer);
        // The key is pinned only now: the first message that decrypted proves
        // the machine knew the code.
        if (code && s.machine && s.remoteKey) {
          const machine = { id: s.machine.id, name: s.machine.name, key: s.remoteKey };
          saved = saveMachine(machine).then(() => {
            location.hash = `#/m/${machine.id}`;
            ctx.render();
          });
        } else {
          ctx.render();
        }
      }
      if (msg.type === "fleet.state") { fleet.set(peer, msg); ctx.render(); }
      if (msg.type === "pair.code") { invites.set(peer, msg); ctx.render(); }
      if (msg.type === "vault.set" && s.machine) {
        const id = s.machine.id;
        const key = Uint8Array.from(atob(msg.key), (c) => c.charCodeAt(0));
        void saved.then(() => saveVault(id, key));
      }
      window.dispatchEvent(new CustomEvent("remote-message", { detail: { peer, msg } }));
    };
    s.onTerm = (agent, bytes, reset) => {
      window.dispatchEvent(new CustomEvent("remote-term", { detail: { peer, agent, bytes, reset } }));
    };
    s.start();
  };

  const ctx: App = {
    relay,
    sessions,
    fleet,
    invites,
    trouble,
    link: "connecting",
    pairNote: "",
    render,
    pair: (machineId, code) => open(null, `m:${machineId}`, code),
  };

  /** The draw in progress: the home page reads the machines first. */
  let drawing: Promise<void> | undefined;

  function render(): void {
    drawing = undefined;
    if (halted) return;
    const route = location.hash.split("/").filter(Boolean);
    leaveHome();
    // Only the agent page keeps something open on a machine: a chat or a terminal.
    if (!(route[1] === "m" && route[3] === "a")) leaveAgent();
    if (!(route[1] === "m" && route[3] === "new")) leaveNewAgent();
    if (route[1] !== "pair") leavePair();
    if (route[1] === "m" && route[3] === "a") return showAgent(app, ctx, route[2], route[4]);
    if (route[1] === "m" && route[3] === "new") return showNewAgent(app, ctx, route[2]);
    if (route[1] === "m" && route[2]) return showMachine(app, ctx, route[2]);
    if (route[1] === "pair") return showPair(app, ctx);
    drawing = showHome(app, ctx);
  }

  const linkIs = (state: LinkState) => {
    ctx.link = state;
    setLink(state);
  };

  relay.onFrame = (f) => sessions.get(f.peer)?.frame(f);
  relay.onPeers = async (peers) => {
    const known = await machines();
    for (const m of known) {
      const peer = `m:${m.id}`;
      const on = peers.some((p) => p.id === peer && p.online);
      if (on && !sessions.has(peer)) open(m, peer);
      if (!on) { sessions.delete(peer); fleet.delete(peer); }
    }
    render();
  };
  relay.onUp = () => {
    linkIs("up");
    render();
    const held = sessionStorage.getItem(PAIR_KEY);
    if (held) {
      sessionStorage.removeItem(PAIR_KEY);
      const dot = held.indexOf(".");
      ctx.pair(held.slice(0, dot), held.slice(dot + 1));
    }
  };
  relay.onDown = () => {
    linkIs("down");
    sessions.clear();
    render();
  };
  relay.onAuthLost = async () => {
    await saveToken(null);
    halt("Please sign in again.");
  };
  // The relay's word is not authenticated, so nothing is erased until the person clicks.
  relay.onRevoked = () => halt("This device was removed. Sign in and pair it again.", async () => {
    // Another tab shares this storage: if it already started over, this tab's
    // identity is gone, and what is stored now belongs to the new one.
    if ((await storedDeviceId()) !== device.id) {
      location.reload();
      return null;
    }
    // The removal is account-wide: a new identity must pair with every machine again.
    await saveToken(null);
    await forgetDevice();
    return (await loadDevice({ generateKeys })).id;
  });
  // Another tab of this browser has the connection. Nothing reconnects until the person says so.
  relay.onReplaced = () => {
    halted = true;
    sessions.clear();
    leaveAll();
    const b = el("button", "Use this tab", "btn primary wide");
    b.type = "button";
    b.onclick = () => {
      halted = false;
      gate(app, "ByteHive Remote", ["Connecting."]);
      relay.resume();
    };
    gate(app, "ByteHive Remote", ["Open in another tab."], b);
  };
  relay.onQuota = (which) => gate(app, "ByteHive Remote", [`This account is over its limit (${which}).`]);
  // A new page starts at its heading, for keyboards and screen readers, as a page load would.
  window.addEventListener("hashchange", () => {
    render();
    void Promise.resolve(drawing).then(() => {
      const heading = app.querySelector("h1");
      if (!heading) return;
      heading.tabIndex = -1;
      heading.focus({ preventScroll: true });
      document.title = `${heading.textContent}: ByteHive Remote`;
    });
  });
  await relay.connect();
  render();
  void restorePush();
}

main().catch((e: unknown) => {
  gate(app, "ByteHive Remote", [`ByteHive Remote could not start: ${e instanceof Error ? e.message : String(e)}. Reload to try again.`]);
});
