// Starts an agent on the computer. The computer always asks permission for what a
// remote start does, and only the CLIs it lists can be started: nothing typed here
// becomes a command.

import type { ToWeb } from "../../../src/remote/protocol";
import type { App, Fleet } from "../main";
import type { MachineSession } from "../session";
import { bar, el } from "./ui";

/** How long the page waits to hear the new agent before Start can be pressed again. */
const NO_ANSWER_MS = 20_000;

function field(label: string, input: HTMLElement): HTMLElement {
  const l = el("label", label);
  l.append(input);
  return l;
}

function options(items: { id: string; label: string }[]): HTMLSelectElement {
  const s = el("select");
  for (const i of items) {
    const o = el("option", i.label);
    o.value = i.id;
    s.append(o);
  }
  return s;
}

/** The form on screen. A fleet update must not rebuild it: what was typed would be lost. */
let shown: { key: string; session: MachineSession; page: HTMLElement; heard(fleet: Fleet | undefined): void; stop: () => void } | null = null;

export function leaveNewAgent(): void {
  shown?.stop();
  shown = null;
}

export function showNewAgent(root: HTMLElement, app: App, machineId: string): void {
  const peer = `m:${machineId}`;
  const session = app.sessions.get(peer);
  const fleet = app.fleet.get(peer);
  if (session?.open && shown && shown.key === peer && shown.session === session && root.contains(shown.page)) return shown.heard(fleet);
  leaveNewAgent();

  const page = el("div", undefined, "sheet");
  page.append(bar({ back: { href: `#/m/${machineId}`, label: "Back" } }), el("h1", "New agent"));
  if (session?.machine) page.append(el("p", `On ${session.machine.name}`, "machine-note"));
  if (!session?.open || !fleet) {
    page.append(el("p", session?.open ? "Loading." : "Not connected.", "machine-note"));
    root.replaceChildren(page);
    return;
  }
  if (!fleet.projects.length || !fleet.presets.length) {
    page.append(el("p", "There is no project open on that computer to start an agent in.", "machine-note"));
    root.replaceChildren(page);
    return;
  }

  const form = el("form", undefined, "fields");
  const project = options(fleet.projects.map((p) => ({ id: p.id, label: p.name })));
  const preset = options(fleet.presets);
  const task = el("textarea");
  task.rows = 4;
  const start = el("button", "Start", "btn primary wide");
  start.type = "submit";
  const status = el("p", undefined, "form-note");
  status.setAttribute("role", "status");
  form.append(field("Project", project), field("CLI", preset), field("Task (optional)", task), status, start);

  /** The agents there were when Start was pressed; null when nothing is on its way. */
  let before: Set<string> | null = null;
  let wait = 0;
  const settle = (text: string) => {
    window.clearTimeout(wait);
    before = null;
    start.disabled = false;
    status.textContent = text;
  };

  form.onsubmit = (e) => {
    e.preventDefault();
    if (before) return;
    before = new Set(app.fleet.get(peer)?.agents.map((a) => a.id));
    start.disabled = true;
    session.send({ type: "agent.new", project: project.value, preset: preset.value, task: task.value.trim() });
    status.textContent = "Asked the computer to start it. It appears in the list when it is up.";
    wait = window.setTimeout(() => settle("The computer hasn't answered. Look in the list before you start another."), NO_ANSWER_MS);
  };
  // The only answer to a start that can't happen is an error from the computer.
  const listener = (e: Event) => {
    const { peer: from, msg } = (e as CustomEvent<{ peer: string; msg: ToWeb }>).detail;
    if (from === peer && msg.type === "error" && (msg.code === "no-project" || msg.code === "no-preset")) settle(msg.message);
  };
  window.addEventListener("remote-message", listener);

  page.append(form);
  root.replaceChildren(page);
  shown = {
    key: peer,
    session,
    page,
    heard: (now) => {
      const up = before && now?.agents.find((a) => !before!.has(a.id));
      if (!up) return;
      task.value = "";
      settle(`${up.name} is up. It's in the list.`);
    },
    stop: () => {
      window.clearTimeout(wait);
      window.removeEventListener("remote-message", listener);
    },
  };
}
