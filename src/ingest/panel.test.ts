// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { BlockedList } from "./blocked";
import type { RecentResult } from "./index";
import { answerText, createIngestPanel, senderName, stamp, todayLine, type PanelApi } from "./panel";
import type { LogStats } from "./log";
import type { BlockedEntry, ToolResult } from "./types";

vi.mock("./index", () => ({}));

const stats = (over: Partial<LogStats> = {}): LogStats => ({ messages: 0, cacheHits: 0, blocked: 0, calls: 0, tokens: 0, saved: 0, ...over });
const held = (id: string, at: number): BlockedEntry => ({ id, hash: "h", source: "fleet", from: "Ana", to: "p1", toName: "Bob", text: `text ${id}`, reason: "smalltalk", at });
const cost: ToolResult = { status: "ok", values: { usd: 0.54 }, timestamp: 1, metadata: { tool: "estimate_cost", version: "1", cacheable: true } };

function setup(over: Partial<PanelApi> = {}, entries: BlockedEntry[] = [], recent: RecentResult[] = []) {
  let items = entries;
  const api: PanelApi = {
    enabled: () => true,
    list: async (): Promise<BlockedList> => ({ entries: items, damaged: false }),
    stats: async () => stats(),
    recent: () => recent,
    resend: async (_d, e) => {
      items = items.filter((x) => x.id !== e.id);
      return true;
    },
    remove: async (_d, id) => {
      const hit = items.find((x) => x.id === id) ?? null;
      items = items.filter((x) => x.id !== id);
      return hit;
    },
    restore: async (_d, e) => {
      items = [...items, e].sort((a, b) => a.at - b.at);
    },
    subscribe: () => () => {},
    openSettings: vi.fn(),
    ...over,
  };
  const body = document.createElement("div");
  document.body.replaceChildren(body);
  const panel = createIngestPanel(api.openSettings, api);
  panel.mount(body);
  return { api, body, panel };
}

const settle = () => new Promise((r) => setTimeout(r, 0));

async function opened(t: ReturnType<typeof setup>, dir: string | null = "D:/p") {
  t.panel.setContext(dir ? { key: dir, dir } : null);
  t.panel.show();
  await settle();
}

const press = (body: HTMLElement, label: string, nth = 0) =>
  [...body.querySelectorAll("button")].filter((b) => b.textContent === label)[nth]?.click();

beforeEach(() => document.body.replaceChildren());

describe("the words", () => {
  it("says the day in one sentence, in the right number", () => {
    expect(todayLine(stats())).toBe("No messages through the gate today.");
    expect(todayLine(stats({ messages: 1, calls: 1, tokens: 1 }))).toBe("Today: 1 message, 0 were held, 0 from cache, 1 classifier call using 1 token.");
    expect(todayLine(stats({ messages: 14, blocked: 3, cacheHits: 2, calls: 5, tokens: 1840 }))).toBe(
      "Today: 14 messages, 3 were held, 2 from cache, 5 classifier calls using 1,840 tokens.",
    );
  });

  it("reads a cost as money and a failure as a failure", () => {
    expect(answerText(cost)).toBe("$0.54");
    expect(answerText({ ...cost, status: "error", error: "unknown model" })).toBe("Failed: unknown model");
  });
});

describe("the row labels", () => {
  it("only calls a person's own sources You", () => {
    expect(senderName(undefined, "kanban")).toBe("You");
    expect(senderName(undefined, "phone")).toBe("You");
    expect(senderName(undefined, "fleet")).toBe("An agent");
    expect(senderName("Ana", "fleet")).toBe("Ana");
  });

  it("dates a message held on another day", () => {
    const now = new Date(2026, 10, 7, 14, 0, 0).getTime();
    expect(stamp(new Date(2026, 10, 7, 9, 5, 1).getTime(), now)).toBe("09:05:01");
    expect(stamp(new Date(2026, 10, 5, 9, 5, 1).getTime(), now)).toBe("Nov 5 09:05:01");
  });
});

describe("the panel", () => {
  it("asks for a project when there is none", async () => {
    const t = setup();
    await opened(t, null);
    expect(t.body.textContent).toContain("Open a project");
  });

  it("says when nothing is held back", async () => {
    const t = setup();
    await opened(t);
    expect(t.body.textContent).toContain("Nothing held back.");
    expect(t.body.querySelector(".igp-h")).toBeNull();
  });

  it("explains the switch being off and leads to Settings", async () => {
    const t = setup({ enabled: () => false });
    await opened(t);
    expect(t.body.textContent).toContain("The gate is off");
    press(t.body, "Open Settings");
    expect(t.api.openSettings).toHaveBeenCalled();
  });

  it("shows held messages oldest first like Flow, so a new one never moves a row under the pointer", async () => {
    const t = setup({}, [held("a", 1), held("b", 2)]);
    await opened(t);
    const rows = [...t.body.querySelectorAll<HTMLElement>(".igp-held .flow-row")];
    expect(rows.map((r) => r.dataset.id)).toEqual(["a", "b"]);
    expect(rows[0].textContent).toContain("Ana");
    expect(rows[0].textContent).toContain("Bob");
    expect(rows[0].textContent).toContain("Small talk");
  });

  it("sends a message anyway, removes the row and moves focus to the next one", async () => {
    const t = setup({}, [held("a", 1), held("b", 2)]);
    await opened(t);
    press(t.body, "Send anyway");
    await settle();
    await settle();
    expect([...t.body.querySelectorAll<HTMLElement>(".igp-held .flow-row")].map((r) => r.dataset.id)).toEqual(["b"]);
    expect(t.body.textContent).toContain("Sent to Bob.");
    expect(document.activeElement?.textContent).toBe("Send anyway");
  });

  it("keeps the row and says so when the agent can't be reached", async () => {
    const t = setup({ resend: async () => false }, [held("a", 1)]);
    await opened(t);
    press(t.body, "Send anyway");
    await settle();
    await settle();
    expect(t.body.querySelectorAll(".igp-held .flow-row")).toHaveLength(1);
    expect(t.body.textContent).toContain("Couldn't reach Bob");
  });

  it("deletes a row and can bring it back", async () => {
    const t = setup({}, [held("a", 1)]);
    await opened(t);
    press(t.body, "Delete");
    await settle();
    await settle();
    expect(t.body.querySelectorAll(".igp-held .flow-row")).toHaveLength(0);
    press(t.body, "Undo");
    await settle();
    await settle();
    expect(t.body.querySelectorAll(".igp-held .flow-row")).toHaveLength(1);
    expect(t.body.textContent).toContain("Message restored.");
  });

  it("offers another try when the files can't be read", async () => {
    let fail = true;
    const t = setup({
      list: async () => {
        if (fail) throw new Error("io");
        return { entries: [], damaged: false };
      },
    });
    await opened(t);
    expect(t.body.textContent).toContain("Couldn't read");
    fail = false;
    press(t.body, "Try again");
    await settle();
    expect(t.body.textContent).toContain("Nothing held back.");
  });

  it("does not call a damaged list empty or count it", async () => {
    const t = setup({ list: async () => ({ entries: [], damaged: true }) });
    await opened(t);
    expect(t.body.textContent).toContain("damaged");
    expect(t.body.textContent).not.toContain("Nothing held back.");
    expect(t.body.textContent).not.toContain("Held back (0)");
  });

  it("sends once however many times the button is pressed while it is working", async () => {
    let finish = (_ok: boolean): void => {};
    const resend = vi.fn(() => new Promise<boolean>((r) => (finish = r)));
    const t = setup({ resend }, [held("a", 1)]);
    await opened(t);
    press(t.body, "Send anyway");
    press(t.body, "Send anyway");
    press(t.body, "Delete");
    await settle();
    expect(resend).toHaveBeenCalledTimes(1);
    expect(t.body.textContent).toContain("Sending to Bob");
    finish(true);
    await settle();
    await settle();
    expect(resend).toHaveBeenCalledTimes(1);
    expect(t.body.textContent).toContain("Sent to Bob.");
  });

  it("after Delete, focus lands on the next Delete, never on a send", async () => {
    const t = setup({}, [held("a", 1), held("b", 2)]);
    await opened(t);
    press(t.body, "Delete");
    await settle();
    await settle();
    expect(document.activeElement?.textContent).toBe("Delete");
  });

  it("after the last Delete, focus lands on Undo", async () => {
    const t = setup({}, [held("a", 1)]);
    await opened(t);
    press(t.body, "Delete");
    await settle();
    await settle();
    expect(document.activeElement?.textContent).toBe("Undo");
  });

  it("keeps focus on the row when a send fails", async () => {
    const t = setup({ resend: async () => false }, [held("a", 1)]);
    await opened(t);
    press(t.body, "Send anyway");
    await settle();
    await settle();
    expect(document.activeElement?.textContent).toBe("Send anyway");
  });

  it("puts focus back on the restored row after Undo", async () => {
    const t = setup({}, [held("a", 1)]);
    await opened(t);
    press(t.body, "Delete");
    await settle();
    await settle();
    press(t.body, "Undo");
    await settle();
    await settle();
    expect(document.activeElement?.closest<HTMLElement>(".flow-row")?.dataset.id).toBe("a");
  });

  it("Undo brings back every message deleted in a row", async () => {
    const t = setup({}, [held("a", 1), held("b", 2)]);
    await opened(t);
    press(t.body, "Delete");
    await settle();
    await settle();
    press(t.body, "Delete");
    await settle();
    await settle();
    expect(t.body.textContent).toContain("2 messages deleted.");
    press(t.body, "Undo");
    await settle();
    await settle();
    expect(t.body.querySelectorAll(".igp-held .flow-row")).toHaveLength(2);
  });

  it("keeps Undo when the panel is closed and opened again", async () => {
    const t = setup({}, [held("a", 1)]);
    await opened(t);
    press(t.body, "Delete");
    await settle();
    await settle();
    t.panel.hide();
    t.panel.show();
    await settle();
    press(t.body, "Undo");
    await settle();
    await settle();
    expect(t.body.querySelectorAll(".igp-held .flow-row")).toHaveLength(1);
  });

  it("says so when Undo fails, and lets you try again", async () => {
    let fail = true;
    const restore = vi.fn(async () => {
      if (fail) throw new Error("disk");
    });
    const t = setup({ restore }, [held("a", 1)]);
    await opened(t);
    press(t.body, "Delete");
    await settle();
    await settle();
    press(t.body, "Undo");
    await settle();
    await settle();
    expect(t.body.textContent).toContain("Couldn't restore");
    expect(document.activeElement?.textContent).toBe("Undo");
    fail = false;
    press(t.body, "Undo");
    await settle();
    await settle();
    expect(restore).toHaveBeenCalledTimes(2);
    expect(t.body.textContent).toContain("Message restored.");
  });

  it("a live redraw keeps focus and opened text", async () => {
    let fire = (): void => {};
    const t = setup({ subscribe: (fn) => ((fire = fn), () => {}) }, [held("a", 1), held("b", 2)]);
    await opened(t);
    const text = t.body.querySelector<HTMLElement>(".igp-held .flow-text") as HTMLElement;
    text.click();
    text.focus();
    fire();
    await settle();
    await settle();
    const after = t.body.querySelector<HTMLElement>(".igp-held .flow-text") as HTMLElement;
    expect(after.classList.contains("full")).toBe(true);
    expect(after.getAttribute("aria-expanded")).toBe("true");
    expect(document.activeElement).toBe(after);
  });

  it("lists answers with where they came from", async () => {
    const r: RecentResult = { at: 1, source: "kanban", to: "p1", toName: "Bob", text: "giá?", result: cost, cached: true };
    const t = setup({}, [], [r]);
    await opened(t);
    expect(t.body.textContent).toContain("Answered by code (1)");
    expect(t.body.textContent).toContain("Cost estimate");
    expect(t.body.textContent).toContain("$0.54");
    expect(t.body.textContent).toContain("from cache, 0 tokens");
  });

  it("redraws when the gate reports a change, only while open", async () => {
    let fire = (): void => {};
    const list = vi.fn(async () => ({ entries: [], damaged: false }));
    const t = setup({ list, subscribe: (fn) => ((fire = fn), () => {}) });
    fire();
    await settle();
    expect(list).not.toHaveBeenCalled();
    await opened(t);
    const before = list.mock.calls.length;
    fire();
    await settle();
    expect(list.mock.calls.length).toBe(before + 1);
  });
});
