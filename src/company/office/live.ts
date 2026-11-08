/* The office's view of the real company. */
import { readBoardFile } from "../../boardfile";
import { revealPane } from "../../agentbridge";
import { activeWs, workspaces } from "../../appstate";
import { onTasksChange, taskOf } from "../../tasks";
import * as engine from "../engine";
import { floorContact } from "../org";
import { officeModel, type BoardCounts, type JobCards, type OfficePane } from "./model";
import { deskReport } from "./report";
import type { OfficeSource } from "./source";

/** How often the wall boards are read while the office is on screen. */
const BOARDS_MS = 5000;

/** How long after its last output a terminal counts as printing. */
const PRINTING_MS = 2500;

const panesNow = (now: number): OfficePane[] => [...workspaces.values()].flatMap((ws) => [...ws.panes.values()].flatMap((p) =>
  (p.spec.personId ? [{ personId: p.spec.personId, wsId: ws.id, paneId: p.id, task: taskOf(p.id) ?? null, printing: now - p.lastOutputAt < PRINTING_MS }] : [])));

const count = (lists: Array<{ title: string; cards: unknown[] }>, title: string) =>
  lists.find((l) => l.title.trim().toLowerCase() === title)?.cards.length ?? 0;

export function liveSource(): OfficeSource {
  const boards: Record<string, BoardCounts> = {};
  let jobCards: Record<string, JobCards> = {};
  let timer: number | null = null;

  const readBoards = async () => {
    const s = engine.companyView();
    const byJob: Record<string, JobCards> = {};
    for (const f of s?.company.floors.filter((x) => s.company.people.some((p) => p.floor === x.id)) ?? []) {
      const file = await readBoardFile(f.dir).catch(() => null);
      if (!file) continue;
      boards[f.id] = { todo: count(file.board.lists, "to do"), doing: count(file.board.lists, "doing"), done: count(file.board.lists, "done") };
      for (const l of file.board.lists) {
        const done = l.title.trim().toLowerCase() === "done";
        for (const c of l.cards) {
          if (!c.job) continue;
          const t = (byJob[c.job] ??= { done: 0, total: 0 });
          t.total += 1;
          if (done) t.done += 1;
        }
      }
    }
    jobCards = byJob;
  };

  return {
    snapshot(now) {
      const s = engine.companyView();
      if (!s) return { model: null, note: engine.companyElsewhere(), error: engine.companyError() };
      const openDirs = [...workspaces.values()].flatMap((w) => (w.dir ? [w.dir] : []));
      return { model: officeModel(s.company, s.jobs, engine.companyAlerts(), panesNow(now), { roles: s.roles, boards, jobCards, openDirs, flights: engine.companyFlights(now) }), note: null, error: null };
    },
    onChange(fn) {
      const off = engine.onCompanyChange(fn);
      const offTasks = onTasksChange(fn);
      return () => { off(); offTasks(); };
    },
    giveJob: engine.giveJob,
    messageFloor: async (floorId, text) => {
      const s = engine.companyView();
      const who = s && floorContact(s.company, floorId);
      if (!who) throw new Error(`Nobody on ${s?.company.floors.find((f) => f.id === floorId)?.name ?? "this floor"} can get a message.`);
      await engine.messagePerson(who.id, text);
    },
    approve: engine.approve,
    reject: engine.reject,
    raiseCap: engine.raiseCap,
    finishJob: engine.finishJob,
    stopJob: engine.stopJob,
    dismiss: engine.dismissAlert,
    showPerson(id, focus = true) {
      for (const ws of workspaces.values()) for (const p of ws.panes.values()) if (p.spec.personId === id) return revealPane(ws.id, p.id, focus);
      return false;
    },
    wakePerson: engine.wakePerson,
    async report(personId) {
      const s = engine.companyView();
      const p = s?.company.people.find((x) => x.id === personId);
      if (!s || !p) return null;
      const dir = s.company.floors.find((f) => f.id === p.floor)?.dir ?? activeWs?.dir ?? null;
      const file = dir ? await readBoardFile(dir).catch(() => null) : null;
      return deskReport({ id: p.id, name: p.name, branch: p.branch ?? null }, file?.board ?? null, s.jobs);
    },
    watching(on) {
      if (on && timer === null) {
        void readBoards();
        timer = window.setInterval(() => void readBoards(), BOARDS_MS);
      } else if (!on && timer !== null) {
        window.clearInterval(timer);
        timer = null;
      }
    },
  };
}
