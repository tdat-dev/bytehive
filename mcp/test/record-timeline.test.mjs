// @vitest-environment node
// The extension's recording timeline (browser-extension/record/timeline.js):
// plain JS with no Chrome APIs, so it is tested here with the MCP's runner.
import { describe, it, expect } from "vitest";
import { clock, describe as line, addEvent, consoleLine, networkLine, toMarkdown, counts } from "../../browser-extension/record/timeline.js";

describe("recording timeline", () => {
  it("writes times as m:ss.d", () => {
    expect(clock(0)).toBe("0:00.0");
    expect(clock(7420)).toBe("0:07.4");
    expect(clock(723000)).toBe("12:03.0");
  });

  it("describes actions that change the screen, and skips the rest", () => {
    expect(line("navigate", { url: "https://a.com/x" })).toBe("open https://a.com/x");
    expect(line("computer", { action: "left_click", ref: "ref_3" })).toBe("left click ref_3");
    expect(line("computer", { action: "double_click", coordinate: [10.4, 20.6] })).toBe("double click (10,21)");
    expect(line("computer", { action: "type", text: "hello\nworld" })).toBe('type "hello world"');
    expect(line("computer", { action: "screenshot" })).toBe("");
    expect(line("read_page", {})).toBe("");
  });

  it("folds the same error into one line with a count", () => {
    const list = [];
    addEvent(list, 1000, "error", "TypeError: x is undefined");
    addEvent(list, 1500, "action", "left click ref_1");
    addEvent(list, 2000, "error", "TypeError: x is undefined");
    expect(list).toHaveLength(2);
    expect(list[0].n).toBe(2);
    expect(counts(list)).toEqual({ actions: 1, errors: 2 });
    expect(toMarkdown("Ana", list)).toBe("# Ana\n\n0:01.0 ERROR TypeError: x is undefined (×2)\n0:01.5 left click ref_1\n");
  });

  it("keeps only console errors and failed requests", () => {
    expect(consoleLine("log", "hi")).toBe("");
    expect(consoleLine("error", "boom")).toBe("boom");
    expect(networkLine("GET", "https://a.com/api", 200)).toBe("");
    expect(networkLine("POST", "https://a.com/api", 500)).toBe("POST https://a.com/api 500");
    expect(networkLine("GET", "https://a.com/x", 0, "net::ERR_ABORTED")).toBe("");
    expect(networkLine("GET", "https://a.com/x", 0, "net::ERR_FAILED")).toBe("GET https://a.com/x failed: net::ERR_FAILED");
  });
});
