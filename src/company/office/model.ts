/* The office as a plain object. The scene and the shell read this and nothing else,
 * so what they show cannot drift from the company, and all of it can be tested. */
import { statusLine, type TaskStatus } from "../../taskstate";
import type { Task } from "../../tasks";
import { sameFolder } from "../../workspaces";
import { needsYou, type NeedRow } from "../needs";
import type { Company, Job, JobStatus, Person, Rank, RoleDef } from "../types";
import type { Alert } from "../watch";

export type OfficeState = "needs" | "working" | "review" | "idle" | "stopped";
export type OfficeTask = Pick<Task, "status" | "title">;

/** `printing`: its terminal really put something out in the last moments. */
export interface OfficePane { personId: string; wsId: string; paneId: string; task: OfficeTask | null; printing?: boolean }
export interface BoardCounts { todo: number; doing: number; done: number }
/** A message on its way, for the paper plane. `at` is when it left. */
export interface Flight { id: string; from: string; to: string; at: number }

export interface OfficeExtra {
  roles?: RoleDef[];
  boards?: Record<string, BoardCounts>;
  flights?: Flight[];
  openDirs?: string[];
}

export interface OfficePerson {
  id: string;
  name: string;
  rank: Rank;
  title: string;
  department: string;
  cli: string;
  shirt: string;
  hair: string;
  prop: string;
  state: OfficeState;
  /** Their terminal is putting something out right now. Only that makes marks rise from a desk. */
  printing?: boolean;
  doing: string | null;
  floorId: string | null;
  seat: number;
  paneId: string | null;
  wsId: string | null;
}

export interface OfficeFloor {
  id: string;
  name: string;
  dir: string;
  open: boolean;
  lit: boolean;
  people: OfficePerson[];
  lead: string | null;
  needs: number;
  papers: number;
  board: BoardCounts | null;
}

export interface OfficeJob { id: string; title: string; status: JobStatus; label: string; tokensUsed: number; tokenCap: number }

export interface OfficeModel {
  founder: OfficePerson | null;
  floors: OfficeFloor[];
  needs: NeedRow[];
  jobs: OfficeJob[];
  flights: Flight[];
  counts: { people: number; working: number; needs: number };
}

const MAX_PAPERS = 3;

/** The Jobs still going, as the rail names them. */
const GOING: Partial<Record<JobStatus, string>> = { planning: "Planning", "awaiting-plan": "Waiting for your approval", running: "Running", paused: "Paused" };

export const FOUNDER_SHIRT = "#eef0f4";
const SHIRTS = new Map([["Management", "#5ec2f0"], ["Engineering", "#5fd08a"], ["Design", "#ff6b6b"], ["Marketing", "#e0913a"], ["Research", "#b18cf0"]]);
const SPARE_SHIRTS = ["#7d8b99", "#c98fb4", "#9db36b", "#6f9fd8"];
export const HAIRS = ["#2a2e3a", "#4a3226", "#7a5230", "#b08a55", "#55585f", "#7a2f2a"];

const hash = (s: string) => [...s].reduce((n, c) => (n * 31 + c.charCodeAt(0)) >>> 0, 7);

/** What everyone chosen by the app sees. A role from the Founder may name any department. */
export const departmentShirt = (department: string): string => SHIRTS.get(department) ?? SPARE_SHIRTS[hash(department) % SPARE_SHIRTS.length];
export const hairFor = (id: string): string => HAIRS[hash(id) % HAIRS.length];

/** A look comes from a file on disk, so a colour is only trusted once it is one. */
const colourOr = (c: unknown, fallback: string): string => (typeof c === "string" && /^#(?:[0-9a-f]{3}|[0-9a-f]{6})$/i.test(c) ? c : fallback);

export function lookOf(p: Person, role: RoleDef | undefined): { shirt: string; hair: string; prop: string } {
  const shirt = p.rank === "founder" ? FOUNDER_SHIRT : departmentShirt(role?.department ?? "");
  const hair = hairFor(p.id);
  if (p.look.edited) return { shirt: colourOr(p.look.shirt, shirt), hair: colourOr(p.look.hair, hair), prop: p.look.prop };
  return { shirt: role || p.rank === "founder" ? shirt : colourOr(p.look.shirt, shirt), hair, prop: p.look.prop };
}

/** The Founder always exists, and every open project is a floor, so only a Job says
 *  that someone used the Company. */
export const companyUsed = (jobs: Job[]): boolean => jobs.length > 0;

/** The one place a character's state comes from. Stopped covers no pane and a merged agent: the chair is empty. */
export function officeState(task: Pick<Task, "status"> | null | undefined): OfficeState {
  const s = task?.status.state;
  return s === "needs" || s === "working" || s === "review" || s === "idle" ? s : "stopped";
}

const titleCase = (id: string) => id.replace(/(^|-)([a-z])/g, (_, a: string, b: string) => `${a ? " " : ""}${b.toUpperCase()}`);

function toPerson(p: Person, role: RoleDef | undefined, pane: OfficePane | undefined, seat: number): OfficePerson {
  return {
    id: p.id, name: p.name, rank: p.rank, title: role?.title ?? titleCase(p.role), department: role?.department ?? "",
    cli: p.cli, ...lookOf(p, role), state: officeState(pane?.task), printing: !!pane?.printing, doing: pane?.task?.title ?? null,
    floorId: p.floor, seat, paneId: pane?.paneId ?? null, wsId: pane?.wsId ?? null,
  };
}

/** What a person is asking shows in Needs you: they run with skip permissions, so it is rare, and worth a line. */
const askRow = (p: OfficePerson, status: TaskStatus): NeedRow =>
  ({ key: `ask-${p.id}`, kind: "alert", jobId: null, personId: p.id, title: statusLine(p.name, status), detail: "", standing: false });

export function officeModel(company: Company, jobs: Job[], alerts: Alert[], panes: OfficePane[], extra: OfficeExtra = {}): OfficeModel {
  const roleOf = (id: string) => extra.roles?.find((r) => r.id === id);
  const paneOf = (id: string) => panes.find((p) => p.personId === id);
  const open = (dir: string) => (extra.openDirs ?? []).some((d) => sameFolder(d, dir));

  const top = company.people.find((p) => p.rank === "founder");
  const founder = top ? toPerson(top, roleOf(top.role), paneOf(top.id), 0) : null;

  const rows = needsYou(jobs, alerts, { floors: company.floors, roles: extra.roles ?? [] });

  const floors = company.floors.map((f): OfficeFloor => {
    const here = company.people.filter((p) => p.floor === f.id);
    const seated = [...here.filter((p) => p.rank === "lead"), ...here.filter((p) => p.rank !== "lead")];
    const people = seated.map((p, seat) => toPerson(p, roleOf(p.role), paneOf(p.id), seat));
    const wanted = new Set([...people.filter((p) => p.state === "needs").map((p) => p.id), ...rows.filter((r) => people.some((p) => p.id === r.personId)).map((r) => r.personId!)]);
    return {
      id: f.id, name: f.name, dir: f.dir, open: open(f.dir), lit: open(f.dir) && people.some((p) => p.state !== "stopped"), people,
      lead: people.find((p) => p.rank === "lead")?.id ?? null, needs: wanted.size,
      papers: Math.min(MAX_PAPERS, people.filter((p) => p.state === "review").length), board: extra.boards?.[f.id] ?? null,
    };
  }).filter((f) => f.people.length || f.open);

  const everyone = [...(founder ? [founder] : []), ...floors.flatMap((f) => f.people)];
  const asks = everyone.filter((p) => p.state === "needs").map((p) => askRow(p, paneOf(p.id)!.task!.status));
  const at = rows.findIndex((r) => r.kind === "merge");
  const needs = at < 0 ? [...rows, ...asks] : [...rows.slice(0, at), ...asks, ...rows.slice(at)];

  return {
    founder, floors, needs,
    jobs: jobs.flatMap((j) => (GOING[j.status] ? [{ id: j.id, title: j.title, status: j.status, label: GOING[j.status]!, tokensUsed: j.tokensUsed, tokenCap: j.tokenCap }] : [])),
    flights: extra.flights ?? [],
    counts: { people: everyone.length, working: everyone.filter((p) => p.state === "working").length, needs: needs.length },
  };
}

/** What the scene draws, as one string: the same string means nothing to redraw. */
export function sceneKey(m: OfficeModel): string {
  const who = (p: OfficePerson) => [p.id, p.state, p.shirt, p.hair, p.prop, p.cli, p.seat, p.rank, !!p.printing].join(":");
  return JSON.stringify([m.founder && who(m.founder), m.floors.map((f) => [f.id, f.name, f.open, f.lit, f.needs, f.papers, f.board, f.people.map(who)]), m.flights.map((x) => x.id)]);
}
