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
  fsReadDir: async (root: string) => [...files.keys()]
    .filter((k) => k.startsWith(`${root}|jobs\\`))
    .map((k) => ({ name: k.split("\\").pop()!, is_dir: false, size: 0 })),
}));

import { companyRoot, loadCompany, loadJobs, parseCompany, saveCompany, saveJob } from "./store";
import { emptyCompany } from "./org";
import type { Job } from "./types";

beforeEach(() => files.clear());

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
  it("refuses a corrupt file instead of starting over", () => {
    expect(() => parseCompany("{oops")).toThrow();
  });
  it("round trips a job", async () => {
    const job: Job = { id: "J-0001", title: "t", ask: "a", status: "planning", plan: null, tokenCap: 1, tokensUsed: 0, tokenBase: {}, people: [], createdAt: 1 };
    await saveJob("R", job);
    expect((await loadJobs("R"))[0].id).toBe("J-0001");
  });
});
