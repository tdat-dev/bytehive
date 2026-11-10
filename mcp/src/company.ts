/* The company seen from an agent. This side only reads company.json and queues
 * requests; the app owns the file and checks everything again on delivery. */
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { z } from "zod";
import { parseOwned, personById, resolveTargets, validatePlan } from "../../src/company/org";
import type { Company, CompanyRequest, Job, JobPlan, RoleDef } from "../../src/company/types";
import { appendLine } from "./fleet.js";

export const companyRoot = (home = os.homedir()) => process.env.BYTEHIVE_COMPANY_ROOT?.trim() || path.join(home, ".bytehive", "company");

export function readCompany(root: string): Company | null {
  try {
    return parseOwned<Company>(fs.readFileSync(path.join(root, "company.json"), "utf8"));
  } catch {
    return null;
  }
}

/** A Lead or the Founder moves a card to Doing for someone else; only the person
 *  who does the work takes it that way, so the app's watchers follow the worker. */
export function claimsCards(root: string, personId: string): boolean {
  const rank = readCompany(root)?.people.find((p) => p.id === personId)?.rank;
  return rank !== "lead" && rank !== "founder";
}

export function readRoles(root: string): RoleDef[] {
  const dir = path.join(root, "roles");
  if (!fs.existsSync(dir)) return [];
  const out: RoleDef[] = [];
  for (const id of fs.readdirSync(dir)) {
    try {
      const def = parseOwned<Omit<RoleDef, "prompt">>(fs.readFileSync(path.join(dir, id, "role.json"), "utf8"));
      out.push({ ...def, prompt: fs.readFileSync(path.join(dir, id, "prompt.md"), "utf8") });
    } catch { /* an incomplete role is not offered */ }
  }
  return out;
}

const CLOSED: Job["status"][] = ["done", "rejected", "stopped"];

/** Every Job that still wants something, for a Founder who may have lost track of
 *  one: it has a new session after a full context, or it restarted. */
function openJobs(root: string): unknown[] {
  const dir = path.join(root, "jobs");
  if (!fs.existsSync(dir)) return [];
  const out: Job[] = [];
  for (const name of fs.readdirSync(dir).filter((n) => n.endsWith(".json")).sort()) {
    try {
      out.push(parseOwned<Job>(fs.readFileSync(path.join(dir, name), "utf8")));
    } catch { /* the app raises an alert for a damaged Job file */ }
  }
  return out.filter((j) => !CLOSED.includes(j.status)).map(({ id, title, status, tokensUsed, tokenCap, people }) => ({ id, title, status, tokensUsed, tokenCap, people }));
}

export function queueRequest(root: string, req: CompanyRequest): void {
  fs.mkdirSync(root, { recursive: true });
  appendLine(path.join(root, "requests.jsonl"), JSON.stringify(req));
}

const company = (root: string): Company => {
  const c = readCompany(root);
  if (!c) throw new Error("The company has not been set up in ByteHive yet.");
  return c;
};

function founderOnly(c: Company, id: string): void {
  if (personById(c, id)?.rank !== "founder") throw new Error("Only the Founder can do this. Ask your boss.");
}

export function sendAsMember(root: string, from: string, to: string | undefined, message: string, now: number): string {
  const text = message.trim();
  if (!text) throw new Error("message must not be empty");
  const r = resolveTargets(company(root), from, to?.trim() || null);
  if ("error" in r) throw new Error(r.error);
  for (const p of r.people) queueRequest(root, { type: "message", ts: now, from, to: p.name, message: text });
  return `queued for ${r.people.map((p) => p.name).join(", ")}. If a job holds it, you will hear back.`;
}

export function planAsFounder(root: string, from: string, job: string, plan: JobPlan, now: number): void {
  const c = company(root);
  founderOnly(c, from);
  const errors = validatePlan(c, plan, readRoles(root));
  if (errors.length) throw new Error(errors.join(" "));
  queueRequest(root, { type: "plan", ts: now, from, job, plan });
}

export function statusFor(root: string, id: string): unknown {
  const c = company(root);
  const me = personById(c, id);
  const people = c.people
    .filter((p) => me?.rank === "founder" || p.floor === me?.floor || p.id === me?.boss)
    .map(({ id: pid, name, role, rank, floor, boss }) => ({ id: pid, name, role, rank, floor, boss }));
  const founder = me?.rank === "founder";
  return { floors: founder ? c.floors : c.floors.filter((f) => f.id === me?.floor), people, limits: c.limits, ...(founder ? { jobs: openJobs(root) } : {}) };
}

export function floorDir(root: string, id: string, floor: string): string {
  const c = company(root);
  founderOnly(c, id);
  const f = c.floors.find((x) => x.id === floor);
  if (!f) throw new Error(`Unknown floor ${floor}. See company_status.`);
  return f.dir;
}

const planSchema = {
  job: z.string().describe("The job id, like J-0142"),
  summary: z.string().describe("What will be done, in two lines, for the owner to read"),
  cards: z.array(z.object({
    floor: z.string(),
    title: z.string(),
    desc: z.string(),
    checklist: z.array(z.string()),
    assignee: z.string().optional().describe("Name of an existing person, or omit for the Lead to choose"),
  })),
  hires: z.array(z.object({ role: z.string(), floor: z.string() })),
  tokenCap: z.number().int().positive().optional(),
  newRoles: z.array(z.object({
    id: z.string().describe("Short lowercase id, like data-analyst"),
    title: z.string(),
    department: z.string(),
    prompt: z.string().describe("What this person does and how, at least three sentences. The owner reads it before approving."),
  })).optional().describe("Roles to create when none fits. Staff only, saved only if the owner approves; hire them in hires by id."),
};

type Reply = { content: { type: "text"; text: string }[]; isError?: boolean };
const reply = (fn: () => unknown): Reply => {
  try {
    const out = fn();
    return { content: [{ type: "text", text: typeof out === "string" ? out : JSON.stringify(out, null, 2) }] };
  } catch (e) {
    return { content: [{ type: "text", text: e instanceof Error ? e.message : String(e) }], isError: true };
  }
};

/** Tools only members of the company get. */
export function registerCompanyTools(server: McpServer, personId: string, root = companyRoot()): void {
  server.registerTool("company_status", {
    description: "The floors and people you can see in the company, and its limits. The Founder also gets every open job.",
    inputSchema: {},
  }, async () => reply(() => statusFor(root, personId)));

  server.registerTool("role_list", {
    description: "Roles the Founder can hire.",
    inputSchema: {},
  }, async () => reply(() => readRoles(root).map(({ id, title, department, skills }) => ({ id, title, department, skills }))));

  server.registerTool("job_plan", {
    description: "Founder only: send a plan for the owner to approve. Hires happen only after approval.",
    inputSchema: planSchema,
  }, async ({ job, summary, cards, hires, tokenCap, newRoles }) =>
    reply(() => { planAsFounder(root, personId, job, { summary, cards, hires, tokenCap, newRoles }, Date.now()); return "plan sent, wait for the answer"; }));

  server.registerTool("job_finish", {
    description: "Founder only: report a job finished so the owner can review and merge it. If a job needs no work, call this while it is planning with the reason as the summary: it closes with nothing to merge.",
    inputSchema: { job: z.string(), summary: z.string() },
  }, async ({ job, summary }) => reply(() => {
    founderOnly(company(root), personId);
    queueRequest(root, { type: "job-done", ts: Date.now(), from: personId, job, summary });
    return "reported to the owner";
  }));
}
