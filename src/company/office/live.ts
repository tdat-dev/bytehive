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

/** One board's cards by the Job they belong to. A Map: a Job id comes from a file in a repo,
 *  and "__proto__" as a key of a plain object would write to every object in the page. */
export function cardsOnBoard(lists: Array<{ title: string; cards: Array<{ job?: string }> }>): Map<string, JobCards> {
  const byJob = new Map<string, JobCards>();
  for (const l of lists) {
    const done = l.title.trim().toLowerCase() === "done";
    for (const c of l.cards) {
      if (!c.job) continue;
      const t = byJob.get(c.job) ?? { done: 0, total: 0 };
      t.total += 1;
      if (done) t.done += 1;
      byJob.set(c.job, t);
    }
  }
  return byJob;
}

/** Every floor's cards by Job, added up. fromEntries makes each key an own property, whatever it is. */
export function cardsByJob(floors: Iterable<Map<string, JobCards>>): Record<string, JobCards> {
  const all = new Map<string, JobCards>();
  for (const f of floors) {
    for (const [job, c] of f) {
      const t = all.get(job) ?? { done: 0, total: 0 };
      all.set(job, { done: t.done + c.done, total: t.total + c.total });
    }
  }
  return Object.fromEntries(all);
}

export function liveSource(): OfficeSource {
  const boards: Record<string, BoardCounts> = {};
  /** Each floor's last good tally: a board that could not be read this time keeps its count. */
  const floorCards = new Map<string, Map<string, JobCards>>();
  let jobCards: Record<string, JobCards> = {};
  let timer: number | null = null;

  const readBoards = async () => {
    const s = engine.companyView();
    const floors = s?.company.floors.filter((x) => s.company.people.some((p) => p.floor === x.id)) ?? [];
    for (const id of [...floorCards.keys()]) if (!floors.some((f) => f.id === id)) floorCards.delete(id);
    for (const f of floors) {
      const file = await readBoardFile(f.dir).catch(() => null);
      if (!file) continue;
      boards[f.id] = { todo: count(file.board.lists, "to do"), doing: count(file.board.lists, "doing"), done: count(file.board.lists, "done") };
      floorCards.set(f.id, cardsOnBoard(file.board.lists));
    }
    jobCards = cardsByJob(floorCards.values());
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
