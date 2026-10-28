import { describe, expect, it } from "vitest";
import { filterGrid, gridCounts, gridProjects, projectCounts, statusGroup, type GridRow } from "./gridfilter";

const ROWS: GridRow[] = [
  { id: "a", project: "Shop", state: "needs" },
  { id: "b", project: "Shop", state: "working" },
  { id: "c", project: "Blog", state: "review" },
  { id: "d", project: "Blog", state: "idle" },
  { id: "e", project: "Blog", state: "stopped" },
];

describe("statusGroup", () => {
  it("treats review as needs-you and the rest as quiet", () => {
    expect(statusGroup("needs")).toBe("needs");
    expect(statusGroup("review")).toBe("needs");
    expect(statusGroup("working")).toBe("working");
    expect(statusGroup("idle")).toBe("quiet");
    expect(statusGroup("stopped")).toBe("quiet");
    expect(statusGroup("merged")).toBe("quiet");
  });
});

describe("filterGrid", () => {
  it("shows everything by default, in order", () => {
    expect(filterGrid(ROWS, null, "all").map((r) => r.id)).toEqual(["a", "b", "c", "d", "e"]);
  });

  it("narrows to one project", () => {
    expect(filterGrid(ROWS, "Blog", "all").map((r) => r.id)).toEqual(["c", "d", "e"]);
  });

  it("narrows to a status group across projects", () => {
    expect(filterGrid(ROWS, null, "needs").map((r) => r.id)).toEqual(["a", "c"]);
    expect(filterGrid(ROWS, null, "quiet").map((r) => r.id)).toEqual(["d", "e"]);
  });

  it("combines both filters", () => {
    expect(filterGrid(ROWS, "Blog", "needs").map((r) => r.id)).toEqual(["c"]);
    expect(filterGrid(ROWS, "Shop", "quiet")).toEqual([]);
  });
});

describe("gridCounts", () => {
  it("counts per chip", () => {
    expect(gridCounts(ROWS)).toEqual({ all: 5, needs: 2, working: 1, quiet: 2 });
  });
});

describe("gridProjects", () => {
  it("lists each project once, sorted", () => {
    expect(gridProjects(ROWS)).toEqual(["Blog", "Shop"]);
  });
});

describe("projectCounts", () => {
  it("counts agents per project, sorted", () => {
    expect(projectCounts(ROWS)).toEqual([
      { project: "Blog", n: 3 },
      { project: "Shop", n: 2 },
    ]);
  });
});
