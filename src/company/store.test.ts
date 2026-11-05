import { describe, it, expect, vi, beforeEach } from "vitest";

const files = new Map<string, string>();
const key = (root: string, rel: string) => `${root}|${rel}`;
vi.mock("../ipc", () => ({
  fsReadFile: async (root: string, rel: string) => {
    const content = files.get(key(root, rel));
    if (content === undefined) throw { Failed: "no such path: gone" };
    return { content, mtime: 1 };
  },
  fsWriteFile: async (root: string, rel: string, content: string) => { files.set(key(root, rel), content); return { mtime: 2 }; },
  fsCreateFile: async (root: string, rel: string) => { if (!files.has(key(root, rel))) files.set(key(root, rel), ""); },
  fsCreateDir: async () => {},
  fsRename: async (root: string, from: string, to: string) => {
    files.set(key(root, to), files.get(key(root, from))!);
    files.delete(key(root, from));
  },
  fsReadDir: async (root: string) => [...files.keys()]
    .filter((k) => k.startsWith(`${root}|jobs\\`))
    .map((k) => ({ name: k.split("\\").pop()!, is_dir: false, size: 0 })),
}));

import { clearAlert, standingAlerts } from "./alerts";
import { companyRoot, loadCompany, loadJobs, parseCompany, saveCompany, saveJob } from "./store";
import { emptyCompany } from "./org";
import type { Job } from "./types";

beforeEach(() => {
  files.clear();
  for (const a of standingAlerts()) clearAlert(a.key);
});

describe("store", () => {
  it("puts the company under the home folder", () => {
    expect(companyRoot("C:\\Users\\me\\")).toBe("C:\\Users\\me\\.bytehive\\company");
  });
  it("starts an empty company when there is no file", async () => {
    expect((await loadCompany("R")).people[0].name).toBe("Founder");
  });
  it("round trips the company", async () => {
    const c = { ...emptyCompany(), nextJob: 7 };
    await saveCompany("R", c);
    expect((await loadCompany("R")).nextJob).toBe(7);
  });
  it("swaps company.json in whole, so a reader never sees half a write", async () => {
    await saveCompany("R", emptyCompany());
    const writes: string[] = [];
    const real = files.set.bind(files);
    files.set = (k, v) => { writes.push(k); return real(k, v); };
    await saveCompany("R", { ...emptyCompany(), nextJob: 7 });
    files.set = real;
    expect(writes[writes.length - 1]).toBe(key("R", "company.json"));
    expect(writes.slice(0, -1).every((k) => k !== key("R", "company.json"))).toBe(true);
    expect([...files.keys()]).toEqual([key("R", "company.json")]);
  });
  it("reads a file saved with a byte order mark, as Windows editors do", () => {
    expect(parseCompany(`﻿${JSON.stringify({ version: 1, people: [], floors: [] })}`).version).toBe(1);
  });
  it("refuses a corrupt file instead of starting over", () => {
    expect(() => parseCompany("{oops")).toThrow();
  });
  const job: Job = { id: "J-0001", title: "t", ask: "a", status: "planning", plan: null, tokenCap: 1, tokensUsed: 0, tokenBase: {}, people: [], createdAt: 1 };
  it("round trips a job", async () => {
    await saveJob("R", job);
    expect((await loadJobs("R"))[0].id).toBe("J-0001");
  });
  it("swaps a job file in whole, so a write cut short costs nothing", async () => {
    await saveJob("R", job);
    const writes: string[] = [];
    const real = files.set.bind(files);
    files.set = (k, v) => { writes.push(k); return real(k, v); };
    await saveJob("R", { ...job, title: "new" });
    files.set = real;
    const at = key("R", "jobs\\J-0001.json");
    expect(writes[0]).toBe(key("R", "jobs\\J-0001.json.tmp"));
    expect(writes[writes.length - 1]).toBe(at);
    expect(writes.slice(0, -1).every((k) => k !== at)).toBe(true);
    expect([...files.keys()]).toEqual([at]);
  });
  it("reads a job saved with a byte order mark, as Windows editors do", async () => {
    files.set(key("R", "jobs\\J-0001.json"), `\uFEFF${JSON.stringify(job)}`);
    expect((await loadJobs("R"))[0].id).toBe("J-0001");
    expect(standingAlerts()).toEqual([]);
  });
  it.each([["half written", "{ \"id\": \"J-0002\", "], ["not a job", "{}"], ["not an object", "null"]])("raises an alert for a job file that is %s, and leaves it in place", async (_what, text) => {
    await saveJob("R", job);
    files.set(key("R", "jobs\\J-0002.json"), text);
    expect((await loadJobs("R")).map((j) => j.id)).toEqual(["J-0001"]);
    expect(files.get(key("R", "jobs\\J-0002.json"))).toBe(text);
    expect(standingAlerts()).toEqual([expect.objectContaining({ level: "warn", text: expect.stringContaining("J-0002.json") })]);
  });
  it("skips a temp file a crash left behind", async () => {
    await saveJob("R", job);
    files.set(key("R", "jobs\\J-0002.json.tmp"), "{ half");
    expect((await loadJobs("R")).map((j) => j.id)).toEqual(["J-0001"]);
    expect(standingAlerts()).toEqual([]);
  });
});
