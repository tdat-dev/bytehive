import { beforeEach, describe, expect, it, vi } from "vitest";

const disk = vi.hoisted(() => ({ files: new Map<string, { content: string; mtime: number }>(), clock: 1 }));
vi.mock("./ipc", () => ({
  fsCreateDir: async () => {},
  fsCreateFile: async () => {},
  fsReadFile: async (_root: string, path: string) => {
    const f = disk.files.get(path);
    if (!f) throw new Error("not found");
    return f;
  },
  fsStat: async (_root: string, path: string) => {
    const f = disk.files.get(path);
    if (!f) throw new Error("not found");
    return { mtime: f.mtime };
  },
  fsWriteFile: async (_root: string, path: string, content: string, expected: number | null) => {
    const f = disk.files.get(path);
    if (expected !== null && f && f.mtime !== expected) throw new Error(`Conflict(${f.mtime})`);
    const mtime = ++disk.clock;
    disk.files.set(path, { content, mtime });
    return { mtime };
  },
}));

import { openMemory, fixPrompt } from "./memoryview";

const FILE = ".bytehive\\memory.json";
const tick = () => new Promise((r) => setTimeout(r, 0));
const entries = () => JSON.parse(disk.files.get(FILE)!.content).entries;

describe("project memory panel", () => {
  beforeEach(() => {
    document.body.innerHTML = "";
    disk.files.clear();
  });

  it("puts the keyboard in the first field at once, without waiting for the file", () => {
    openMemory({ dir: "D:/p", project: "zoldify", agents: () => [], send: async () => {} } as never);
    expect((document.activeElement as HTMLElement | null)?.id).toBe("mmNew");
  });

  it("adds, edits, deletes and puts back a fact", async () => {
    const sent: [string, string][] = [];
    openMemory({ dir: "D:/p", project: "zoldify", agents: () => [], send: async (id, t) => { sent.push([id, t]); return true; } });
    await tick();
    expect(document.querySelector(".mm-empty")!.textContent).toContain("zoldify");
    (document.querySelector("#mmNew") as HTMLTextAreaElement).value = "Tests run with npx vitest run";
    document.querySelector<HTMLFormElement>(".mm-add")!.requestSubmit();
    await tick(); await tick();
    expect(entries()).toMatchObject([{ id: "m1", text: "Tests run with npx vitest run", by: "you" }]);
    expect(document.querySelector(".mm-t")!.textContent).toBe("Tests run with npx vitest run");

    document.querySelector<HTMLButtonElement>('[data-id="m1"] [data-act="edit"]')!.click();
    (document.querySelector("#mmEdit") as HTMLTextAreaElement).value = "Tests: npm test";
    document.querySelector<HTMLFormElement>(".mm-edit")!.requestSubmit();
    await tick(); await tick();
    expect(entries()[0].text).toBe("Tests: npm test");

    document.querySelector<HTMLButtonElement>('[data-id="m1"] [data-act="delete"]')!.click();
    await tick(); await tick();
    expect(entries()).toEqual([]);
    document.querySelector<HTMLButtonElement>('[data-act="undo"]')!.click();
    await tick(); await tick();
    expect(entries().map((e: { text: string }) => e.text)).toEqual(["Tests: npm test"]);
  });

  it("says why a secret is refused", async () => {
    openMemory({ dir: "D:/p", project: "p", agents: () => [], send: async () => true });
    await tick();
    (document.querySelector("#mmNew") as HTMLTextAreaElement).value = "token: ghp_abcdefghijklmnopqrstuvwxyz0123";
    document.querySelector<HTMLFormElement>(".mm-add")!.requestSubmit();
    await tick(); await tick();
    expect(document.querySelector(".mm-err")!.textContent).toMatch(/secrets/);
  });

  it("sends a fact to the chosen agent to fix, with what you said", async () => {
    disk.files.set(FILE, { content: JSON.stringify({ version: 1, entries: [{ id: "m4", text: "Deploys go through make ship", at: 1, by: "Ana" }] }), mtime: 1 });
    const sent: [string, string][] = [];
    openMemory({ dir: "D:/p", project: "p", agents: () => [{ id: "pa", name: "Ana" }, { id: "pb", name: "Bob" }], send: async (id, t) => { sent.push([id, t]); return true; } });
    await tick();
    document.querySelector<HTMLButtonElement>('[data-id="m4"] [data-act="ask"]')!.click();
    (document.querySelector("#mmNote") as HTMLTextAreaElement).value = "we use the\nGitHub workflow now";
    (document.querySelector("#mmTo") as HTMLSelectElement).value = "pb";
    document.querySelector<HTMLFormElement>(".mm-ask")!.requestSubmit();
    await tick();
    expect(sent).toEqual([["pb", fixPrompt({ id: "m4", text: "Deploys go through make ship", at: 1, by: "Ana" }, "we use the GitHub workflow now")]]);
    expect(sent[0][1]).toContain("[m4]");
    expect(sent[0][1]).toContain("The user says: we use the GitHub workflow now.");
    expect(sent[0][1]).not.toContain("\n");
    expect(document.querySelector(".mm-status")!.textContent).toContain("Sent to Bob");
  });

  it("switches to another project's memory from its tab", async () => {
    const went: string[] = [];
    openMemory({ dir: "D:/p", project: "bytehive", agents: () => [], send: async () => true,
      projects: [{ id: "w1", name: "bytehive" }, { id: "w2", name: "zoldify" }], current: "w1", switchTo: (id) => went.push(id) });
    await tick();
    expect([...document.querySelectorAll(".mm-tab")].map((b) => [b.textContent, b.getAttribute("aria-current")])).toEqual([["bytehive", "true"], ["zoldify", null]]);
    document.querySelector<HTMLButtonElement>('[data-proj-tab="w2"]')!.click();
    expect(went).toEqual(["w2"]);
    expect(document.querySelector(".mm-back")).toBeNull();
  });

  it("tells you to start an agent when none of the project's is running", async () => {
    disk.files.set(FILE, { content: JSON.stringify({ version: 1, entries: [{ id: "m1", text: "a fact", at: 1, by: "you" }] }), mtime: 1 });
    openMemory({ dir: "D:/p", project: "quy", agents: () => [], send: async () => true });
    await tick();
    document.querySelector<HTMLButtonElement>('[data-act="ask-all"]')!.click();
    expect(document.querySelector(".mm-askall .mm-note")!.textContent).toContain("Start an agent in quy");
  });
});
