// Every agent on every paired computer, and one computer on its own page. Every
// name that came from the relay or a machine goes in through textContent, never as markup.

import type { RemoteAgent } from "../../../src/remote/protocol";
import { machines, type PairedMachine } from "../keys";
import type { App, Invite } from "../main";
import { notifyControl } from "../push";
import { bar, el, icon, link, mark, needsYouFirst, say, statusLabel, swap, themeButton } from "./ui";

export function ago(at: number, now = Date.now()): string {
  const minutes = Math.floor((now - at) / 60_000);
  const plural = (n: number, unit: string) => `${n} ${unit}${n === 1 ? "" : "s"} ago`;
  if (minutes < 1) return "just now";
  if (minutes < 60) return plural(minutes, "minute");
  if (minutes < 1440) return plural(Math.floor(minutes / 60), "hour");
  return plural(Math.floor(minutes / 1440), "day");
}

/** What is known about a computer right now, in words. The state of the mark goes with them. */
function machineState(app: App, id: string): { state: string; text: string } {
  const peer = `m:${id}`;
  if (app.link !== "up") return { state: "connecting", text: "Waiting for the connection" };
  const trouble = app.trouble.get(peer);
  if (trouble === "wrong-key") return { state: "stopped", text: "This computer's key changed. Pair it again." };
  if (trouble === "update") return { state: "stopped", text: "Update ByteHive on this computer" };
  if (app.sessions.get(peer)?.open) return { state: "up", text: "Online" };
  const seen = app.relay.peers.find((p) => p.id === peer);
  if (seen?.online) return { state: "connecting", text: "Connecting" };
  return { state: "offline", text: seen?.lastSeen ? `Offline, last seen ${ago(seen.lastSeen)}` : "Offline" };
}

function stateLine(s: { state: string; text: string }): HTMLElement {
  const p = el("p", undefined, "machine-state");
  p.append(mark(s.state), el("span", s.text));
  return p;
}

function agentRow(a: RemoteAgent, machineId: string, pinned?: string): HTMLElement {
  const li = el("li");
  const row = link("", `#/m/${machineId}/a/${a.id}`, "agent");
  const words = el("span", undefined, "agent-words");
  words.append(el("span", pinned ? `${a.project} / ${a.name}` : a.name, "agent-name"));
  const note = pinned ? [a.title, pinned].filter(Boolean).join(" · ") : a.title;
  if (note) words.append(el("span", note, "agent-note"));
  row.append(mark(a.status), words);
  if (!pinned) row.append(el("span", statusLabel(a.status), "agent-status"));
  row.append(icon("next"));
  li.append(row);
  return li;
}

/** The agents of one computer under their projects, in name order. */
function projects(agents: RemoteAgent[], machineId: string): HTMLElement[] {
  const by = new Map<string, RemoteAgent[]>();
  for (const a of agents) by.set(a.project, [...(by.get(a.project) ?? []), a]);
  return [...by.keys()].sort((x, y) => x.localeCompare(y)).map((name) => {
    const p = el("div", undefined, "project");
    const list = el("ul", undefined, "agents");
    list.append(...by.get(name)!.sort(needsYouFirst).map((a) => agentRow(a, machineId)));
    p.append(el("h3", name), list);
    return p;
  });
}

function machineCard(app: App, m: PairedMachine): HTMLElement {
  const peer = `m:${m.id}`;
  const session = app.sessions.get(peer);
  const s = el("section", undefined, "machine");
  const head = el("div", undefined, "machine-head");
  const h = el("h2");
  const open = link(session?.machine?.name ?? m.name, `#/m/${m.id}`);
  open.append(icon("next"));
  h.append(open);
  const state = machineState(app, m.id);
  head.append(h, stateLine(state));
  s.append(head);
  const fleet = app.fleet.get(peer);
  if (state.state === "offline") {
    s.append(el("p", "Its agents show here again when ByteHive is open on it with Remote on.", "machine-note"));
  } else if (session?.open && fleet) {
    if (fleet.agents.length) s.append(...projects(fleet.agents.filter((a) => a.status !== "needs"), m.id));
    else {
      const none = el("p", "No agents running. ", "machine-note");
      none.append(link("Start one", `#/m/${m.id}/new`));
      s.append(none);
    }
  } else if (session?.open) s.append(el("p", "Loading its agents.", "machine-note"));
  return s;
}

function pulse(app: App, known: PairedMachine[]): HTMLElement[] {
  const agents = known.flatMap((m) => app.fleet.get(`m:${m.id}`)?.agents ?? []);
  const needs = agents.filter((a) => a.status === "needs").length;
  const title = needs ? `${needs} agent${needs === 1 ? " needs" : "s need"} you` : "Nothing needs you";
  const count = (s: string) => agents.filter((a) => a.status === s).length;
  const parts = ["working", "idle", "done", "stopped"].map((s) => [count(s), s] as const).filter(([n]) => n).map(([n, s]) => `${n} ${s}`);
  const online = known.filter((m) => app.sessions.get(`m:${m.id}`)?.open).length;
  const running = parts.length ? `${parts.join(", ")}.` : needs ? "Nothing else is running." : "No agents running.";
  const line = app.link !== "up" ? "Waiting for the connection."
    : !online ? "No computer is online."
    : known.length > 1 ? `${running} ${online} of ${known.length} computers online.`
    : running;
  return [el("h1", title, "pulse"), el("p", line, "pulse-line")];
}

/** Bumped by every draw and by leaving: the read of the machines that started last is the one that lands. */
let turn = 0;

export function leaveHome(): void {
  turn++;
}

export async function showHome(root: HTMLElement, app: App): Promise<void> {
  const mine = ++turn;
  const known = await machines();
  if (mine !== turn) return;

  const page = el("div", undefined, "home");
  const grid = el("div", undefined, "home-grid");
  const lead = el("div", undefined, "home-lead");
  const rest = el("div", undefined, "home-rest");
  grid.append(lead, rest);
  page.append(bar({}), grid);
  const foot = el("div", undefined, "foot");

  if (!known.length) {
    lead.append(el("h1", "No computer paired yet", "pulse"));
    lead.append(el("p", "Turn on Remote in ByteHive's settings on your computer. It gives you a code to pair this device with.", "pulse-line"));
    lead.append(link("Pair a computer", "#/pair", "btn primary"), foot);
    foot.append(link("Saved sessions", "#/saved", "btn quiet"), themeButton());
  } else {
    lead.append(...pulse(app, known));
    const needs = known.flatMap((m) => (app.fleet.get(`m:${m.id}`)?.agents ?? [])
      .filter((a) => a.status === "needs")
      .map((a) => agentRow(a, m.id, app.sessions.get(`m:${m.id}`)?.machine?.name ?? m.name)));
    if (needs.length) {
      const s = el("section", undefined, "needs");
      const list = el("ul", undefined, "agents");
      list.append(...needs);
      s.append(el("h2", "Needs you", "sr"), list);
      lead.append(s);
    }
    rest.append(...known.map((m) => machineCard(app, m)));
    foot.append(link("Pair a computer", "#/pair", "btn quiet"), link("Saved sessions", "#/saved", "btn quiet"), notifyControl(), themeButton());
    rest.append(foot);
  }
  swap(root, page);
}

/** The panel of the code on screen: a fleet update must not draw the same QR again. */
let shownInvite: { key: string; node: HTMLElement } | null = null;

function invitePanel(machineId: string, invite: Invite): HTMLElement {
  const key = `${machineId}.${invite.code}`;
  if (shownInvite?.key === key) return shownInvite.node;
  const box = el("div", undefined, "invite");
  const canvas = el("canvas");
  canvas.setAttribute("role", "img");
  canvas.setAttribute("aria-label", "QR code that pairs a new device");
  const until = new Date(invite.expires).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  box.append(canvas, el("code", invite.code, "invite-code"), el("p", `Scan it with the new device, or type the code there. It works once, until ${until}.`));
  // The same link the computer's own QR holds: a phone's camera opens it straight onto this app.
  const pairLink = `${location.origin}/pair#${key}`;
  void import("qrcode")
    .then(({ default: QRCode }) => QRCode.toCanvas(canvas, pairLink, { width: 224, margin: 2 }))
    .catch(() => canvas.remove());
  shownInvite = { key, node: box };
  say(`New device code: ${invite.code}`);
  return box;
}

export function showMachine(root: HTMLElement, app: App, machineId: string): void {
  const peer = `m:${machineId}`;
  const session = app.sessions.get(peer);
  const page = el("div", undefined, "sheet");
  const name = session?.machine?.name ?? app.relay.peers.find((p) => p.id === peer)?.name ?? "Computer";
  page.append(bar({ back: { href: "#/", label: "All computers" } }), el("h1", name), stateLine(machineState(app, machineId)));
  const fleet = app.fleet.get(peer);
  if (!session?.open || !fleet) {
    page.append(el("p", session?.open ? "Loading its agents." : "Not connected. Its agents show here when ByteHive is open on it with Remote on.", "machine-note"));
    swap(root, page);
    return;
  }
  const add = el("button", "Add a device", "btn quiet");
  add.id = "invite";
  add.type = "button";
  add.onclick = () => session.send({ type: "pair.invite" });
  const actions = el("div", undefined, "actions");
  actions.append(link("New agent", `#/m/${machineId}/new`, "btn primary"), add);
  if (fleet.agents.length) page.append(...projects(fleet.agents, machineId));
  else page.append(el("p", "No agents running on this computer.", "machine-note"));
  page.append(actions);
  const invite = app.invites.get(peer);
  const fresh = invite && invite.expires > Date.now() ? invitePanel(machineId, invite) : null;
  if (fresh) page.append(fresh);
  const justMade = fresh && !fresh.isConnected;
  swap(root, page);
  // On a phone it lands under the list.
  if (justMade) fresh.scrollIntoView?.({ block: "nearest" });
}
