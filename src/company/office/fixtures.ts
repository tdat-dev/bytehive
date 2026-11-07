/* A whole company in memory, for the demo page and the shell's tests. It goes through
 * officeModel like the real one, so the demo cannot drift from the model. */
import type { Board, Card } from "../../board";
import { emptyCompany } from "../org";
import { FOUNDER_ID, type Company, type Job, type JobPlan, type Person, type RoleDef } from "../types";
import type { Alert } from "../watch";
import { FLIGHT_MS } from "./layout";
import { officeModel, type BoardCounts, type OfficeModel, type OfficePane, type OfficeTask } from "./model";
import { BUILDING, clampLevel, rowTarget, type Level } from "./nav";
import { deskReport } from "./report";
import type { OfficeSource } from "./source";

export interface DemoOptions { empty?: boolean; quiet?: boolean; flight?: boolean; floors?: number; now?: number }

const role = (id: string, title: string, department: string, shirt: string, prop: string, rank: RoleDef["rank"] = "staff", cli = "claude"): RoleDef =>
  ({ id, title, department, rank, cli, denyTools: [], look: { shirt, hair: "#2a2e3a", prop }, prompt: "", skills: [] });

const ROLES: RoleDef[] = [
  role("founder", "Founder", "Management", "#f5b83d", "none", "founder"),
  role("lead", "Team Lead", "Management", "#5ec2f0", "clipboard", "lead"),
  role("developer", "Developer", "Engineering", "#5fd08a", "laptop"),
  role("designer", "Designer", "Design", "#ff6b6b", "pencil"),
  role("qa", "QA", "Engineering", "#b18cf0", "magnifier"),
  role("copywriter", "Copywriter", "Marketing", "#e6c84a", "pen"),
  role("marketing", "Marketing", "Marketing", "#e0913a", "megaphone"),
  role("researcher", "Researcher", "Research", "#3ad29f", "book"),
];

const FLOORS = ["bytehive", "gcare", "vanta", "oldsite", "scratch"];
const CLOSED = ["oldsite"];
/** People the extra floors of `?floors=8` are filled with, up to the company's limit. */
const EXTRAS = ["Nam", "Lan", "Quang", "Thao", "Vy", "Khoa", "Duc", "Ha", "Phuc", "Trang", "Long"];
const EXTRA_ROLES = ["developer", "designer", "qa", "researcher"];
const PER_EXTRA_FLOOR = 4;

const dirOf = (floor: string) => `D:\\demo\\${floor}`;

function hire(name: string, roleId: string, floor: string, over: Partial<Person> = {}): Person {
  const r = ROLES.find((x) => x.id === roleId)!;
  return { id: name.toLowerCase(), name, role: roleId, rank: r.rank, floor, boss: FOUNDER_ID, cli: "claude", look: { ...r.look }, ...over };
}

const task = (state: string, title: string, extra: object = {}): OfficeTask => ({ title, status: { state, ask: null, ...extra } }) as OfficeTask;
const pane = (personId: string, t: OfficeTask): OfficePane => ({ personId, wsId: "demo", paneId: `pane-${personId}`, task: t });
const asking = (title: string): OfficeTask => task("needs", title, { ask: { kind: "run", detail: "npm publish", prompt: "", options: [] } });

const job = (over: Partial<Job> & Pick<Job, "id" | "title" | "status">): Job =>
  ({ ask: over.title, plan: null, tokenCap: 5_000_000, tokensUsed: 1_200_000, tokenBase: {}, people: [], createdAt: 1, ...over });

const PLAN: JobPlan = {
  summary: "Split the work between the two teams.",
  hires: [{ role: "developer", floor: "bytehive" }, { role: "copywriter", floor: "gcare" }],
  cards: [
    { floor: "bytehive", title: "Add the price table", desc: "", checklist: [] },
    { floor: "bytehive", title: "Wire up the checkout", desc: "", checklist: [] },
    { floor: "gcare", title: "Write the launch post", desc: "", checklist: [] },
  ],
};

const card = (id: string, title: string, assigneeId: string, over: Partial<Card> = {}): Card =>
  ({ id, title, desc: "", labels: [], due: null, checklist: [], assigneeId, ...over });
const finished = (id: string, title: string, assigneeId: string, by: string, files: string[]): Card =>
  card(id, title, assigneeId, { job: "J-0141", done: { repoRoot: dirOf("bytehive"), files, summary: `${title} is done.`, at: 1, by } });

const BOARD: Board = {
  lists: [
    { id: "todo", title: "To do", cards: [card("c1", "Price table on mobile", "linh"), card("c2", "Empty state for the cart", "an")] },
    { id: "doing", title: "Doing", cards: [
      card("c3", "Checkout errors", "linh", { job: "J-0141", checklist: [{ id: "k1", text: "Card declined", done: true }, { id: "k2", text: "Network lost", done: false }] }),
      card("c4", "New icons", "an", { job: "J-0141" }),
    ] },
    { id: "done", title: "Done", cards: [
      finished("c5", "Login page", "linh", "Linh", ["src/login.ts"]),
      finished("c6", "Sign up page", "linh", "Linh", ["src/signup.ts"]),
      finished("c7", "Logo on the invoice", "an", "An", ["src/invoice.ts"]),
      finished("c8", "Test the refund path", "minh", "Minh", ["tests/refund.test.ts"]),
      finished("c9", "Plan the sprint", "hai", "Hai", []),
    ] },
  ],
};

const countOf = (b: Board, id: string) => b.lists.find((l) => l.id === id)?.cards.length ?? 0;
const BOARD_COUNTS: BoardCounts = { todo: countOf(BOARD, "todo"), doing: countOf(BOARD, "doing"), done: countOf(BOARD, "done") };

interface Demo {
  company: Company;
  jobs: Job[];
  alerts: Alert[];
  panes: OfficePane[];
  openDirs: string[];
  boards: Record<string, BoardCounts>;
  flight: boolean;
}

function extraFloors(from: number, to: number, already: number, limit: number): { ids: string[]; people: Person[] } {
  const ids: string[] = [];
  const people: Person[] = [];
  for (let n = from; n < to; n += 1) {
    const id = `project-${n + 1}`;
    ids.push(id);
    for (let i = 0; i < PER_EXTRA_FLOOR && already + people.length < limit; i += 1) {
      const k = people.length;
      people.push(hire(EXTRAS[k % EXTRAS.length], EXTRA_ROLES[k % EXTRA_ROLES.length], id));
    }
  }
  return { ids, people };
}

function demoState(o: DemoOptions): Demo {
  const base = emptyCompany();
  if (o.empty) return { company: base, jobs: [], alerts: [], panes: [pane(FOUNDER_ID, task("idle", ""))], openDirs: [], boards: {}, flight: false };

  const wanted = Math.max(0, o.floors ?? FLOORS.length);
  const staff = [
    hire("Hai", "lead", "bytehive"), hire("Linh", "developer", "bytehive"), hire("An", "designer", "bytehive"), hire("Minh", "qa", "bytehive"),
    hire("Mai", "lead", "gcare"), hire("Tuan", "copywriter", "gcare", { cli: "codex" }),
    hire("Sol", "researcher", "vanta", { cli: "opencode" }),
    hire("Bao", "marketing", "oldsite"),
  ];
  const kept = FLOORS.slice(0, wanted);
  const here = staff.filter((p) => kept.includes(p.floor!));
  const extra = extraFloors(FLOORS.length, wanted, here.length + 1, base.limits.peopleTotal);
  const ids = [...kept, ...extra.ids];
  const people = [...here, ...extra.people];
  const present = (id: string) => id === FOUNDER_ID || people.some((p) => p.id === id);

  const panes = [
    pane(FOUNDER_ID, task("working", "Planning J-0142")),
    pane("hai", o.quiet ? task("working", "Reading the plan") : asking("Publish the package")),
    pane("linh", task("working", "Checkout errors")),
    pane("an", task("working", "New icons")),
    pane("minh", task("review", "Test the refund path")),
    pane("mai", task("idle", "")),
    pane("tuan", task("working", "Launch post")),
    pane("sol", task("stopped", "")),
    ...extra.people.map((p, i) => pane(p.id, task(i % 3 === 2 ? "idle" : "working", "Project work"))),
  ].filter((p) => present(p.personId));

  const running = job({ id: "J-0141", title: "Ship the checkout", status: "running", people: ["linh", "an"] });
  const jobs = o.quiet ? [running] : [
    job({ id: "J-0142", title: "Add a pricing page", status: "awaiting-plan", plan: PLAN }),
    running,
    job({ id: "J-0139", title: "Rewrite the docs", status: "paused", tokensUsed: 5_000_000, people: ["tuan"] }),
    job({ id: "J-0138", title: "Fix the login page", status: "awaiting-merge", people: ["linh"], summary: "Login works on mobile again." }),
  ];
  const alerts: Alert[] = o.quiet || !present("tuan") ? [] : [{ key: "stall-tuan", job: "J-0141", personId: "tuan", text: "Stuck: Tuan has not moved \"Launch post\".", level: "warn" }];

  return {
    company: { ...base, floors: ids.map((id) => ({ id, dir: dirOf(id), name: id })), people: [...base.people, ...people], nextJob: 143 },
    jobs, alerts, panes,
    openDirs: ids.filter((id) => !CLOSED.includes(id)).map(dirOf),
    boards: { bytehive: BOARD_COUNTS },
    flight: !!o.flight,
  };
}

function modelOf(d: Demo, now: number): OfficeModel {
  const flights = d.flight ? [{ id: "demo-flight", from: "tuan", to: FOUNDER_ID, at: now - FLIGHT_MS / 2 }] : [];
  return officeModel(d.company, d.jobs, d.alerts, d.panes, { roles: ROLES, boards: d.boards, openDirs: d.openDirs, flights });
}

export const demoModel = (o: DemoOptions = {}): OfficeModel => modelOf(demoState(o), o.now ?? 0);

/** `building`, `floor:<id>` or `desk:<personId>`, as the demo's URL gives them. */
export function levelFrom(param: string | null, m: OfficeModel): Level {
  const [kind, id] = (param ?? "").split(":");
  if (kind === "floor" && id) return clampLevel({ kind: "floor", floorId: id }, m);
  if (kind === "desk" && id) return rowTarget({ key: "", kind: "alert", jobId: null, personId: id, title: "", detail: "" }, m);
  return BUILDING;
}

export function demoSource(o: DemoOptions = {}): OfficeSource {
  const d = demoState(o);
  const listeners = new Set<() => void>();
  const changed = () => { for (const fn of [...listeners]) fn(); };
  const update = async (id: string, over: Partial<Job>) => {
    d.jobs = d.jobs.map((j) => (j.id === id ? { ...j, ...over } : j));
    changed();
  };

  return {
    snapshot: (now) => ({ model: modelOf(d, now), note: null, error: null }),
    onChange(fn) {
      listeners.add(fn);
      return () => { listeners.delete(fn); };
    },
    async giveJob(ask) {
      const id = `J-${String(d.company.nextJob).padStart(4, "0")}`;
      d.company = { ...d.company, nextJob: d.company.nextJob + 1 };
      d.jobs = [job({ id, title: ask, ask, status: "planning", tokensUsed: 0 }), ...d.jobs];
      changed();
    },
    messageFloor: async () => {},
    approve: (id) => update(id, { status: "running" }),
    reject: (id, note) => update(id, { status: "rejected", summary: note }),
    raiseCap: (id, extra) => update(id, { status: "running", tokenCap: (d.jobs.find((j) => j.id === id)?.tokenCap ?? 0) + extra }),
    finishJob: (id) => update(id, { status: "done" }),
    stopJob: (id) => update(id, { status: "stopped" }),
    dismiss(key) {
      d.alerts = d.alerts.filter((a) => a.key !== key);
      changed();
    },
    showPerson: () => true,
    async wakePerson(id) {
      d.panes = [...d.panes.filter((p) => p.personId !== id), pane(id, task("idle", ""))];
      changed();
    },
    async report(personId) {
      const p = d.company.people.find((x) => x.id === personId);
      return p ? deskReport({ id: p.id, name: p.name, branch: p.branch ?? null }, BOARD, d.jobs) : null;
    },
    watching() {},
  };
}
