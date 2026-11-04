import { describe, it, expect } from "vitest";
import { projectOf, orderForDash } from "./dashorder";
import type { FleetStatus } from "./fleet";

const ws = { id: "w1", name: "maestro", dir: "D:\\maestro" };

describe("projectOf", () => {
  it("is the workspace's project", () => {
    expect(projectOf({}, ws)).toEqual({ key: "d:\\maestro", name: "maestro", dir: "D:\\maestro" });
  });
  it("is the folder a resumed conversation carries on in", () => {
    expect(projectOf({ resumeIn: "D:/quy/" }, ws)).toEqual({ key: "d:\\quy", name: "quy", dir: "D:/quy/" });
  });
  it("puts the same folder in one project, however it is spelled", () => {
    expect(projectOf({ resumeIn: "d:/maestro/" }, ws).key).toBe(projectOf({}, ws).key);
  });
  it("falls back to the workspace when there is no folder", () => {
    expect(projectOf({}, { id: "w9", name: "Project 2", dir: null })).toEqual({ key: "ws:w9", name: "Project 2", dir: null });
  });
});

const a = (name: string, status: FleetStatus, project: string) => ({ name, status, project, projectKey: project.toLowerCase() });
const names = (xs: { name: string }[]) => xs.map((x) => x.name);

describe("orderForDash", () => {
  it("keeps each project together, the one that needs you first", () => {
    const out = orderForDash([
      a("Ana", "active", "alpha"), a("Bo", "idle", "beta"), a("Cy", "needs", "beta"), a("Di", "idle", "alpha"),
    ]);
    expect(names(out)).toEqual(["Cy", "Bo", "Ana", "Di"]);
  });
  it("puts projects with only stopped agents last", () => {
    const out = orderForDash([a("Ana", "stopped", "alpha"), a("Bo", "idle", "zeta")]);
    expect(names(out)).toEqual(["Bo", "Ana"]);
  });
  it("does not reorder rows when an agent flips between active and idle", () => {
    const before = orderForDash([a("Ana", "active", "p"), a("Bo", "idle", "p")]);
    const after = orderForDash([a("Ana", "idle", "p"), a("Bo", "active", "p")]);
    expect(names(before)).toEqual(names(after));
  });
  it("lists stopped agents after running ones inside a project", () => {
    expect(names(orderForDash([a("Ana", "stopped", "p"), a("Bo", "idle", "p")]))).toEqual(["Bo", "Ana"]);
  });
});
