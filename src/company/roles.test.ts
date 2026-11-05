import { describe, it, expect, vi, beforeEach } from "vitest";

const files = new Map<string, string>();
vi.mock("../ipc", () => ({
  fsReadFile: async (_root: string, rel: string) => {
    const content = files.get(rel);
    if (content === undefined) throw { Failed: "no such path: gone" };
    return { content, mtime: 1 };
  },
}));

import { DEFAULT_ROLES, roleSkills, skillFiles } from "./roles";

describe("roleSkills", () => {
  const marketing = DEFAULT_ROLES.find((r) => r.id === "marketing")!;
  const at = (role: string, skill: string) => `roles\\${role}\\skills\\${skill}\\SKILL.md`;
  beforeEach(() => files.clear());

  it("reads the owner's copy, so an edited SKILL.md counts", async () => {
    files.set(at("marketing", "campaign-brief"), "edited by hand");
    expect(await roleSkills("R", marketing)).toEqual([{ name: "campaign-brief", text: "edited by hand" }]);
  });
  it("falls back on the app's copy of a skill the folder does not have", async () => {
    expect(await roleSkills("R", marketing)).toEqual(skillFiles(marketing));
  });
  it("finds a skill the app never shipped", async () => {
    files.set(at("analyst", "charts"), "draw them");
    const analyst = { ...marketing, id: "analyst", skills: ["charts", "ghost"] };
    expect(await roleSkills("R", analyst)).toEqual([{ name: "charts", text: "draw them" }]);
  });
});

describe("default roles", () => {
  it("covers the starting catalog", () => {
    expect(DEFAULT_ROLES.map((r) => r.id).sort()).toEqual(
      ["copywriter", "designer", "developer", "founder", "lead", "marketing", "qa", "researcher"],
    );
  });
  it("names the CLI every role runs on", () => {
    for (const r of DEFAULT_ROLES) {
      expect(typeof r.cli).toBe("string");
      expect(r.cli.length).toBeGreaterThan(0);
    }
  });
  it("gives every role a prompt", () => {
    for (const r of DEFAULT_ROLES) expect(r.prompt.length).toBeGreaterThan(80);
  });
  it("forbids the Founder and the Lead from editing files", () => {
    for (const id of ["founder", "lead"]) expect(DEFAULT_ROLES.find((r) => r.id === id)!.denyTools).toContain("Edit");
  });
  it("keeps the Founder out of the shell too", () => {
    const deny = DEFAULT_ROLES.find((r) => r.id === "founder")!.denyTools;
    expect(deny).toContain("Bash");
    expect(deny).toContain("PowerShell");
    expect(DEFAULT_ROLES.find((r) => r.id === "lead")!.denyTools).not.toContain("Bash");
  });
  it("ships a real skill for marketing", () => {
    const m = DEFAULT_ROLES.find((r) => r.id === "marketing")!;
    expect(skillFiles(m).map((s) => s.name)).toContain("campaign-brief");
    expect(skillFiles(m)[0].text).toMatch(/^---\nname: /);
  });
  it("tells every role with skills to use them", () => {
    for (const r of DEFAULT_ROLES.filter((x) => x.skills.length)) {
      for (const s of r.skills) expect(r.prompt).toContain(s);
      expect(skillFiles(r).length).toBe(r.skills.length);
    }
  });
});
