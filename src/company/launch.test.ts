import { describe, it, expect } from "vitest";
import { buildRoleLaunch, composePrompt, mcpConfig, nodeRunsCompany, pluginManifest } from "./launch";
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

describe("the Founder's laws", () => {
  it("say a new role goes in the plan, for the owner to read", () => {
    expect(FOUNDER_LAWS).toContain("newRoles");
    expect(FOUNDER_LAWS).not.toContain("role_propose");
  });
});

describe("nodeRunsCompany", () => {
  it.each(["v22.13.0", "v22.13.0\r\n", "v22.14.1\n", "v23.0.0", "v100.1.0"])("takes %j", (v) => {
    expect(nodeRunsCompany(v)).toBe(true);
  });
  it.each(["v22.12.9", "v22.9.0", "v20.19.0", "v9.99.0", "", "'node' is not recognized", "22"])("refuses %j", (v) => {
    expect(nodeRunsCompany(v)).toBe(false);
  });
});

describe("buildRoleLaunch", () => {
  it("passes the prompt file, the MCP config, the plugin and the identity", () => {
    const l = buildRoleLaunch({ person, role, promptFile: "C:\\p\\prompt.md", mcpConfig: "C:\\p\\mcp.json", pluginDir: "C:\\p\\plugin" });
    expect(l.args).toEqual(["--append-system-prompt-file", "C:\\p\\prompt.md", "--mcp-config", "C:\\p\\mcp.json", "--plugin-dir", "C:\\p\\plugin"]);
    expect(l.env).toContainEqual(["BYTEHIVE_AGENT_ID", "p1"]);
  });
  it("adds denied tools only when the role has some, and skips an empty plugin", () => {
    const l = buildRoleLaunch({ person, role: { ...role, denyTools: ["Edit", "Write"] }, promptFile: "f", mcpConfig: "m", pluginDir: null });
    expect(l.args).toEqual(["--append-system-prompt-file", "f", "--mcp-config", "m", "--disallowedTools", "Edit,Write"]);
  });
});

describe("mcpConfig", () => {
  it("starts the server the app carries with node, under the name the laws use", () => {
    const server = "C:\\Program Files\\ByteHive\\mcp-server\\bytehive-mcp.mjs";
    expect(JSON.parse(mcpConfig(server))).toEqual({ mcpServers: { bytehive: { command: "node", args: [server] } } });
  });
});

describe("pluginManifest", () => {
  it("is valid JSON with a safe name", () => {
    expect(JSON.parse(pluginManifest({ ...person, id: "p 1!" })).name).toBe("bytehive-role-p-1");
  });
});
