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

describe("card_add", () => {
  it("puts a card on the Job it names, for the app to watch", async () => {
    const client = await connect("l1");
    await client.callTool({ name: "card_add", arguments: { list: "To do", title: "Write tests", job: "J-0007" } });
    const board = JSON.parse(fs.readFileSync(path.join(dir, ".bytehive", "board.json"), "utf8"));
    expect(board.lists.flatMap((l: { cards: unknown[] }) => l.cards)).toEqual([expect.objectContaining({ title: "Write tests", job: "J-0007" })]);
  });
});
