import { describe, it, expect } from "vitest";
import { buildRoleLaunch, composePrompt, pluginManifest } from "./launch";
import { emptyCompany } from "./org";
import { FOUNDER_LAWS, LEAD_LAWS, STAFF_LAWS } from "../laws";
import { FOUNDER_ID, type Person, type RoleDef } from "./types";

const look = { shirt: "#fff", hair: "#000", prop: "none" };
const role: RoleDef = { id: "marketing", title: "Marketing", department: "Marketing", rank: "staff", cli: "claude", denyTools: [], look, prompt: "You are the marketer.", skills: ["campaign-brief"] };
const person: Person = { id: "p1", name: "Vy", role: "marketing", rank: "staff", floor: "gcare", boss: FOUNDER_ID, cli: "claude", look };
const c = { ...emptyCompany(), floors: [{ id: "gcare", dir: "D:\\gcare", name: "gcare" }], people: [...emptyCompany().people, person] };

describe("composePrompt", () => {
  it("puts the rank's laws, the role and the person's place in one prompt", () => {
    const text = composePrompt(person, role, c);
    expect(text).toContain(STAFF_LAWS);
    expect(text).toContain("You are the marketer.");
    expect(text).toContain("Vy");
    expect(text).toContain("Founder");
    expect(text).toContain("gcare");
  });
  it("picks the laws by rank", () => {
    expect(composePrompt({ ...person, rank: "lead" }, role, c)).toContain(LEAD_LAWS);
    expect(composePrompt({ ...person, rank: "founder", floor: null, boss: null }, role, c)).toContain(FOUNDER_LAWS);
  });
  it("does not tell a member to wait for the user's approval", () => {
    expect(STAFF_LAWS).not.toMatch(/wait for the user/i);
  });
});

describe("buildRoleLaunch", () => {
  it("passes the prompt file, the plugin and the identity", () => {
    const l = buildRoleLaunch({ person, role, promptFile: "C:\\p\\prompt.md", pluginDir: "C:\\p\\plugin" });
    expect(l.args).toEqual(["--append-system-prompt-file", "C:\\p\\prompt.md", "--plugin-dir", "C:\\p\\plugin"]);
    expect(l.env).toContainEqual(["BYTEHIVE_AGENT_ID", "p1"]);
  });
  it("adds denied tools only when the role has some, and skips an empty plugin", () => {
    const l = buildRoleLaunch({ person, role: { ...role, denyTools: ["Edit", "Write"] }, promptFile: "f", pluginDir: null });
    expect(l.args).toEqual(["--append-system-prompt-file", "f", "--disallowedTools", "Edit,Write"]);
  });
});

describe("pluginManifest", () => {
  it("is valid JSON with a safe name", () => {
    expect(JSON.parse(pluginManifest({ ...person, id: "p 1!" })).name).toBe("bytehive-role-p-1");
  });
});
