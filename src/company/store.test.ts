import { describe, it, expect, vi, beforeEach } from "vitest";

const files = new Map<string, string>();
const readFails = new Map<string, unknown>();
const key = (root: string, rel: string) => `${root}|${rel}`;
vi.mock("../ipc", () => ({
  fsReadFile: async (root: string, rel: string) => {
    if (readFails.has(rel)) throw readFails.get(rel);
    const content = files.get(key(root, rel));
    if (content === undefined) throw { Failed: "no such path: The system cannot find the file specified. (os error 2)" };
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
import { companyRoot, pickRoot, rootParts, loadCompany, loadJobs, parseCompany, readLock, saveCompany, saveJob, writeLock } from "./store";
import { emptyCompany } from "./org";
import { DEFAULT_LIMITS, type Job } from "./types";

beforeEach(() => {
  files.clear();
  readFails.clear();
  for (const a of standingAlerts()) clearAlert(a.key);
});

describe("pickRoot", () => {
  it("is the real folder in a release build", () => {
    expect(pickRoot("C:\\Users\\me\\", { dev: false })).toBe("C:\\Users\\me\\.bytehive\\company");
  });
  it("ignores an override in a release build", () => {
    expect(pickRoot("C:\\Users\\me", { dev: false, root: "D:\\x" })).toBe("C:\\Users\\me\\.bytehive\\company");
  });
  it("is the sandbox in a dev build", () => {
    expect(pickRoot("C:\\Users\\me", { dev: true })).toBe("C:\\Users\\me\\.bytehive\\company-dev");
  });
  it("takes an override in a dev build, without a trailing slash", () => {
    expect(pickRoot("C:\\Users\\me", { dev: true, root: "D:\\scratch\\co\\" })).toBe("D:\\scratch\\co");
  });
  it("writes the override with backslashes", () => {
    expect(pickRoot("C:\\Users\\me", { dev: true, root: "D:/scratch/co/" })).toBe("D:\\scratch\\co");
  });
  it("never lets a dev build use the real company, in any spelling", () => {
    const sandbox = "C:\\Users\\me\\.bytehive\\company-dev";
    for (const root of [
      "C:\\Users\\me\\.bytehive\\company",
      "C:\\Users\\me\\.bytehive\\company\\",
      "c:/users/ME/.ByteHive/Company",
      "C:\\Users\\me\\.bytehive\\company\\people\\x",
      "C:/Users/me/.bytehive/company/jobs",
      "C:\\Users\\me\\.bytehive\\scratch\\..\\company",
      "C:\\Users\\me\\.bytehive\\.\\company",
    ]) expect(pickRoot("C:\\Users\\me", { dev: true, root })).toBe(sandbox);
  });
  it("keeps a sibling whose name only starts like the real folder", () => {
    expect(pickRoot("C:\\Users\\me", { dev: true, root: "C:\\Users\\me\\.bytehive\\company-x" })).toBe("C:\\Users\\me\\.bytehive\\company-x");
  });
  it("falls back to the sandbox for a path that is not absolute", () => {
    expect(pickRoot("C:\\Users\\me", { dev: true, root: "scratch\\co" })).toBe("C:\\Users\\me\\.bytehive\\company-dev");
  });
});

describe("rootParts", () => {
  it("splits a drive path into the drive and the folders to create", () => {
    expect(rootParts("D:\\scratch\\co")).toEqual({ base: "D:\\", rel: "scratch\\co" });
  });
  it("splits a share path after the share", () => {
    expect(rootParts("\\\\srv\\sh\\a\\b")).toEqual({ base: "\\\\srv\\sh\\", rel: "a\\b" });
  });
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
  it("fills in the limits a file does not have, and keeps the ones it does", () => {
    const c = parseCompany(JSON.stringify({ version: 1, people: [], floors: [], limits: { peopleTotal: 3 } }));
    expect(c.limits).toEqual({ ...DEFAULT_LIMITS, peopleTotal: 3 });
    expect(parseCompany(JSON.stringify({ version: 1, people: [], floors: [] })).limits).toEqual(DEFAULT_LIMITS);
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
  it("round trips the engine lock, whole, and reads a missing one as none", async () => {
    expect(await readLock("R")).toBeNull();
    await writeLock("R", { pid: 7, at: 5 });
    expect(await readLock("R")).toEqual({ pid: 7, at: 5 });
    expect([...files.keys()]).toEqual([key("R", "engine.lock")]);
  });
  it.each([
    ["the file is being swapped in", { Failed: "no such path: Access is denied. (os error 5)" }],
    ["something has it open", { Failed: "The process cannot access the file because it is being used by another process. (os error 32)" }],
    ["the app could not be asked", "ipc gone"],
  ])("reads the lock as unknown, not as none, when %s", async (_what, e) => {
    await writeLock("R", { pid: 7, at: 5 });
    readFails.set("engine.lock", e);
    expect(await readLock("R")).toBe("unknown");
  });
  it("reads a lock a crash filled with zeros as none: no later read would ever do better", async () => {
    files.set(key("R", "engine.lock"), "\0\0\0\0");
    readFails.set("engine.lock", { Failed: "binary file" });
    expect(await readLock("R")).toBeNull();
  });
  it("does not start an empty company over one it could not read", async () => {
    readFails.set("company.json", { Failed: "no such path: Access is denied. (os error 5)" });
    await expect(loadCompany("R")).rejects.toEqual({ Failed: expect.stringContaining("os error 5") });
  });
  it("skips a temp file a crash left behind", async () => {
    await saveJob("R", job);
    files.set(key("R", "jobs\\J-0002.json.tmp"), "{ half");
    expect((await loadJobs("R")).map((j) => j.id)).toEqual(["J-0001"]);
    expect(standingAlerts()).toEqual([]);
  });
});
