// One agent: its conversation and its terminal. Names go in through textContent.

import type { RemoteAgent } from "../../../src/remote/protocol";
import type { App } from "../main";
import type { MachineSession } from "../session";
import { openChat } from "./chat";
import { openTerminal } from "./terminal";
import { bar, el, mark, statusLabel } from "./ui";

type Tab = "chat" | "terminal";

/** The agent page on screen. A fleet update must not rebuild it: that would close and reopen the terminal. */
interface Shown {
  key: string;
  session: MachineSession;
  page: HTMLElement;
  heading: HTMLElement;
  status: HTMLElement;
  /** Ends what the open tab holds on the machine. */
  stop: () => void;
}
let shown: Shown | null = null;

/** Called whenever the page is on another route: closes the terminal on the machine. */
export function leaveAgent(): void {
  shown?.stop();
  shown = null;
}

function drawName(h: HTMLElement, a: RemoteAgent): void {
  h.replaceChildren(el("span", a.project, "h-project"), el("span", " / ", "h-sep"), el("span", a.name));
}

function drawState(p: HTMLElement, a: RemoteAgent): void {
  p.replaceChildren(mark(a.status), el("span", `${statusLabel(a.status)}${a.title ? `: ${a.title}` : ""}`));
}

function notice(root: HTMLElement, machineId: string, text: string): void {
  const page = el("div", undefined, "sheet");
  page.append(bar({ back: { href: `#/m/${machineId}`, label: "Back" } }), el("h1", "Agent"), el("p", text, "machine-note"));
  root.replaceChildren(page);
}

/** Stopping ends the agent's work: it asks again on the page itself before it sends. */
function stopControl(session: MachineSession, machineId: string, agentId: string): HTMLElement {
  const box = el("div", undefined, "stop");
  const ask = el("button", "Stop this agent", "btn quiet small");
  ask.type = "button";
  ask.onclick = () => {
    const sure = el("button", "Yes, stop it", "btn danger small");
    sure.type = "button";
    sure.onclick = () => {
      session.send({ type: "agent.stop", agent: agentId });
      location.hash = `#/m/${machineId}`;
    };
    const keep = el("button", "Keep it running", "btn quiet small");
    keep.type = "button";
    keep.onclick = () => {
      box.classList.remove("asking");
      box.replaceChildren(ask);
      ask.focus();
    };
    box.classList.add("asking");
    box.replaceChildren(el("span", "Stop this agent and close its terminal? "), sure, keep);
    keep.focus();
  };
  box.append(ask);
  return box;
}

export function showAgent(root: HTMLElement, app: App, machineId: string, agentId: string): void {
  const peer = `m:${machineId}`;
  const session = app.sessions.get(peer);
  const fleet = app.fleet.get(peer);
  const agent = fleet?.agents.find((a) => a.id === agentId);
  if (!session?.open) {
    leaveAgent();
    return notice(root, machineId, "Not connected. This page comes back when the computer does.");
  }
  if (!fleet) {
    leaveAgent();
    return notice(root, machineId, "Loading.");
  }
  if (!agent) {
    leaveAgent();
    return notice(root, machineId, "Not found. This agent isn't open on the computer any more.");
  }

  const key = `${peer}/${agentId}`;
  if (shown && shown.key === key && shown.session === session && root.contains(shown.page)) {
    drawName(shown.heading, agent);
    drawState(shown.status, agent);
    return;
  }
  leaveAgent();

  const page = el("div", undefined, "session");
  const heading = el("h1");
  drawName(heading, agent);
  const status = el("p", undefined, "session-state");
  drawState(status, agent);
  const titles = el("div", undefined, "titles");
  titles.append(heading, status);
  const tabs = el("div", undefined, "tabs");
  tabs.setAttribute("role", "tablist");
  tabs.setAttribute("aria-label", "Show this agent as");
  const panel = el("div", undefined, "panel");
  panel.setAttribute("role", "tabpanel");
  const here: Shown = { key, session, page, heading, status, stop: () => {} };

  const labels: [Tab, string][] = agent.hasChat ? [["chat", "Chat"], ["terminal", "Terminal"]] : [["terminal", "Terminal"]];
  const buttons = labels.map(([tab, label]) => {
    const b = el("button", label, "tab");
    b.type = "button";
    b.id = `tab-${tab}`;
    b.setAttribute("role", "tab");
    b.onclick = () => pick(tab);
    return b;
  });

  // Leaving a tab ends what it holds: the terminal's feed stops on the machine.
  const pick = (tab: Tab): void => {
    here.stop();
    panel.replaceChildren();
    here.stop = () => {};
    for (const b of buttons) {
      const on = b.id === `tab-${tab}`;
      b.setAttribute("aria-selected", String(on));
      b.tabIndex = on ? 0 : -1;
    }
    panel.setAttribute("aria-labelledby", `tab-${tab}`);
    if (tab === "terminal") {
      // Read at open time: the size changes (Fit to this screen, a resize on the computer).
      const now = app.fleet.get(peer)?.agents.find((a) => a.id === agentId);
      if (now) here.stop = openTerminal(panel, session, peer, now);
      else panel.append(el("p", "Not found.", "machine-note"));
    } else here.stop = openChat(panel, session, peer, agentId, () => pick("terminal"));
  };

  tabs.append(...buttons);
  tabs.onkeydown = (e) => {
    if (e.key !== "ArrowLeft" && e.key !== "ArrowRight") return;
    const next = buttons[(buttons.indexOf(e.target as HTMLButtonElement) + 1) % buttons.length];
    next.focus();
    next.click();
  };

  const row = el("div", undefined, "tab-row");
  row.append(tabs, stopControl(session, machineId, agentId));
  const head = el("div", undefined, "session-head");
  head.append(bar({ back: { href: `#/m/${machineId}`, label: "Back" }, middle: titles }), row);
  page.append(head, panel);
  root.replaceChildren(page);
  shown = here;
  pick(labels[0][0]);
}
