import { describe, it, expect, beforeEach, afterEach } from "vitest";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { floorDir, planAsFounder, queueRequest, readCompany, readRoles, sendAsMember, statusFor } from "./company.js";

let root: string;
const look = { shirt: "#fff", hair: "#000", prop: "none" };
const people = [
  { id: "founder", name: "Founder", role: "founder", rank: "founder", floor: null, boss: null, cli: "claude", look },
  { id: "l1", name: "Hai", role: "lead", rank: "lead", floor: "a", boss: "founder", cli: "claude", look },
  { id: "d1", name: "Linh", role: "developer", rank: "staff", floor: "a", boss: "l1", cli: "claude", look },
];
const limits = { peoplePerJob: 5, peoplePerLead: 5, peopleTotal: 20, tokensPerJob: 5000000, stallMinutes: 10, maxReturns: 3 };

beforeEach(() => {
  root = fs.mkdtempSync(path.join(os.tmpdir(), "bytehive-company-"));
  fs.writeFileSync(path.join(root, "company.json"), JSON.stringify({ version: 1, floors: [{ id: "a", dir: "D:\\a", name: "a" }], people, limits, nextJob: 1 }));
  const r = path.join(root, "roles", "developer");
  fs.mkdirSync(r, { recursive: true });
  fs.writeFileSync(path.join(r, "role.json"), JSON.stringify({ id: "developer", title: "Developer", department: "Eng", rank: "staff", cli: "claude", denyTools: [], look, skills: [] }));
  fs.writeFileSync(path.join(r, "prompt.md"), "p");
});
afterEach(() => fs.rmSync(root, { recursive: true, force: true }));

const lines = () => fs.readFileSync(path.join(root, "requests.jsonl"), "utf8").trim().split("\n").map((l) => JSON.parse(l));

describe("company MCP", () => {
  it("reads the roster", () => {
    expect(readCompany(root)?.people.length).toBe(3);
  });
  it("reads files saved with a byte order mark, as Windows editors do", () => {
    for (const f of ["company.json", path.join("roles", "developer", "role.json")]) {
      const at = path.join(root, f);
      fs.writeFileSync(at, `﻿${fs.readFileSync(at, "utf8")}`);
    }
    expect(readCompany(root)?.people.length).toBe(3);
    expect(readRoles(root).map((r) => r.id)).toEqual(["developer"]);
  });
  it("queues a message to the boss when no target is given", () => {
    expect(sendAsMember(root, "d1", undefined, "done", 5)).toBe("sent to Hai");
    expect(lines()[0]).toMatchObject({ type: "message", from: "d1", to: "Hai", message: "done", ts: 5 });
  });
  it("tells a person with nobody to message so, instead of saying it was sent", () => {
    const at = path.join(root, "company.json");
    fs.writeFileSync(at, JSON.stringify({ version: 1, floors: [], people: [people[0]], limits, nextJob: 1 }));
    expect(() => sendAsMember(root, "founder", undefined, "hi", 5)).toThrow(/nobody/);
    expect(fs.existsSync(path.join(root, "requests.jsonl"))).toBe(false);
  });
  it("refuses a skip level message and queues nothing", () => {
    expect(() => sendAsMember(root, "d1", "Founder", "hi", 5)).toThrow(/Hai/);
    expect(fs.existsSync(path.join(root, "requests.jsonl"))).toBe(false);
  });
  it("only lets the Founder plan, and validates the plan", () => {
    const plan = { summary: "s", cards: [], hires: [{ role: "developer", floor: "a" }] };
    expect(() => planAsFounder(root, "l1", "J-0001", plan, 5)).toThrow(/Founder/);
    expect(() => planAsFounder(root, "founder", "J-0001", { ...plan, hires: [{ role: "ghost", floor: "a" }] }, 5)).toThrow(/ghost/);
    planAsFounder(root, "founder", "J-0001", plan, 5);
    expect(lines()[0]).toMatchObject({ type: "plan", job: "J-0001" });
  });
  it("lets the Founder read any floor and others only their own", () => {
    expect(floorDir(root, "founder", "a")).toBe("D:\\a");
    expect(() => floorDir(root, "d1", "a")).toThrow(/Founder/);
  });
  it("shows the Founder the floors and people", () => {
    expect(statusFor(root, "founder")).toMatchObject({ floors: [{ id: "a" }], people: expect.any(Array) });
  });
  it("lists the open jobs for the Founder, so a job can't be forgotten", () => {
    const dir = path.join(root, "jobs");
    fs.mkdirSync(dir);
    const job = (id: string, status: string) => fs.writeFileSync(path.join(dir, `${id}.json`), JSON.stringify({ id, title: `t ${id}`, ask: "a", status, plan: null, tokenCap: 10, tokensUsed: 1, tokenBase: {}, people: ["d1"], createdAt: 1 }));
    for (const [id, status] of [["J-1", "running"], ["J-2", "paused"], ["J-3", "planning"], ["J-4", "awaiting-plan"], ["J-5", "awaiting-merge"], ["J-6", "done"], ["J-7", "rejected"], ["J-8", "stopped"]]) job(id, status);
    fs.writeFileSync(path.join(dir, "J-9.json"), "{ half");
    const seen = statusFor(root, "founder") as { jobs: Array<{ id: string; status: string; title: string }> };
    expect(seen.jobs.map((j) => j.id)).toEqual(["J-1", "J-2", "J-3", "J-4", "J-5"]);
    expect(seen.jobs[0]).toEqual({ id: "J-1", title: "t J-1", status: "running", tokensUsed: 1, tokenCap: 10, people: ["d1"] });
    expect(statusFor(root, "l1")).not.toHaveProperty("jobs");
    expect(statusFor(root, "d1")).not.toHaveProperty("jobs");
  });
  it("lists no jobs before there are any", () => {
    expect(statusFor(root, "founder")).toMatchObject({ jobs: [] });
  });
  it("appends rather than overwrites", () => {
    queueRequest(root, { type: "job-done", ts: 1, from: "founder", job: "J-1", summary: "x" });
    queueRequest(root, { type: "job-done", ts: 2, from: "founder", job: "J-2", summary: "y" });
    expect(lines().length).toBe(2);
  });
});

describe("a new role in a plan", () => {
  const draft = { id: "data-analyst", title: "Analyst", department: "Data", prompt: "Reads numbers." };
  const plan = { summary: "s", cards: [], hires: [{ role: "data-analyst", floor: "a" }], newRoles: [draft] };
  it("travels with the plan and saves nothing", () => {
    planAsFounder(root, "founder", "J-0001", plan, 5);
    expect(lines()[0]).toMatchObject({ type: "plan", plan: { newRoles: [draft] } });
    expect(readRoles(root).map((r) => r.id)).toEqual(["developer"]);
  });
  it("refuses an id the app would refuse, or a role that exists", () => {
    expect(() => planAsFounder(root, "founder", "J-0001", { ...plan, newRoles: [{ ...draft, id: "../founder" }] }, 5)).toThrow(/id/);
    expect(() => planAsFounder(root, "founder", "J-0001", { ...plan, newRoles: [{ ...draft, id: "developer" }] }, 5)).toThrow(/exists/);
    expect(fs.existsSync(path.join(root, "requests.jsonl"))).toBe(false);
  });
});
