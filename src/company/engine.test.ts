// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Board } from "../board";
import type { AgentSpec, Pane, Workspace } from "../panetypes";
import type { JobPlan } from "./types";

const ROOT = "H:\\.bytehive\\company";
const io = vi.hoisted(() => ({
  files: new Map<string, string>(),
  boards: new Map<string, unknown>(),
  writeFails: [] as unknown[],
  readFails: new Map<string, unknown>(),
  sent: [] as Array<[string, string]>,
  told: [] as Array<{ name: string; text: string; delay: number; settle?: number }>,
  spawned: [] as string[],
  spawnFails: new Set<string>(),
  asking: false,
  starting: false,
  gate: null as Promise<void> | null,
  saveFails: false,
  usage: new Map<string, { fresh: number | null }>(),
  boardReads: [] as string[],
  onBoot: null as ((spec: AgentSpec) => void) | null,
  gone: new Set<string>(),
  sessions: new Set<string>(),
  node: "v22.13.0\r\n" as string | null,
  alive: new Set<number>(),
  lockFails: false,
  seedFails: false,
  made: [] as string[],
  closed: [] as Array<string | null>,
  excluded: [] as string[],
  excludeFails: false,
  nodeWait: 0,
}));

vi.mock("@tauri-apps/api/path", () => ({
  homeDir: async () => "H:\\",
  resolveResource: async (rel: string) => `C:\\Program Files\\ByteHive\\${rel.replace(/\//g, "\\")}`,
}));
vi.mock("../ipc", () => ({
  fsCreateDir: async (_root: string, rel: string) => { io.made.push(rel); },
  fsCreateFile: async (root: string, rel: string) => { if (!io.files.has(`${root}\\${rel}`)) io.files.set(`${root}\\${rel}`, ""); },
  fsWriteFile: async (root: string, rel: string, content: string) => {
    if (io.saveFails && rel === "company.json.tmp") throw { Failed: "access denied: company.json.tmp" };
    if (io.seedFails && rel.startsWith("roles\\")) throw { Failed: "disk full" };
    io.files.set(`${root}\\${rel}`, content);
    return { mtime: 1 };
  },
  fsRename: async (root: string, from: string, to: string) => {
    io.files.set(`${root}\\${to}`, io.files.get(`${root}\\${from}`)!);
    io.files.delete(`${root}\\${from}`);
  },
  fsReadFile: async (root: string, rel: string) => {
    if (io.lockFails && rel === "engine.lock") throw { Failed: "no such path: Access is denied. (os error 5)" };
    const content = io.files.get(`${root}\\${rel}`);
    if (content === undefined) throw { Failed: "no such path: The system cannot find the file specified. (os error 2)" };
    return { content, mtime: 1 };
  },
  claudeSessionExists: async (dir: string, id: string) => io.sessions.has(`${dir}|${id}`),
  fsReadDir: async (root: string, rel: string) => {
    if (io.gone.has(root)) throw { Failed: "bad root: gone" };
    const at = `${root}\\${rel}\\`;
    const names = [...io.files.keys()].filter((k) => k.startsWith(at)).map((k) => k.slice(at.length).split("\\"));
    return [...new Map(names.map((n) => [n[0], { name: n[0], is_dir: n.length > 1, size: 0 }])).values()];
  },
  fsReadLinesFrom: async (root: string, rel: string, offset: number | null) => {
    const text = io.files.get(`${root}\\${rel}`);
    if (text === undefined) throw { Failed: "no such path: gone" };
    const from = offset ?? text.length;
    return { text: text.slice(from), next: text.length, from, size: text.length };
  },
  fsTruncateIfSize: async () => false,
  gitExcludeBytehive: async (dir: string) => {
    if (io.excludeFails) throw { Failed: "git's exclude file: access denied" };
    io.excluded.push(dir);
  },
  appPid: async () => 7,
  processAlive: async (pid: number) => io.alive.has(pid),
  runCapture: async (program: string, args: string[], _cwd: string | null, timeoutMs: number) => {
    io.nodeWait = timeoutMs;
    if (![program, ...args].join(" ").endsWith("node --version") || io.node === null) throw { Failed: "node took longer than 30 s" };
    return io.node;
  },
  sendMessage: async (id: string, text: string) => { io.sent.push([id, text]); return true; },
}));
vi.mock("../agentcost", () => ({
  agentUsage: (pane: Pane) => io.usage.get(pane.spec.personId ?? "") ?? null,
}));
vi.mock("../boardfile", () => ({
  readBoardFile: async (dir: string) => {
    io.boardReads.push(dir);
    if (io.readFails.has(dir)) throw io.readFails.get(dir);
    const board = io.boards.get(dir);
    return board ? { board: structuredClone(board), mtime: 1 } : null;
  },
  writeBoardFile: async (dir: string, board: unknown) => {
    const fail = io.writeFails.shift();
    if (fail) throw fail;
    io.boards.set(dir, board);
    return 2;
  },
}));

vi.mock("../spawnmodal", () => ({
  cliLookOf: () => ({ color: "#fff", mono: "C" }),
  paneAsking: () => io.asking,
  paneAtPrompt: () => !io.starting,
  closeProject: async (ws: Workspace) => { io.closed.push(ws.dir); workspaces.delete(ws.id); },
  deliverTask: (_ws: unknown, names: string[], text: string, delay: number, _every?: number, _giveUp?: number, settle?: number) => {
    io.told.push({ name: names[0], text, delay, settle });
  },
  spawnPerson: async (dir: string, spec: { name: string; personId: string }) => {
    if (io.gate) await io.gate;
    if (io.spawnFails.has(spec.name)) throw new Error("no terminal");
    io.onBoot?.(spec as AgentSpec);
    io.spawned.push(spec.name);
    addPane(dir, spec, { running: true, spawnedAt: Date.now() });
  },
}));

function addPane(dir: string, spec: object, over: Partial<Pane>): Pane {
  let ws = [...workspaces.values()].find((w) => w.dir === dir);
  if (!ws) {
    ws = { id: dir, name: "x", dir, panes: new Map() } as unknown as Workspace;
    workspaces.set(dir, ws);
  }
  const pane = { id: `pane-${ws.panes.size}-${(spec as { name: string }).name}`, running: false, spawnedAt: null, spec, ...over } as unknown as Pane;
  ws.panes.set(pane.id, pane);
  return pane;
}

type Engine = typeof import("./engine");
let engine: Engine;
let workspaces: Map<string, Workspace>; // the engine's own: vi.resetModules gives each test a new one
/** initCompany leaves start in the queue: past this it has run. */
const started = () => engine.enqueue(async () => {});
const job = (n = 0) => engine.companyView()!.jobs[n];
const cards = (dir: string) => (io.boards.get(dir) as Board).lists.flatMap((l) => l.cards);
const planFor = (floor: string, hires = 0): JobPlan => ({
  summary: "s",
  cards: [{ floor, title: "Do it", desc: "d", checklist: [] }],
  hires: Array.from({ length: hires }, () => ({ role: "developer", floor })),
});
const request = async (line: object) => {
  const at = `${ROOT}\\requests.jsonl`;
  io.files.set(at, `${io.files.get(at) ?? ""}${JSON.stringify(line)}\n`);
  await vi.advanceTimersByTimeAsync(1500);
};

/** A card needs someone on its floor: whoever the plan doesn't hire already works there. */
function staffFloors(plan: JobPlan): void {
  const c = engine.companyView()!.company;
  for (const floor of new Set(plan.cards.map((x) => x.floor))) {
    if (plan.hires.some((h) => h.floor === floor)) continue;
    c.people.push({ id: `r-${floor}`, name: `Resident ${floor}`, role: "developer", rank: "staff", floor, boss: "founder", cli: "claude", look: { shirt: "#fff", hair: "#000", prop: "none" } });
  }
}

/** A company with floors for D:\a and D:\b, one Job given and its plan in. */
async function waitingJob(plan: JobPlan): Promise<void> {
  await engine.giveJob("do it");
  staffFloors(plan);
  await request({ type: "plan", ts: 1, from: "founder", job: job().id, plan });
  expect(job().status).toBe("awaiting-plan");
  io.told.length = 0;
  io.sent.length = 0;
}

const openProjects = () => {
  for (const dir of ["D:\\a", "D:\\b"]) workspaces.set(dir, { id: dir, name: dir.slice(-1), dir, panes: new Map() } as unknown as Workspace);
};

beforeEach(async () => {
  vi.useFakeTimers();
  vi.resetModules();
  ({ workspaces } = await import("../appstate"));
  io.files.clear();
  io.boards.clear();
  io.readFails.clear();
  io.spawnFails.clear();
  io.writeFails = [];
  io.sent = [];
  io.told = [];
  io.spawned = [];
  io.asking = false;
  io.starting = false;
  io.gate = null;
  io.saveFails = false;
  io.usage.clear();
  io.onBoot = null;
  io.gone.clear();
  io.sessions.clear();
  io.node = "v22.13.0\r\n";
  io.alive.clear();
  io.lockFails = false;
  io.seedFails = false;
  io.made = [];
  io.closed = [];
  io.excluded = [];
  io.excludeFails = false;
  openProjects();
  engine = await import("./engine");
  await engine.initCompany();
  await vi.advanceTimersByTimeAsync(1500); // the first tick notes that requests.jsonl isn't there yet
});
afterEach(() => { vi.useRealTimers(); });

describe("the MCP server a member gets", () => {
  it("is the one the app carries, in a config of their own", async () => {
    await engine.giveJob("do it");
    const at = `${ROOT}\\people\\founder\\mcp.json`;
    const text = io.files.get(at)!;
    const server = "C:\\Program Files\\ByteHive\\mcp-server\\bytehive-mcp.mjs";
    expect(JSON.parse(text)).toEqual({ mcpServers: { bytehive: { command: "node", args: [server] } } });
    expect(text).toContain(JSON.stringify(server)); // a Windows path, escaped, its space kept
    const pane = [...workspaces.values()].flatMap((w) => [...w.panes.values()]).find((p) => p.spec.personId === "founder")!;
    expect(pane.spec.args[pane.spec.args.indexOf("--mcp-config") + 1]).toBe(at);
    expect(pane.spec.args).not.toContain("--strict-mcp-config"); // the owner's other servers keep working
  });
});

describe("giving a job without a Node that can run the MCP server", () => {
  const NEEDS = "ByteHive needs Node.js 22.13 or newer to run the Company.";
  it("gives node 30 seconds", async () => {
    await engine.giveJob("do it");
    expect(io.nodeWait).toBe(30_000);
  });
  it.each([
    ["v22.12.9\r\n", `${NEEDS} Found v22.12.9.`],
    ["v18.20.0\r\n", `${NEEDS} Found v18.20.0.`],
    ["", `${NEEDS} Node.js was not found. If you just installed it, restart ByteHive.`],
  ])("says what it found and starts nothing, given %j", async (node, said) => {
    io.node = node;
    await expect(engine.giveJob("do it")).rejects.toThrow(said);
    expect(engine.companyView()!.jobs).toEqual([]);
    expect(io.spawned).toEqual([]);
  });
  it("does not block a job when the check itself fails, and says why in the log", async () => {
    const spy = vi.spyOn(console, "error").mockImplementation(() => {});
    io.node = null;
    await engine.giveJob("do it");
    expect(engine.companyView()!.jobs).toHaveLength(1);
    expect(spy).toHaveBeenCalledWith(expect.stringContaining("node took longer than 30 s"));
    spy.mockRestore();
  });
  it("looks again next time: Node may have been installed since", async () => {
    io.node = "";
    await engine.giveJob("do it").catch(() => {});
    io.node = "v24.1.0\n";
    await engine.giveJob("do it");
    expect(engine.companyView()!.jobs).toHaveLength(1);
  });
});

describe("starting the app", () => {
  it("brings everyone's prompt up to date: a session restarts on the file it was given", async () => {
    await waitingJob({ summary: "s", cards: [], hires: [{ role: "developer", floor: "a" }] });
    await engine.approve(job().id);
    const staff = engine.companyView()!.company.people.find((p) => p.rank === "staff")!;
    const file = `${ROOT}\\people\\${staff.id}\\prompt.md`;
    io.files.set(file, "laws from an older version");
    vi.resetModules();
    engine = await import("./engine");
    await engine.initCompany();
    await started();
    expect(io.files.get(file)).toContain(`Your name is ${staff.name}.`);
  });
  it("points everyone's MCP config at this app: a pane that restarts never gets a new one", async () => {
    await engine.giveJob("do it");
    const file = `${ROOT}\\people\\founder\\mcp.json`;
    io.files.set(file, "a server from another install");
    vi.resetModules();
    engine = await import("./engine");
    await engine.initCompany();
    await started();
    expect(io.files.get(file)).toContain("Program Files");
  });
});

describe("approve", () => {
  it("rewrites the prompt of someone a new Lead becomes boss of, so a restart reads the truth", async () => {
    await waitingJob({ summary: "s", cards: [], hires: [{ role: "developer", floor: "a" }] });
    await engine.approve(job().id);
    const staff = engine.companyView()!.company.people.find((p) => p.rank === "staff")!;
    await engine.giveJob("a lead");
    await request({ type: "plan", ts: 1, from: "founder", job: job(1).id, plan: { summary: "s", cards: [], hires: [{ role: "lead", floor: "a" }] } });
    await engine.approve(job(1).id);
    const lead = engine.companyView()!.company.people.find((p) => p.rank === "lead")!;
    expect(io.files.get(`${ROOT}\\people\\${staff.id}\\prompt.md`)).toContain(`Your boss is ${lead.name}.`);
  });

  it("places the cards once, however many clicks", async () => {
    await waitingJob(planFor("a"));
    await Promise.all([engine.approve(job().id), engine.approve(job().id)]);
    await engine.approve(job().id);
    expect(cards("D:\\a").length).toBe(1);
    expect(job().status).toBe("running");
  });

  it("does nothing to a job that isn't waiting", async () => {
    await engine.giveJob("do it");
    await engine.approve(job().id);
    expect(job().status).toBe("planning");
    expect(io.boards.size).toBe(0);
  });

  it("tries the board again on a Conflict, on a fresh copy", async () => {
    await waitingJob(planFor("a"));
    io.writeFails = [{ Conflict: 5 }, { Conflict: 6 }];
    await engine.approve(job().id);
    expect(cards("D:\\a").length).toBe(1);
    expect(io.told.concat(io.sent.map(([, text]) => ({ name: "", text, delay: 0 }))).every((m) => !/went wrong/.test(m.text))).toBe(true);
  });

  it("gives up after three Conflicts and tells the Founder, who still gets the rest", async () => {
    await waitingJob({ ...planFor("a"), cards: [...planFor("a").cards, ...planFor("b").cards] });
    io.writeFails = [{ Conflict: 1 }, { Conflict: 2 }, { Conflict: 3 }];
    await engine.approve(job().id);
    expect(job().status).toBe("running");
    expect(cards("D:\\b").length).toBe(1);
    const said = [...io.told.map((m) => m.text), ...io.sent.map(([, t]) => t)].join();
    expect(said).toContain("approved");
    expect(said).toContain("Cards for a");
  });

  it("gets past a corrupt board and a hire that won't start", async () => {
    await waitingJob({ ...planFor("a", 1), cards: [...planFor("a").cards, ...planFor("b").cards] });
    io.readFails.set("D:\\a", new Error("board.json is not valid JSON"));
    io.spawnFails.add("Ana");
    await engine.approve(job().id);
    expect(cards("D:\\b").length).toBe(1);
    const said = [...io.told.map((m) => m.text), ...io.sent.map(([, t]) => t)].join();
    expect(said).toContain("Cards for a");
    expect(said).toContain("Ana did not start: no terminal");
  });

  it("matches a card's assignee whatever the case", async () => {
    const plan = planFor("a");
    await waitingJob({ ...plan, cards: [{ ...plan.cards[0], assignee: "founder" }] });
    await engine.approve(job().id);
    expect(cards("D:\\a")[0]).toMatchObject({ assignee: "Founder", assigneeId: "founder" });
  });

  it("places nothing when the plan no longer holds, and says why", async () => {
    await waitingJob(planFor("a", 1));
    const c = engine.companyView()!.company;
    c.limits = { ...c.limits, peopleTotal: 1 };
    await engine.approve(job().id);
    expect(job().status).toBe("planning");
    expect(io.boards.size).toBe(0);
    expect(io.spawned).toEqual(["Founder"]);
  });
});

describe("a role the plan drafts", () => {
  const file = (name: string) => io.files.get(`${ROOT}\\roles\\analyst\\${name}`);

  it("is not on disk until the owner approves, and then the hire runs on it", async () => {
    await waitingJob(drafting());
    expect(file("role.json")).toBeUndefined();
    await engine.approve(job().id);
    expect(JSON.parse(file("role.json")!)).toMatchObject({ id: "analyst", rank: "staff", cli: "claude", denyTools: [] });
    expect(file("prompt.md")).toBe(analyst.prompt);
    const hire = engine.companyView()!.company.people.find((p) => p.role === "analyst")!;
    expect(io.spawned).toContain(hire.name);
    expect(io.files.get(`${ROOT}\\people\\${hire.id}\\prompt.md`)).toContain(analyst.prompt);
  });

  it("is never saved when the plan is sent back", async () => {
    await waitingJob(drafting());
    await engine.reject(job().id, "no");
    expect(file("role.json")).toBeUndefined();
    expect(engine.companyView()!.roles.map((r) => r.id)).not.toContain("analyst");
  });
});

describe("reject", () => {
  it("leaves a running job alone", async () => {
    await waitingJob(planFor("a"));
    await engine.approve(job().id);
    io.told.length = 0;
    io.sent.length = 0;
    await engine.reject(job().id, "no");
    expect(job().status).toBe("running");
    expect([...io.told, ...io.sent]).toEqual([]);
  });
});

describe("the queue", () => {
  it("starts one Founder when two jobs arrive together", async () => {
    let open = () => {};
    io.gate = new Promise((r) => { open = r; });
    const both = Promise.all([engine.giveJob("one"), engine.giveJob("two")]);
    await vi.advanceTimersByTimeAsync(0);
    open();
    await both;
    expect(io.spawned).toEqual(["Founder"]);
    expect(engine.companyView()!.jobs.map((j) => j.id)).toEqual(["J-0001", "J-0002"]);
  });

  it("runs operations one at a time, in order, and survives a failure", async () => {
    const seen: string[] = [];
    let open = () => {};
    const first = engine.enqueue(async () => { seen.push("a"); await new Promise<void>((r) => { open = r; }); seen.push("a done"); });
    const second = engine.enqueue(async () => { seen.push("b"); throw new Error("boom"); });
    const third = engine.enqueue(async () => { seen.push("c"); return 3; });
    await vi.advanceTimersByTimeAsync(0);
    expect(seen).toEqual(["a"]);
    open();
    await first;
    await expect(second).rejects.toThrow("boom");
    expect(await third).toBe(3);
    expect(seen).toEqual(["a", "a done", "b", "c"]);
  });

  it("writes nothing for a request that changes nothing", async () => {
    await engine.giveJob("do it");
    const before = io.files.get(`${ROOT}\\jobs\\J-0001.json`);
    io.files.set(`${ROOT}\\jobs\\J-0001.json`, "untouched");
    await request({ type: "message", ts: 1, from: "founder", to: "Nobody", message: "hi" });
    expect(io.files.get(`${ROOT}\\jobs\\J-0001.json`)).toBe("untouched");
    expect(before).toContain("J-0001");
  });

  it("keeps reading after a request that throws", async () => {
    await engine.giveJob("do it");
    await request({ type: "plan", ts: 1, from: "founder", job: "J-0001" }); // no plan: validatePlan throws
    staffFloors(planFor("a"));
    await request({ type: "plan", ts: 2, from: "founder", job: "J-0001", plan: planFor("a") });
    expect(job().status).toBe("awaiting-plan");
  });
});

describe("delivery", () => {
  const founderPane = (over: Partial<Pane>) => addPane("D:\\a", { name: "Founder", personId: "founder" }, over);

  it("waits out the boot of someone it just started", async () => {
    await engine.giveJob("do it");
    expect(io.sent).toEqual([]);
    expect(io.told).toEqual([{ name: "Founder", text: expect.stringContaining("JOB J-0001"), delay: 8000, settle: 8000 }]);
  });

  it("types straight in once they are at their prompt", async () => {
    const pane = founderPane({ running: true, spawnedAt: Date.now() - 60_000 });
    await engine.giveJob("do it");
    expect(io.sent).toEqual([[pane.id, expect.stringContaining("JOB J-0001")]]);
  });

  it("waits while a question is on screen, however long they have been up", async () => {
    founderPane({ running: true, spawnedAt: Date.now() - 60_000 });
    io.asking = true;
    await engine.giveJob("do it");
    expect(io.sent).toEqual([]);
    expect(io.told).toEqual([expect.objectContaining({ delay: 0, settle: 8000 })]);
  });

  it("waits for one who is up but not at their prompt yet: a resume can outlast the head start", async () => {
    founderPane({ running: true, spawnedAt: Date.now() - 60_000 });
    io.starting = true;
    await engine.giveJob("do it");
    expect(io.sent).toEqual([]);
    expect(io.told).toEqual([expect.objectContaining({ delay: 0, settle: 8000 })]);
  });

  it("wakes a stopped person", async () => {
    const restart = vi.fn(async () => { pane.running = true; pane.spawnedAt = Date.now(); });
    const pane = founderPane({ running: false, restart });
    await engine.giveJob("do it");
    expect(restart).toHaveBeenCalledOnce();
    expect(io.told).toEqual([expect.objectContaining({ delay: 8000 })]);
  });

  it("does not boot a pane that is already booting", async () => {
    const restart = vi.fn(async () => {});
    founderPane({ running: false, spawning: true, restart });
    await engine.giveJob("do it");
    expect(restart).not.toHaveBeenCalled();
    expect(io.told).toEqual([expect.objectContaining({ delay: 8000 })]);
  });
});

describe("where the Founder sits", () => {
  const OFFICE = `${ROOT}\\office`;
  const founderPanes = () => [...workspaces.values()].flatMap((w) => [...w.panes.values()].filter((p) => p.spec.personId === "founder").map((p) => ({ dir: w.dir, spec: p.spec })));
  const activate = async (dir: string | null) => {
    const { setActiveWs } = await import("../appstate");
    setActiveWs(dir ? workspaces.get(dir)! : null);
  };
  const noFolder = () => workspaces.set("loose", { id: "loose", name: "Project 1", dir: null, panes: new Map() } as unknown as Workspace);
  const alertKeys = () => engine.companyAlerts().map((a) => a.key);

  it("sits in the active project, in its agent list", async () => {
    await activate("D:\\b");
    await engine.giveJob("do it");
    expect(founderPanes().map((f) => f.dir)).toEqual(["D:\\b"]);
    expect(founderPanes()[0].spec.cwd).toBe("D:\\b");
  });

  it("opens no tab and makes no folder of its own", async () => {
    await engine.giveJob("do it");
    expect([...workspaces.values()].map((w) => w.dir)).toEqual(["D:\\a", "D:\\b"]);
    expect(io.made).not.toContain("office");
  });

  it("takes the first project with a folder when the active one has none", async () => {
    noFolder();
    await activate("loose");
    await engine.giveJob("do it");
    expect(founderPanes().map((f) => f.dir)).toEqual(["D:\\a"]);
  });

  it("takes the first project with a folder from Home", async () => {
    await activate(null);
    await engine.giveJob("do it");
    expect(founderPanes().map((f) => f.dir)).toEqual(["D:\\a"]);
  });

  it.each([["nothing is open", false], ["no project has a folder", true]])("refuses a job when %s", async (_what, loose) => {
    workspaces.clear();
    if (loose) noFolder();
    await expect(engine.giveJob("do it")).rejects.toThrow("Open a project first.");
    expect(engine.companyView()!.jobs).toEqual([]);
    expect(io.spawned).toEqual([]);
  });

  it("stays where it is for later jobs, whichever project is active", async () => {
    await activate("D:\\b");
    await engine.giveJob("one");
    await activate("D:\\a");
    await engine.giveJob("two");
    expect(founderPanes().map((f) => f.dir)).toEqual(["D:\\b"]);
    expect(io.spawned).toEqual(["Founder"]);
  });

  it("starts blank in the active project when its project closed, without calling it lost", async () => {
    await activate("D:\\b");
    await engine.giveJob("one");
    engine.companyView()!.company.people[0].session = "sess-b";
    workspaces.get("D:\\b")!.panes.clear();
    await activate("D:\\a");
    await engine.giveJob("two");
    expect(founderPanes().map((f) => f.dir)).toEqual(["D:\\a"]);
    expect(founderPanes()[0].spec.sessionId).toBeUndefined();
    expect(alertKeys()).not.toContain("lost-founder");
  });

  it("carries on its conversation when it comes back to the same folder", async () => {
    await engine.giveJob("one");
    engine.companyView()!.company.people[0].session = "sess-a";
    workspaces.get("D:\\a")!.panes.clear();
    io.sessions.add("D:\\a|sess-a");
    await engine.giveJob("two");
    expect(founderPanes()[0].spec.sessionId).toBe("sess-a");
  });

  it("does not make its project a floor with people: the Conductor keeps working there", async () => {
    await engine.giveJob("do it");
    expect(engine.isCompanyFloor("D:\\a")).toBe(false);
    expect(engine.companyView()!.company.people[0]).toMatchObject({ rank: "founder", floor: null, boss: null });
  });

  it("starts in a project when a message reaches it with no pane", async () => {
    await engine.giveJob("do it");
    workspaces.get("D:\\a")!.panes.clear();
    staffFloors(planFor("b"));
    await request({ type: "message", ts: 1, from: "r-b", to: "Founder", message: "done" });
    expect(founderPanes().map((f) => f.dir)).toEqual(["D:\\a"]);
    expect(io.told[io.told.length - 1]).toMatchObject({ name: "Founder", text: expect.stringContaining("done") });
  });

  it("raises an alert for a message it cannot get with no project open, and keeps reading", async () => {
    await engine.giveJob("do it");
    staffFloors(planFor("b"));
    workspaces.clear();
    await request({ type: "message", ts: 1, from: "r-b", to: "Founder", message: "done" });
    expect(alertKeys()).toContain("founder-no-project");
    workspaces.set("D:\\a", { id: "D:\\a", name: "a", dir: "D:\\a", panes: new Map() } as unknown as Workspace);
    await request({ type: "message", ts: 2, from: "r-b", to: "Founder", message: "again" });
    expect(founderPanes().map((f) => f.dir)).toEqual(["D:\\a"]);
    expect(alertKeys()).not.toContain("founder-no-project");
  });

  describe("after an update from 0.14.0", () => {
    /** The app comes back with the tabs it saved: the old office tab, the Founder stopped in it. */
    async function upgrade() {
      vi.resetModules();
      ({ workspaces } = await import("../appstate"));
      openProjects();
      addPane(OFFICE, { name: "Founder", personId: "founder", cwd: OFFICE, sessionId: "sess-office" }, {});
      engine = await import("./engine");
      await engine.initCompany();
      await started();
    }

    it("closes the office tab and leaves the folder on disk", async () => {
      io.files.set(`${OFFICE}\\.keep`, "");
      await upgrade();
      expect(io.closed).toEqual([OFFICE]);
      expect([...workspaces.values()].map((w) => w.dir)).toEqual(["D:\\a", "D:\\b"]);
      expect(founderPanes()).toEqual([]);
      expect(io.files.has(`${OFFICE}\\.keep`)).toBe(true);
    });

    it("does not make the office a floor", async () => {
      await upgrade();
      expect(engine.companyView()!.company.floors.map((f) => f.dir)).not.toContain(OFFICE);
    });

    it("starts the Founder fresh in a project on the next job", async () => {
      await engine.giveJob("one");
      engine.companyView()!.company.people[0].session = "sess-office";
      await engine.giveJob("two"); // saves the company with that session
      await upgrade();
      expect(engine.companyView()!.company.people[0].session).toBe("sess-office");
      io.sessions.add(`${OFFICE}|sess-office`);
      await engine.giveJob("three");
      expect(founderPanes().map((f) => f.dir)).toEqual(["D:\\a"]);
      expect(founderPanes()[0].spec.sessionId).toBeUndefined();
      expect(alertKeys()).not.toContain("lost-founder");
    });

    it("leaves a running Job, its staff and its cards as they are", async () => {
      await waitingJob({ ...planFor("a"), hires: [{ role: "developer", floor: "a" }] });
      await engine.approve(job().id);
      const before = { people: engine.companyView()!.company.people, jobs: engine.companyView()!.jobs, board: structuredClone(io.boards.get("D:\\a")) };
      await upgrade();
      const after = engine.companyView()!;
      expect(after.jobs.map((j) => [j.id, j.status])).toEqual(before.jobs.map((j) => [j.id, j.status]));
      expect(after.jobs[0].status).toBe("running");
      expect(after.company.people.map((p) => p.id)).toEqual(before.people.map((p) => p.id));
      expect(io.boards.get("D:\\a")).toEqual(before.board);
    });
  });
});

describe("keeping git status clean", () => {
  const openProject = (dir: string) => workspaces.set(dir, { id: dir, name: dir.slice(-1), dir, panes: new Map() } as unknown as Workspace);

  it("excludes .bytehive/ in every project that is a floor", () => {
    expect(io.excluded).toEqual(["D:\\a", "D:\\b"]);
  });

  it("does it when a project becomes a floor", async () => {
    openProject("D:\\c");
    await engine.giveJob("do it");
    expect(io.excluded).toContain("D:\\c");
  });

  it("asks once per folder", async () => {
    await engine.giveJob("one");
    await engine.giveJob("two");
    expect(io.excluded.filter((d) => d === "D:\\a")).toHaveLength(1);
  });

  it("does it where the Founder starts, floor or not", async () => {
    await engine.giveJob("do it");
    staffFloors(planFor("b"));
    workspaces.clear();
    openProject("D:\\c");
    await request({ type: "message", ts: 1, from: "r-b", to: "Founder", message: "done" });
    expect(io.excluded).toContain("D:\\c");
    expect(engine.companyView()!.company.floors.map((f) => f.dir)).not.toContain("D:\\c");
  });

  it("never blocks a job, says why, and tries again", async () => {
    const spy = vi.spyOn(console, "error").mockImplementation(() => {});
    openProject("D:\\c");
    io.excludeFails = true;
    await engine.giveJob("one");
    expect(engine.companyView()!.jobs).toHaveLength(1);
    expect(spy).toHaveBeenCalledWith(expect.stringContaining("access denied"));
    spy.mockRestore();
    io.excludeFails = false;
    await engine.giveJob("two");
    expect(io.excluded).toContain("D:\\c");
  });
});

describe("hooks", () => {
  const SETTINGS = `${ROOT}\\people\\founder\\settings.json`;
  const paneOf = (id: string) => [...workspaces.values()].flatMap((w) => [...w.panes.values()]).find((p) => p.spec.personId === id)!;

  it("are off for the Founder, through a settings file of its own", async () => {
    await engine.giveJob("do it");
    expect(JSON.parse(io.files.get(SETTINGS)!)).toEqual({ disableAllHooks: true });
    const { args } = paneOf("founder").spec;
    expect(args[args.indexOf("--settings") + 1]).toBe(SETTINGS);
  });

  it("stay on for everyone else", async () => {
    await waitingJob({ summary: "s", cards: [], hires: [{ role: "developer", floor: "a" }] });
    await engine.approve(job().id);
    const staff = engine.companyView()!.company.people.find((p) => p.rank === "staff")!;
    expect(paneOf(staff.id).spec.args).not.toContain("--settings");
    expect(io.files.has(`${ROOT}\\people\\${staff.id}\\settings.json`)).toBe(false);
  });

  it("stay off after a restart: a pane restarts on the file it was given", async () => {
    await engine.giveJob("do it");
    io.files.delete(SETTINGS);
    vi.resetModules();
    engine = await import("./engine");
    await engine.initCompany();
    await started();
    expect(JSON.parse(io.files.get(SETTINGS)!)).toEqual({ disableAllHooks: true });
  });
});

describe("floors", () => {
  it("gives two folders of the same name their own floors", async () => {
    workspaces.set("D:\\x\\a", { id: "x", name: "a", dir: "D:\\x\\a", panes: new Map() } as unknown as Workspace);
    await engine.giveJob("do it");
    expect(engine.companyView()!.company.floors.map((f) => f.id)).toEqual(["a", "b", "a-2"]);
  });
});

describe("a detached window", () => {
  afterEach(() => { vi.unstubAllGlobals(); });
  it("says the Company runs in the main window", () => {
    vi.stubGlobal("location", { search: "?detach=abc" });
    expect(engine.companyElsewhere()).toBe("The Company runs in the main window.");
  });
  it("has nothing to say in the main window", () => {
    expect(engine.companyElsewhere()).toBeNull();
  });
});

describe("two running apps", () => {
  const LOCK = `${ROOT}\\engine.lock`;
  const lock = () => JSON.parse(io.files.get(LOCK)!) as { pid: number; at: number };
  const otherApp = (at = Date.now()) => io.files.set(LOCK, JSON.stringify({ pid: 99, at }));
  /** A second app opens while another one runs the company. */
  async function secondApp() {
    otherApp();
    vi.resetModules();
    engine = await import("./engine");
    await engine.initCompany();
    await started();
  }
  const ELSEWHERE = "The Company is running in another ByteHive window.";

  it("keeps the lock fresh while it runs the company", async () => {
    expect(lock().pid).toBe(7);
    const first = lock().at;
    await vi.advanceTimersByTimeAsync(4500);
    expect(lock().at).toBeGreaterThan(first);
  });

  it("keeps the lock fresh through a long operation", async () => {
    let open = () => {};
    const long = engine.enqueue(() => new Promise<void>((r) => { open = r; }));
    const first = lock().at;
    await vi.advanceTimersByTimeAsync(20_000);
    expect(Date.now() - lock().at).toBeLessThan(3000);
    expect(lock().at).toBeGreaterThan(first);
    open();
    await long;
  });

  it("keeps the lock fresh while its start waits behind a long operation", async () => {
    otherApp();
    await vi.advanceTimersByTimeAsync(1500);
    expect(engine.companyView()).toBeNull();
    let open = () => {};
    const long = engine.enqueue(() => new Promise<void>((r) => { open = r; }));
    await vi.advanceTimersByTimeAsync(16_500); // the other app is gone: this one takes the lock, and start queues
    expect(lock().pid).toBe(7);
    await vi.advanceTimersByTimeAsync(20_000);
    expect(Date.now() - lock().at).toBeLessThan(3000);
    open();
    await long;
    await started();
    expect(engine.companyView()).not.toBeNull();
  });

  it("says why it could not start, and keeps the heartbeat going", async () => {
    io.files.clear();
    io.seedFails = true;
    vi.resetModules();
    engine = await import("./engine");
    await engine.initCompany();
    await started();
    expect(engine.companyError()).toContain("disk full");
    expect(engine.companyView()).toBeNull();
    const first = lock().at;
    await vi.advanceTimersByTimeAsync(4500);
    expect(lock().at).toBeGreaterThan(first);
  });

  it("stays read-only beside another app: no tick, no watch, no writes", async () => {
    for (const k of [...io.files.keys()]) if (k !== LOCK) io.files.delete(k);
    io.files.set(`${ROOT}\\requests.jsonl`, "");
    await secondApp();
    await engine.giveJob("do it");
    for (let i = 0; i < 6; i += 1) {
      otherApp();
      await vi.advanceTimersByTimeAsync(1500);
    }
    await engine.approve("J-0001");
    expect(engine.companyView()).toBeNull();
    expect(engine.companyElsewhere()).toBe(ELSEWHERE);
    expect(io.spawned).toEqual([]);
    expect(lock().pid).toBe(99);
    expect([...io.files.keys()].sort()).toEqual([LOCK, `${ROOT}\\requests.jsonl`].sort());
  });

  it("takes over the lock of a dead app after 15 seconds with no heartbeat", async () => {
    await secondApp();
    expect(engine.companyView()).toBeNull();
    await vi.advanceTimersByTimeAsync(16_500);
    expect(engine.companyView()).not.toBeNull();
    expect(engine.companyElsewhere()).toBeNull();
    expect(lock().pid).toBe(7);
  });

  it("starts at once on a lock that is stale already", async () => {
    otherApp(Date.now() - 20_000);
    vi.resetModules();
    engine = await import("./engine");
    await engine.initCompany();
    await started();
    expect(engine.companyView()).not.toBeNull();
    expect(lock().pid).toBe(7);
  });

  it("carries on after a window reload: the fresh lock is its own", async () => {
    vi.resetModules();
    engine = await import("./engine");
    await engine.initCompany();
    await started();
    expect(engine.companyView()).not.toBeNull();
  });

  it("steps down when another app holds the lock, and picks up from the files later", async () => {
    await engine.giveJob("do it");
    otherApp();
    await vi.advanceTimersByTimeAsync(1500);
    expect(engine.companyView()).toBeNull();
    expect(engine.companyElsewhere()).toBe(ELSEWHERE);
    await vi.advanceTimersByTimeAsync(16_500);
    expect(engine.companyView()!.jobs.map((j) => j.id)).toEqual(["J-0001"]);
  });

  describe("beside an app that is alive but silent", () => {
    const MIN = 60_000;
    const TAKEOVER = /process 99.*stopped answering.*took over the Company/;
    const standing = () => engine.companyAlerts().filter((a) => a.key === "lock-takeover");

    it("stays read-only after a minute, and starts nobody", async () => {
      await secondApp();
      io.alive.add(99);
      await vi.advanceTimersByTimeAsync(MIN);
      expect(engine.companyView()).toBeNull();
      expect(engine.companyElsewhere()).toBe(ELSEWHERE);
      expect(lock().pid).toBe(99);
    });
    it("takes over after 10 minutes without a heartbeat, and says what happened", async () => {
      otherApp();
      io.alive.add(99);
      await vi.advanceTimersByTimeAsync(9 * MIN);
      expect(engine.companyView()).toBeNull();
      await vi.advanceTimersByTimeAsync(MIN + 1500);
      expect(engine.companyView()).not.toBeNull();
      expect(lock().pid).toBe(7);
      expect(standing()).toEqual([expect.objectContaining({ level: "warn", text: expect.stringMatching(TAKEOVER) })]);
    });
    it("takes over at once from a lock silent that long already, and says so", async () => {
      otherApp(Date.now() - 11 * MIN);
      io.alive.add(99);
      vi.resetModules();
      engine = await import("./engine");
      await engine.initCompany();
      await started();
      expect(engine.companyView()).not.toBeNull();
      expect(standing()).toHaveLength(1);
    });
    it("leaves a live app be after the machine slept: nobody starts twice", async () => {
      await secondApp();
      io.alive.add(99);
      vi.setSystemTime(Date.now() + 8 * 60 * MIN); // no timer runs in a sleep
      await vi.advanceTimersByTimeAsync(1500);
      expect(engine.companyView()).toBeNull();
      expect(lock().pid).toBe(99);
      expect(standing()).toEqual([]);
      otherApp(); // the holder is awake too
      await vi.advanceTimersByTimeAsync(1500);
      expect(engine.companyElsewhere()).toBe(ELSEWHERE);
    });
    it("leaves a live app be when the clock is moved back: nobody starts twice", async () => {
      otherApp();
      io.alive.add(99);
      await vi.advanceTimersByTimeAsync(1500);
      vi.setSystemTime(Date.now() - 11 * MIN);
      await vi.advanceTimersByTimeAsync(3000);
      expect(engine.companyView()).toBeNull();
      expect(lock().pid).toBe(99);
      expect(standing()).toEqual([]);
    });
    it("leaves a live app be after a gap just under 10 minutes", async () => {
      otherApp();
      io.alive.add(99);
      await vi.advanceTimersByTimeAsync(1500);
      vi.setSystemTime(Date.now() + 10 * MIN - 2000);
      await vi.advanceTimersByTimeAsync(4500);
      expect(engine.companyView()).toBeNull();
      expect(lock().pid).toBe(99);
      expect(standing()).toEqual([]);
    });
    it("still takes over from a stuck app, once its heartbeat is 10 minutes old by the moved clock", async () => {
      otherApp();
      io.alive.add(99);
      await vi.advanceTimersByTimeAsync(1500);
      vi.setSystemTime(Date.now() - 11 * MIN);
      await vi.advanceTimersByTimeAsync(20 * MIN);
      expect(engine.companyView()).toBeNull();
      await vi.advanceTimersByTimeAsync(MIN + 3000);
      expect(engine.companyView()).not.toBeNull();
      expect(standing()).toHaveLength(1);
    });
    it("raises no alert when the holder is simply dead", async () => {
      await secondApp();
      await vi.advanceTimersByTimeAsync(16_500);
      expect(engine.companyView()).not.toBeNull();
      expect(standing()).toEqual([]);
    });
  });

  describe("on a lock it cannot read", () => {
    it("never takes over on that one failed read", async () => {
      await secondApp();
      await vi.advanceTimersByTimeAsync(1500);
      io.lockFails = true;
      await vi.advanceTimersByTimeAsync(20_000);
      expect(engine.companyView()).toBeNull();
      expect(lock().pid).toBe(99);
    });
    it("keeps running what it runs, and writes no heartbeat until it can read again", async () => {
      await engine.giveJob("do it");
      const before = lock().at;
      io.lockFails = true;
      await vi.advanceTimersByTimeAsync(4500);
      expect(engine.companyView()).not.toBeNull();
      expect(lock().at).toBe(before);
      io.lockFails = false;
      await vi.advanceTimersByTimeAsync(1500);
      expect(lock().at).toBeGreaterThan(before);
    });
  });

  it("tells the screen when it steps down", async () => {
    const seen = vi.fn();
    engine.onCompanyChange(seen);
    otherApp();
    await vi.advanceTimersByTimeAsync(1500);
    expect(seen).toHaveBeenCalled();
  });
});

describe("a person whose pane closes", () => {
  const SESSION = "11111111-2222-3333-4444-555555555555";
  const TREE = "D:\\wt\\ana";
  /** Ana hired on floor a, booted into a worktree and a session. */
  async function hired() {
    io.onBoot = (spec) => { if (spec.name === "Ana") Object.assign(spec, { sessionId: SESSION, worktree: TREE, branch: "bytehive/ana" }); };
    await waitingJob({ summary: "s", cards: [], hires: [{ role: "developer", floor: "a" }] });
    await engine.approve(job().id);
    io.onBoot = null;
    return engine.companyView()!.company.people.find((p) => p.name === "Ana")!;
  }
  const paneOf = (id: string) => [...workspaces.values()].flatMap((w) => [...w.panes.values()]).find((p) => p.spec.personId === id);
  const close = (id: string) => { for (const ws of workspaces.values()) for (const [k, p] of ws.panes) if (p.spec.personId === id) ws.panes.delete(k); };
  const tell = (name: string) => request({ type: "message", ts: 9, from: "founder", to: name, message: "carry on" });
  const onDisk = (id: string) => (JSON.parse(io.files.get(`${ROOT}\\company.json`)!) as { people: Array<{ id: string }> }).people.find((p) => p.id === id);

  it("has their session, worktree and branch written down at boot", async () => {
    const ana = await hired();
    expect(ana).toMatchObject({ session: SESSION, worktree: TREE, branch: "bytehive/ana" });
    expect(onDisk(ana.id)).toMatchObject({ session: SESSION, worktree: TREE, branch: "bytehive/ana" });
  });

  it("follows a session that moves on, like after /clear", async () => {
    const ana = await hired();
    paneOf(ana.id)!.spec.sessionId = "22222222-2222-3333-4444-555555555555";
    await vi.advanceTimersByTimeAsync(30_000);
    expect(onDisk(ana.id)).toMatchObject({ session: "22222222-2222-3333-4444-555555555555" });
  });

  it("comes back on them, with nothing to tell the owner", async () => {
    const ana = await hired();
    close(ana.id);
    io.sessions.add(`${TREE}|${SESSION}`);
    await tell("Ana");
    expect(paneOf(ana.id)!.spec).toMatchObject({ sessionId: SESSION, worktree: TREE, branch: "bytehive/ana" });
    expect(engine.companyAlerts()).toEqual([]);
  });

  it("raises an alert when the session is gone, and keeps the worktree", async () => {
    const ana = await hired();
    close(ana.id);
    await tell("Ana");
    const spec = paneOf(ana.id)!.spec;
    expect(spec.sessionId).toBeUndefined();
    expect(spec.worktree).toBe(TREE);
    expect(engine.companyAlerts()).toEqual([expect.objectContaining({ personId: ana.id, text: expect.stringMatching(/Ana.*conversation.*blank/), standing: true })]);
  });

  it("raises an alert when the worktree is gone, and the session goes with it", async () => {
    const ana = await hired();
    close(ana.id);
    io.gone.add(TREE);
    await tell("Ana");
    const spec = paneOf(ana.id)!.spec;
    expect(spec.worktree).toBeUndefined();
    expect(spec.branch).toBeUndefined();
    expect(spec.sessionId).toBeUndefined();
    expect(engine.companyAlerts()).toEqual([expect.objectContaining({ personId: ana.id, text: expect.stringMatching(/Ana.*worktree.*conversation.*blank/) })]);
  });

  it("names the branch of a lost worktree, so the commits on it can be found", async () => {
    const ana = await hired();
    close(ana.id);
    io.gone.add(TREE);
    await tell("Ana");
    expect(engine.companyAlerts()[0].text).toContain("bytehive/ana");
  });

  it("names no branch when only the conversation is gone: the worktree is still there", async () => {
    const ana = await hired();
    close(ana.id);
    await tell("Ana");
    expect(engine.companyAlerts()[0].text).not.toContain("bytehive/ana");
  });

  it("writes down what they start on instead, so the next close finds it", async () => {
    const ana = await hired();
    close(ana.id);
    io.gone.add(TREE);
    io.onBoot = (spec) => { Object.assign(spec, { sessionId: "33333333-2222-3333-4444-555555555555", worktree: "D:\\wt\\new", branch: "bytehive/ana-2" }); };
    await tell("Ana");
    expect(onDisk(ana.id)).toMatchObject({ session: "33333333-2222-3333-4444-555555555555", worktree: "D:\\wt\\new", branch: "bytehive/ana-2" });
  });

  it("says nothing for someone who never had any", async () => {
    await engine.giveJob("do it");
    expect(engine.companyAlerts()).toEqual([]);
  });
});

describe("a role's skills", () => {
  const skill = `${ROOT}\\roles\\marketing\\skills\\campaign-brief\\SKILL.md`;
  const hired = () => engine.companyView()!.company.people.find((p) => p.role === "marketing")!;
  const copy = () => io.files.get(`${ROOT}\\people\\${hired().id}\\plugin\\skills\\campaign-brief\\SKILL.md`);

  it("reach a new hire the way the owner edited them", async () => {
    io.files.set(skill, "edited by hand");
    await waitingJob({ summary: "s", cards: [], hires: [{ role: "marketing", floor: "a" }] });
    await engine.approve(job().id);
    expect(copy()).toBe("edited by hand");
  });
  it("reach someone already hired on the next start: a pane restarts on the files it was given", async () => {
    await waitingJob({ summary: "s", cards: [], hires: [{ role: "marketing", floor: "a" }] });
    await engine.approve(job().id);
    io.files.set(skill, "edited later");
    vi.resetModules();
    engine = await import("./engine");
    await engine.initCompany();
    await started();
    expect(copy()).toBe("edited later");
  });
});

describe("a job file that can't be read", () => {
  it("raises an alert and stays where it is, and the rest of the company starts", async () => {
    await engine.giveJob("do it");
    io.files.set(`${ROOT}\\jobs\\J-0009.json`, "{ half");
    vi.resetModules();
    engine = await import("./engine");
    await engine.initCompany();
    await started();
    expect(engine.companyView()!.jobs.map((j) => j.id)).toEqual(["J-0001"]);
    expect(engine.companyAlerts()).toEqual([expect.objectContaining({ text: expect.stringContaining("J-0009.json") })]);
    expect(io.files.get(`${ROOT}\\jobs\\J-0009.json`)).toBe("{ half");
  });
  it("goes away when you dismiss it", async () => {
    io.files.set(`${ROOT}\\jobs\\J-0009.json`, "{ half");
    vi.resetModules();
    engine = await import("./engine");
    await engine.initCompany();
    await started();
    engine.dismissAlert(engine.companyAlerts()[0].key);
    expect(engine.companyAlerts()).toEqual([]);
  });
});

describe("a company that won't load", () => {
  it("keeps the error, and never writes over the file", async () => {
    const spy = vi.spyOn(console, "error").mockImplementation(() => {});
    io.files.set(`${ROOT}\\company.json`, "{oops");
    vi.resetModules();
    engine = await import("./engine");
    await engine.initCompany();
    await started();
    expect(engine.companyError()).toBeTruthy();
    expect(engine.companyView()).toBeNull();
    expect(io.files.get(`${ROOT}\\company.json`)).toBe("{oops");
    expect(spy).toHaveBeenCalled();
    spy.mockRestore();
  });

  it("has no error when it loads", () => {
    expect(engine.companyError()).toBeNull();
  });
});

const analyst = { id: "analyst", title: "A", department: "A", prompt: "Reads numbers and says what they mean." };
const drafting = (): JobPlan => ({ summary: "s", cards: [], hires: [{ role: "analyst", floor: "a" }], newRoles: [analyst] });

describe("a disk that refuses to save", () => {
  const quiet = () => vi.spyOn(console, "error").mockImplementation(() => {});
  const spoken = () => [...io.told.map((m) => m.text), ...io.sent.map(([, text]) => text)].join();

  it("still places the cards, starts the hires and tells the Founder when approve cannot save", async () => {
    const spy = quiet();
    await waitingJob(planFor("a", 1));
    io.saveFails = true;
    await engine.approve(job().id);
    expect(job().status).toBe("running");
    expect(cards("D:\\a").length).toBe(1);
    expect(io.spawned).toContain("Ana");
    expect(spoken()).toContain("approved");
    expect(spoken()).toContain("J-0001 could not be saved: access denied");
    spy.mockRestore();
  });

  it("does not let a failed save open the door to a second approve", async () => {
    const spy = quiet();
    await waitingJob(planFor("a"));
    io.saveFails = true;
    await engine.approve(job().id);
    await engine.approve(job().id);
    expect(cards("D:\\a").length).toBe(1);
    spy.mockRestore();
  });

  it("takes a request in memory when tick cannot save", async () => {
    const spy = quiet();
    await engine.giveJob("do it");
    io.saveFails = true;
    await request({ type: "plan", ts: 1, from: "founder", job: "J-0001", plan: drafting() });
    expect(job().status).toBe("awaiting-plan");
    expect(spy).toHaveBeenCalled();
    spy.mockRestore();
  });

  it("saves a new role and tells the Founder when one cannot be written", async () => {
    await waitingJob(drafting());
    const real = io.files.set.bind(io.files);
    io.files.set = (k, v) => { if (k.endsWith("roles\\analyst\\prompt.md")) throw { Failed: "disk full" }; return real(k, v); };
    await engine.approve(job().id);
    io.files.set = real;
    expect(job().status).toBe("running");
    expect(spoken()).toContain("analyst was not saved: disk full");
  });

  it("tells the Founder a plan was sent back even when reject cannot save", async () => {
    const spy = quiet();
    await waitingJob(planFor("a"));
    io.saveFails = true;
    await engine.reject(job().id, "too many people");
    expect(job().status).toBe("planning");
    expect(spoken()).toContain("too many people");
    expect(spy).toHaveBeenCalled();
    spy.mockRestore();
  });

  it("tells the Founder of a new job when giveJob cannot save", async () => {
    const spy = quiet();
    io.saveFails = true;
    await engine.giveJob("do it");
    expect(io.told).toEqual([expect.objectContaining({ name: "Founder", text: expect.stringContaining("JOB J-0001") })]);
    expect(spy).toHaveBeenCalled();
    spy.mockRestore();
  });

  it("starts reading requests even when the first save fails", async () => {
    const spy = quiet();
    io.saveFails = true;
    vi.resetModules();
    ({ workspaces } = await import("../appstate"));
    openProjects();
    engine = await import("./engine");
    await engine.initCompany();
    await started();
    await vi.advanceTimersByTimeAsync(1500); // the first tick notes that requests.jsonl isn't there yet
    io.saveFails = false;
    await engine.giveJob("do it");
    await request({ type: "plan", ts: 1, from: "founder", job: "J-0001", plan: drafting() });
    expect(job().status).toBe("awaiting-plan");
    expect(spy).toHaveBeenCalled();
    spy.mockRestore();
  });
});

describe("watching a running job", () => {
  /** Job J-0001 running, with Ana on floor a holding one card in Doing. */
  async function running(tokenCap?: number) {
    const plan = planFor("a", 1);
    await waitingJob({ ...plan, tokenCap, cards: [{ ...plan.cards[0], assignee: "Ana" }] });
    await engine.approve(job().id);
    const ana = engine.companyView()!.company.people.find((p) => p.name === "Ana")!;
    const board = io.boards.get("D:\\a") as Board;
    const doing = board.lists.find((l) => l.title === "Doing")!;
    doing.cards.push(board.lists.find((l) => l.title === "To do")!.cards.pop()!);
    io.told.length = 0;
    io.sent.length = 0;
    return ana;
  }
  const stored = () => JSON.parse(io.files.get(`${ROOT}\\jobs\\J-0001.json`)!) as { status: string; tokensUsed: number; tokenCap: number };
  const said = () => [...io.told.map((m) => m.text), ...io.sent.map(([, text]) => text)];
  const founderPaneRunning = () => addPane("D:\\a", { name: "Founder", personId: "founder" }, { running: true, spawnedAt: Date.now() - 60_000 });
  /** Ana reads `before`, then `after`, 30 seconds apart. */
  async function spend(id: string, before: number, after: number) {
    io.usage.set(id, { fresh: before });
    await vi.advanceTimersByTimeAsync(30_000);
    io.usage.set(id, { fresh: after });
    await vi.advanceTimersByTimeAsync(30_000);
  }

  it("only reads the boards of floors with people on them", async () => {
    await running();
    io.boardReads.length = 0;
    await vi.advanceTimersByTimeAsync(30_000);
    expect(io.boardReads).toEqual(["D:\\a"]);
  });

  it("watches a card a Lead added with its job, and its assignee joins the job", async () => {
    await running();
    const board = io.boards.get("D:\\a") as Board;
    board.lists.find((l) => l.title === "Doing")!.cards.push({ id: "lead-card", title: "Added later", desc: "", labels: [], due: null, checklist: [], job: "J-0001", assignee: "Founder", assigneeId: "founder" });
    await vi.advanceTimersByTimeAsync(10 * 60_000 + 30_000);
    expect(job().people).toContain("founder");
    expect(engine.companyAlerts().map((a) => a.text).join()).toContain("Added later"); // nobody above the Founder to nudge
  });

  it("counts the fresh tokens a person adds from when they were first seen", async () => {
    const ana = await running();
    io.usage.set(ana.id, { fresh: 400 });
    await vi.advanceTimersByTimeAsync(30_000);
    expect(job().tokenBase).toEqual({ [ana.id]: 400 });
    io.usage.set(ana.id, { fresh: 650 });
    await vi.advanceTimersByTimeAsync(30_000);
    expect(job().tokensUsed).toBe(250);
    expect(stored().tokensUsed).toBe(250);
  });

  it("counts someone who picked up a card of the job, from when they took it", async () => {
    await running();
    const board = io.boards.get("D:\\a") as Board;
    board.lists.find((l) => l.title === "Doing")!.cards[0].assigneeId = "founder";
    addPane("D:\\a", { name: "Founder", personId: "founder" }, { running: true, spawnedAt: Date.now() });
    await spend("founder", 500, 800);
    expect(job().people).toContain("founder");
    expect(job().tokensUsed).toBe(300);
  });

  it("does not take a person's unread usage for zero", async () => {
    const ana = await running();
    await vi.advanceTimersByTimeAsync(30_000);
    expect(job().tokenBase).toEqual({});
    io.usage.set(ana.id, { fresh: 90_000 });
    await vi.advanceTimersByTimeAsync(30_000);
    expect(job().tokenBase).toEqual({ [ana.id]: 90_000 });
    expect(job().tokensUsed).toBe(0);
  });

  it("pauses the job at its cap and raises an alert", async () => {
    const ana = await running(1000);
    await spend(ana.id, 100, 1200);
    expect(job().status).toBe("paused");
    expect(stored().status).toBe("paused");
    expect(engine.companyAlerts()).toEqual([expect.objectContaining({ job: "J-0001", level: "stop" })]);
  });

  it("tells the Founder when a job pauses, and everyone when it runs again", async () => {
    const ana = await running(1000);
    founderPaneRunning();
    await spend(ana.id, 100, 1200);
    expect(said().join()).toMatch(/J-0001.*token cap/);
    io.sent.length = 0;
    await engine.raiseCap("J-0001", 1000);
    expect(said().filter((t) => t.includes("J-0001")).length).toBe(2);
  });

  it("leaves someone stopped asleep when the job runs again", async () => {
    const ana = await running(1000);
    founderPaneRunning();
    await spend(ana.id, 100, 1200);
    const pane = [...workspaces.values()].flatMap((w) => [...w.panes.values()]).find((p) => p.spec.personId === ana.id)!;
    pane.running = false;
    pane.restart = vi.fn(async () => {});
    io.told.length = 0;
    io.sent.length = 0;
    await engine.raiseCap("J-0001", 1000);
    expect(pane.restart).not.toHaveBeenCalled();
    expect(said().filter((t) => t.includes("running again"))).toHaveLength(1);
  });

  it("keeps the stop alert for as long as the job is paused, and drops it on resume", async () => {
    const ana = await running(1000);
    await spend(ana.id, 100, 1200);
    await vi.advanceTimersByTimeAsync(3 * 30_000);
    expect(engine.companyAlerts()).toEqual([expect.objectContaining({ key: "cap-J-0001", level: "stop" })]);
    await engine.raiseCap("J-0001", 5000);
    await vi.advanceTimersByTimeAsync(30_000);
    expect(engine.companyAlerts()).toEqual([]);
  });

  it("keeps counting when a person's pane is gone and another person spends", async () => {
    await waitingJob({ ...planFor("a"), tokenCap: 10_000_000, hires: [{ role: "developer", floor: "a" }, { role: "developer", floor: "b" }] });
    await engine.approve(job().id);
    const [one, two] = engine.companyView()!.company.people.filter((p) => p.role === "developer");
    io.usage.set(one.id, { fresh: 0 });
    io.usage.set(two.id, { fresh: 0 });
    await vi.advanceTimersByTimeAsync(30_000);
    io.usage.set(one.id, { fresh: 2_000_000 });
    await vi.advanceTimersByTimeAsync(30_000);
    expect(job().tokensUsed).toBe(2_000_000);
    for (const ws of workspaces.values()) for (const [id, pane] of ws.panes) if (pane.spec.personId === one.id) ws.panes.delete(id);
    io.usage.set(two.id, { fresh: 2_000_000 });
    await vi.advanceTimersByTimeAsync(30_000);
    expect(job().tokensUsed).toBe(4_000_000);
  });

  it("rebases when a session restarts, without losing what was counted", async () => {
    const ana = await running();
    await spend(ana.id, 100, 1100);
    expect(job().tokensUsed).toBe(1000);
    io.usage.set(ana.id, { fresh: 50 }); // /clear: a new session starts from nothing
    await vi.advanceTimersByTimeAsync(30_000);
    expect(job().tokensUsed).toBe(1000);
    io.usage.set(ana.id, { fresh: 350 });
    await vi.advanceTimersByTimeAsync(30_000);
    expect(job().tokensUsed).toBe(1300);
    expect(stored().tokensUsed).toBe(1300);
  });

  it("does not nudge a card that comes back to Doing as if it had been quiet all along", async () => {
    await running();
    const board = io.boards.get("D:\\a") as Board;
    const [doing, todo] = ["Doing", "To do"].map((t) => board.lists.find((l) => l.title === t)!);
    await vi.advanceTimersByTimeAsync(9 * 60_000);
    todo.cards.push(doing.cards.pop()!);
    await vi.advanceTimersByTimeAsync(30_000);
    doing.cards.push(todo.cards.pop()!);
    await vi.advanceTimersByTimeAsync(5 * 60_000);
    expect(said().filter((s) => s.includes("gone quiet"))).toEqual([]);
  });

  it("writes nothing while nothing changes", async () => {
    const ana = await running();
    io.usage.set(ana.id, { fresh: 100 });
    await vi.advanceTimersByTimeAsync(30_000);
    io.files.set(`${ROOT}\\jobs\\J-0001.json`, "untouched");
    io.files.set(`${ROOT}\\company.json`, "untouched");
    await vi.advanceTimersByTimeAsync(5 * 30_000);
    expect(io.files.get(`${ROOT}\\jobs\\J-0001.json`)).toBe("untouched");
    expect(io.files.get(`${ROOT}\\company.json`)).toBe("untouched");
  });

  it("asks the boss to look at a card that has gone quiet", async () => {
    await running();
    await vi.advanceTimersByTimeAsync(30_000);
    expect(said()).toEqual([]);
    await vi.advanceTimersByTimeAsync(10 * 60_000);
    expect(said().filter((s) => s.includes("gone quiet") && s.includes("Do it"))).toHaveLength(1);
  });

  it("leaves the cards of a paused job alone", async () => {
    const ana = await running(1000);
    await spend(ana.id, 100, 1200);
    io.told.length = 0;
    io.sent.length = 0;
    await vi.advanceTimersByTimeAsync(30 * 60_000);
    expect(said()).toEqual([]);
  });

  it("stops a running job for good, and tells the Founder", async () => {
    const ana = await running();
    founderPaneRunning();
    await engine.stopJob("J-0001");
    expect(job().status).toBe("stopped");
    expect(stored().status).toBe("stopped");
    expect(said().join()).toMatch(/J-0001.*stopped/);
    io.told.length = 0;
    io.sent.length = 0;
    await request({ type: "message", ts: 2, from: ana.id, to: "Founder", message: "done" });
    expect(said()).toEqual([expect.stringMatching(/^Held/)]);
  });

  it("stops a paused job, and its stop alert goes with it", async () => {
    const ana = await running(1000);
    await spend(ana.id, 100, 1200);
    await engine.stopJob("J-0001");
    await vi.advanceTimersByTimeAsync(30_000);
    expect(job().status).toBe("stopped");
    expect(engine.companyAlerts()).toEqual([]);
  });

  it("takes a stopped job's warning off Needs you at once", async () => {
    const ana = await running(1000);
    await spend(ana.id, 100, 950);
    expect(engine.companyAlerts()).toEqual([expect.objectContaining({ key: "cap-J-0001", level: "warn" })]);
    await engine.stopJob("J-0001");
    expect(engine.companyAlerts()).toEqual([]);
  });

  it("leaves the cards of a stopped job alone and counts nothing more", async () => {
    const ana = await running();
    await spend(ana.id, 100, 200);
    await engine.stopJob("J-0001");
    io.told.length = 0;
    io.sent.length = 0;
    io.usage.set(ana.id, { fresh: 900 });
    await vi.advanceTimersByTimeAsync(30 * 60_000);
    expect(said()).toEqual([]);
    expect(job().tokensUsed).toBe(100);
  });

  it("stops a job still being planned, and tells the Founder", async () => {
    await engine.giveJob("do it");
    founderPaneRunning();
    io.told.length = 0;
    io.sent.length = 0;
    await engine.stopJob("J-0001");
    expect(job().status).toBe("stopped");
    expect(stored().status).toBe("stopped");
    expect(said().join()).toMatch(/J-0001.*stopped/);
  });

  it("does not stop a job that is over, or one that isn't there", async () => {
    await engine.giveJob("do it");
    await engine.stopJob("J-0001");
    io.told.length = 0;
    io.sent.length = 0;
    await engine.stopJob("J-0001");
    await engine.stopJob("J-9999");
    expect(said()).toEqual([]);
  });

  it("closes a job that needs no work, and shows the owner why", async () => {
    await engine.giveJob("is it done?");
    await request({ type: "job-done", ts: 5, from: "founder", job: "J-0001", summary: "Yes, last week." });
    expect(job().status).toBe("done");
    expect(stored()).toMatchObject({ status: "done", summary: "Yes, last week." });
    expect(engine.companyAlerts()).toEqual([expect.objectContaining({ key: "no-work-J-0001", standing: true, text: expect.stringContaining("Yes, last week.") })]);
  });

  it("raises the cap and carries on a paused job", async () => {
    const ana = await running(1000);
    await spend(ana.id, 100, 1200);
    await engine.raiseCap("J-0001", 5000);
    expect(job()).toMatchObject({ status: "running", tokenCap: 6000 });
    expect(stored()).toMatchObject({ status: "running", tokenCap: 6000 });
  });

  it.each([0, -5, NaN, Infinity])("does not raise the cap by %s", async (extra) => {
    const ana = await running(1000);
    await spend(ana.id, 100, 1200);
    await engine.raiseCap("J-0001", extra);
    expect(job()).toMatchObject({ status: "paused", tokenCap: 1000 });
  });

  it("closes a job that waits for merge, and only that", async () => {
    await running(1000);
    await engine.finishJob("J-0001");
    expect(job().status).toBe("running");
    await request({ type: "job-done", ts: 5, from: "founder", job: "J-0001", summary: "ok" });
    expect(job().status).toBe("awaiting-merge");
    await engine.finishJob("J-0001");
    expect(job().status).toBe("done");
    expect(stored().status).toBe("done");
    await engine.finishJob("J-9999");
    expect(job().status).toBe("done");
  });

  it("does not raise the cap of a job that is not paused", async () => {
    await running(1000);
    await engine.raiseCap("J-0001", 5000);
    expect(job().tokenCap).toBe(1000);
  });
});
