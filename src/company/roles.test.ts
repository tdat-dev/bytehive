import { describe, it, expect } from "vitest";
import { DEFAULT_ROLES, skillFiles } from "./roles";

describe("default roles", () => {
  it("covers the starting catalog", () => {
    expect(DEFAULT_ROLES.map((r) => r.id).sort()).toEqual(
      ["copywriter", "designer", "developer", "founder", "lead", "marketing", "qa", "researcher"],
    );
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
