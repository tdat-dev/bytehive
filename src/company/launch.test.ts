import { describe, it, expect } from "vitest";
import { buildRoleLaunch, composePrompt, founderFolder, FOUNDER_SETTINGS, mcpConfig, nodeVerdict, pluginManifest } from "./launch";
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

describe("what the Founder is told about open jobs", () => {
  it("says company_status lists them", () => {
    expect(FOUNDER_LAWS).toMatch(/company_status lists every open job/);
  });
});

describe("what the Founder is told about a job with nothing to do", () => {
  it("says to tell the owner with job_finish instead of planning it", () => {
    expect(FOUNDER_LAWS).toMatch(/If a job needs no work.*job_finish/);
  });
});

describe("what a Lead is told about their own cards", () => {
  it("says to pass the job to card_add", () => {
    expect(LEAD_LAWS).toMatch(/card_add.*job/);
  });
});

describe("founderFolder", () => {
  const a = { dir: "D:\\a" };
  const b = { dir: "D:\\b" };
  const loose = { dir: null };
  it("is the active project", () => {
    expect(founderFolder(b, [a, b])).toBe("D:\\b");
  });
  it("is the first open project with a folder when the active one has none", () => {
    expect(founderFolder(loose, [loose, a, b])).toBe("D:\\a");
  });
  it("is the first open project with a folder when none is active", () => {
    expect(founderFolder(null, [a, b])).toBe("D:\\a");
  });
  it("is nothing when no open project has a folder", () => {
    expect(founderFolder(loose, [loose])).toBeNull();
    expect(founderFolder(null, [])).toBeNull();
  });
});

describe("nodeVerdict", () => {
  const NEEDS = "ByteHive needs Node.js 22.13 or newer to run the Company.";
  it.each(["v22.13.0", "v22.13.0\r\n", "v22.14.1\n", "v23.0.0", "v24.6.0\r\n", "v100.1.0"])("lets %j through", (v) => {
    expect(nodeVerdict({ printed: v })).toEqual({});
  });
  it.each([["v22.12.9", "v22.12.9"], ["v22.9.0\r\n", "v22.9.0"], ["v20.19.0\n", "v20.19.0"], ["v18.20.0", "v18.20.0"], ["v9.99.0", "v9.99.0"]])("refuses %j, naming the version", (v, found) => {
    expect(nodeVerdict({ printed: v })).toEqual({ refuse: `${NEEDS} Found ${found}.` });
  });
  it.each(["", "\r\n", "  "])("says Node was not found when the command printed nothing: %j", (v) => {
    expect(nodeVerdict({ printed: v })).toEqual({ refuse: `${NEEDS} Node.js was not found. If you just installed it, restart ByteHive.` });
  });
  it("does not block when the check itself failed or timed out, and gives the reason to log", () => {
    expect(nodeVerdict({ failed: "node took longer than 30 s" })).toEqual({ log: expect.stringContaining("node took longer than 30 s") });
  });
  it.each(["'node' is not recognized", "22", "Welcome to nvm"])("does not block on an answer it cannot read: %j", (v) => {
    expect(nodeVerdict({ printed: v })).toEqual({ log: expect.stringContaining(v) });
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

describe("the Founder's settings", () => {
  it("turn every hook off", () => {
    expect(JSON.parse(FOUNDER_SETTINGS)).toEqual({ disableAllHooks: true });
  });
  it("are passed when given, and only then", () => {
    const base = { person, role, promptFile: "f", mcpConfig: "m", pluginDir: null };
    expect(buildRoleLaunch({ ...base, settings: "C:\\p\\settings.json" }).args).toEqual(["--append-system-prompt-file", "f", "--mcp-config", "m", "--settings", "C:\\p\\settings.json"]);
    expect(buildRoleLaunch({ ...base, settings: null }).args).not.toContain("--settings");
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
