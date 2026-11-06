/* The server as a member's CLI sees it: over a real MCP connection, with the
 * company under a home folder made for the test. */
import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { InMemoryTransport } from "@modelcontextprotocol/sdk/inMemory.js";
import { createServer } from "./server.js";

const look = { shirt: "#fff", hair: "#000", prop: "none" };
const people = [
  { id: "founder", name: "Founder", role: "founder", rank: "founder", floor: null, boss: null, cli: "claude", look },
  { id: "l1", name: "Hai", role: "lead", rank: "lead", floor: "a", boss: "founder", cli: "claude", look },
  { id: "d1", name: "Linh", role: "developer", rank: "staff", floor: "a", boss: "l1", cli: "claude", look },
];
const limits = { peoplePerJob: 5, peoplePerLead: 5, peopleTotal: 20, tokensPerJob: 5000000, stallMinutes: 10, maxReturns: 3 };

let home: string;
let dir: string;
const company = () => path.join(home, ".bytehive", "company");

async function connect(personId?: string): Promise<Client> {
  vi.stubEnv("BYTEHIVE_AGENT_ID", personId ?? "");
  const server = createServer(dir);
  const [ours, theirs] = InMemoryTransport.createLinkedPair();
  const client = new Client({ name: "test", version: "1.0.0" });
  await Promise.all([server.connect(theirs), client.connect(ours)]);
  return client;
}

beforeEach(() => {
  home = fs.mkdtempSync(path.join(os.tmpdir(), "bytehive-home-"));
  dir = fs.mkdtempSync(path.join(os.tmpdir(), "bytehive-project-"));
  vi.stubEnv("USERPROFILE", home);
  vi.stubEnv("HOME", home);
  fs.mkdirSync(company(), { recursive: true });
  fs.writeFileSync(path.join(company(), "company.json"), JSON.stringify({ version: 1, floors: [{ id: "a", dir, name: "a" }], people, limits, nextJob: 1 }));
});
afterEach(() => {
  vi.unstubAllEnvs();
  fs.rmSync(home, { recursive: true, force: true });
  fs.rmSync(dir, { recursive: true, force: true });
});

const text = (r: unknown) => (r as { content: Array<{ text: string }> }).content[0].text;
const requests = () => {
  const at = path.join(company(), "requests.jsonl");
  return fs.existsSync(at) ? fs.readFileSync(at, "utf8").trim().split("\n").map((l) => JSON.parse(l)) : [];
};

describe("the instructions", () => {
  it("do not tell a member to stop and wait for the user", async () => {
    const said = (await connect("d1")).getInstructions()!;
    expect(said).not.toContain("Stop and let the user review");
    expect(said).not.toContain("Proposed");
    expect(said).toContain("card_done");
  });
  it("still tell everyone else to plan and wait", async () => {
    const said = (await connect()).getInstructions()!;
    expect(said).toContain("Stop and let the user review");
    expect(said).toContain("Proposed");
  });
});

describe("a member's tools", () => {
  const names = async (id?: string) => (await (await connect(id)).listTools()).tools.map((t) => t.name);

  it("are there for a member, and role_propose is not", async () => {
    const tools = await names("d1");
    for (const t of ["company_status", "role_list", "job_plan", "job_finish"]) expect(tools).toContain(t);
    expect(tools).not.toContain("role_propose");
  });
  it("are not there for an agent outside the company", async () => {
    const tools = await names();
    for (const t of ["company_status", "role_list", "job_plan", "job_finish"]) expect(tools).not.toContain(t);
  });
  it("send fleet_send through the company, to the boss when no one is named", async () => {
    const client = await connect("d1");
    expect(text(await client.callTool({ name: "fleet_send", arguments: { message: "done" } }))).toBe("sent to Hai");
    expect(requests()).toEqual([expect.objectContaining({ type: "message", from: "d1", to: "Hai", message: "done" })]);
    expect(fs.existsSync(path.join(dir, ".bytehive", "outbox.jsonl"))).toBe(false);
  });
  it("refuse a message past the boss", async () => {
    const r = await (await connect("d1")).callTool({ name: "fleet_send", arguments: { to: "Founder", message: "hi" } });
    expect(r.isError).toBe(true);
    expect(requests()).toEqual([]);
  });
  it("refuse agent_spawn: hiring goes through the plan", async () => {
    const r = await (await connect("l1")).callTool({ name: "agent_spawn", arguments: { cli: "claude" } });
    expect(r.isError).toBe(true);
    expect(text(r)).toContain("Founder's plan");
  });
  it("say job_finish also closes a job that needs no work", async () => {
    const { tools } = await (await connect("founder")).listTools();
    expect(tools.find((t) => t.name === "job_finish")?.description).toMatch(/needs no work/);
  });
  it("only let the Founder call job_finish", async () => {
    const args = { name: "job_finish", arguments: { job: "J-0001", summary: "done" } };
    const r = await (await connect("l1")).callTool(args);
    expect(r.isError).toBe(true);
    expect(text(r)).toContain("Only the Founder");
    expect(requests()).toEqual([]);
    expect((await (await connect("founder")).callTool(args)).isError).toBeFalsy();
    expect(requests()).toEqual([expect.objectContaining({ type: "job-done", from: "founder", job: "J-0001", summary: "done" })]);
  });
  it("only let the Founder call job_plan", async () => {
    const args = { name: "job_plan", arguments: { job: "J-0001", summary: "s", cards: [], hires: [] } };
    expect((await (await connect("l1")).callTool(args)).isError).toBe(true);
    expect(requests()).toEqual([]);
  });
  it("take newRoles through job_plan", async () => {
    const newRoles = [{ id: "analyst", title: "Analyst", department: "Data", prompt: "Reads numbers." }];
    await (await connect("founder")).callTool({ name: "job_plan", arguments: { job: "J-0001", summary: "s", cards: [], hires: [], newRoles } });
    expect(requests()).toEqual([expect.objectContaining({ type: "plan", plan: expect.objectContaining({ newRoles }) })]);
  });
  it("let the Founder read a floor's board and nobody else", async () => {
    const args = { name: "board_get", arguments: { floor: "a" } };
    expect((await (await connect("founder")).callTool(args)).isError).toBeFalsy();
    expect((await (await connect("d1")).callTool(args)).isError).toBe(true);
    expect((await (await connect()).callTool(args)).isError).toBe(true);
  });
  it("scope company_status to who is asking", async () => {
    const status = async (id: string) => JSON.parse(text(await (await connect(id)).callTool({ name: "company_status", arguments: {} })));
    expect((await status("founder")).people.map((p: { id: string }) => p.id)).toEqual(["founder", "l1", "d1"]);
    expect((await status("d1")).people.map((p: { id: string }) => p.id)).toEqual(["l1", "d1"]);
  });
});

describe("card_add", () => {
  it("puts a card on the Job it names, for the app to watch", async () => {
    const client = await connect("l1");
    await client.callTool({ name: "card_add", arguments: { list: "To do", title: "Write tests", job: "J-0007" } });
    const board = JSON.parse(fs.readFileSync(path.join(dir, ".bytehive", "board.json"), "utf8"));
    expect(board.lists.flatMap((l: { cards: unknown[] }) => l.cards)).toEqual([expect.objectContaining({ title: "Write tests", job: "J-0007" })]);
  });
});
