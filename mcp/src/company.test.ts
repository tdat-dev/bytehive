import { describe, it, expect, beforeEach, afterEach } from "vitest";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { floorDir, planAsFounder, proposeRole, queueRequest, readCompany, readRoles, sendAsMember, statusFor } from "./company.js";

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
  it("appends rather than overwrites", () => {
    queueRequest(root, { type: "job-done", ts: 1, from: "founder", job: "J-1", summary: "x" });
    queueRequest(root, { type: "job-done", ts: 2, from: "founder", job: "J-2", summary: "y" });
    expect(lines().length).toBe(2);
  });
});

describe("role_propose", () => {
  const draft = { job: "J-0001", id: "data-analyst", title: "Analyst", department: "Data", prompt: "p" };
  it("queues a role from the Founder", () => {
    proposeRole(root, "founder", draft, 5);
    expect(lines()[0]).toMatchObject({ type: "role", job: "J-0001", role: { id: "data-analyst", rank: "staff", cli: "claude", denyTools: [] } });
  });
  it("only lets the Founder propose", () => {
    expect(() => proposeRole(root, "l1", draft, 5)).toThrow(/Founder/);
  });
  it("refuses an id the app would refuse", () => {
    expect(() => proposeRole(root, "founder", { ...draft, id: "../founder" }, 5)).toThrow(/id/);
    expect(fs.existsSync(path.join(root, "requests.jsonl"))).toBe(false);
  });
});
