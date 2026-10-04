import { beforeEach, describe, expect, it, vi } from "vitest";

// What the agent was sent, line by line, instead of a real process.
const io = vi.hoisted(() => ({ written: [] as Array<{ id: string; data: string }>, route: null as null | ((id: string) => unknown) }));
vi.mock("./ipc", () => ({
  writeRaw: async (id: string, data: string) => { io.written.push({ id, data }); },
  setInputRouter: (fn: (id: string) => unknown) => { io.route = fn; },
}));

import {
  routeStreamInput,
  answerPending, answerPendingText, askOf, closeStream, dropStream, feedStream, interruptStream, openStream, sendToStream, streamFacts,
} from "./streamagent";
import type { InputRoute } from "./ipc";

const enc = new TextEncoder();
const feed = (id: string, ...msgs: object[]) => feedStream(id, enc.encode(msgs.map((m) => JSON.stringify(m)).join("\n") + "\n"));
const sent = () => io.written.map((w) => JSON.parse(w.data));
const last = () => { const all = sent(); return all[all.length - 1]; };
const delta = (text: string) => ({ type: "stream_event", event: { type: "content_block_delta", delta: { type: "text_delta", text } } });
const permission = (id: string, tool: string, input: object, suggestions: object[] = []) =>
  ({ type: "control_request", request_id: id, request: { subtype: "can_use_tool", tool_name: tool, input, description: "", permission_suggestions: suggestions } });

routeStreamInput();

describe("streamagent", () => {
  let log: string[];
  beforeEach(() => {
    io.written = [];
    log = [];
    dropStream("a");
    openStream("a", { sessionId: "s1", log: (t) => log.push(t) });
  });

  it("starts by asking the CLI to initialize", () => {
    expect(sent()[0]).toMatchObject({ type: "control_request", request: { subtype: "initialize" } });
  });

  it("sends a message as one JSON line and works until its result", async () => {
    expect(await sendToStream("a", "hello")).toBe(true);
    expect(sent()[1]).toEqual({ type: "user", message: { role: "user", content: "hello" }, parent_tool_use_id: null, session_id: "s1" });
    expect(streamFacts("a")?.working).toBe(true);
    feed("a", { type: "result", is_error: false, stop_reason: "end_turn", duration_ms: 1200 });
    expect(streamFacts("a")?.working).toBe(false);
  });

  it("queued messages keep it working until the last turn ends", async () => {
    await sendToStream("a", "one");
    await sendToStream("a", "two");
    feed("a", { type: "result", is_error: false, stop_reason: "end_turn" });
    expect(streamFacts("a")?.working).toBe(true);
    feed("a", { type: "result", is_error: false, stop_reason: "end_turn" });
    expect(streamFacts("a")?.working).toBe(false);
  });

  it("builds the reply as it is written, even across split lines", async () => {
    await sendToStream("a", "hi");
    feed("a", { type: "stream_event", event: { type: "content_block_start", content_block: { type: "text", text: "" } } }, delta("Hel"));
    const half = JSON.stringify(delta("lo there"));
    feedStream("a", enc.encode(half.slice(0, 20)));
    expect(streamFacts("a")?.live).toBe("Hel");
    feedStream("a", enc.encode(half.slice(20) + "\n"));
    expect(streamFacts("a")?.live).toBe("Hello there");
    feed("a", { type: "result", is_error: false, stop_reason: "end_turn" });
    expect(streamFacts("a")?.live).toBe("");
  });

  it("a permission request is something it waits on; Yes allows it", () => {
    feed("a", permission("r9", "Bash", { command: "npm test" }));
    const f = streamFacts("a")!;
    expect(f.ask).toMatchObject({ kind: "run", detail: "npm test" });
    expect(f.ask!.options.map((o) => o.label)).toEqual(["Yes", "No, and tell Claude what to do instead"]);
    expect(answerPending("a", 1)).toBe(true);
    expect(last()).toEqual({ type: "control_response", response: { subtype: "success", request_id: "r9", response: { behavior: "allow", updatedInput: { command: "npm test" } } } });
    expect(streamFacts("a")?.ask).toBeNull();
  });

  it("'don't ask again' passes its suggestions on; No and your own words deny", () => {
    const sugg = [{ type: "addRules", rules: [{ toolName: "Bash" }] }];
    feed("a", permission("r1", "Edit", { file_path: "a.ts" }, sugg));
    expect(streamFacts("a")!.ask).toMatchObject({ kind: "edit", detail: "a.ts" });
    answerPending("a", 2);
    expect(last().response.response).toEqual({ behavior: "allow", updatedInput: { file_path: "a.ts" }, updatedPermissions: sugg });
    feed("a", permission("r2", "Bash", { command: "rm -rf x" }));
    answerPendingText("a", "use the trash instead");
    expect(last().response.response).toEqual({ behavior: "deny", message: "use the trash instead" });
  });

  it("asks each of several questions in turn and answers them together", () => {
    feed("a", permission("q1", "AskUserQuestion", { questions: [
      { question: "Color?", header: "Color", options: [{ label: "Red" }, { label: "Blue" }] },
      { question: "Size?", options: [{ label: "S" }, { label: "L" }] },
    ] }));
    expect(streamFacts("a")!.ask).toMatchObject({ kind: "question", prompt: "Color?", title: "Color" });
    answerPending("a", 2);
    expect(streamFacts("a")!.ask).toMatchObject({ prompt: "Size?" });
    const before = io.written.length;
    answerPendingText("a", "Medium, please");
    expect(io.written.length).toBe(before + 1);
    expect(last().response.response.updatedInput.answers).toEqual({ "Color?": "Blue", "Size?": "Medium, please" });
    expect(streamFacts("a")!.ask).toBeNull();
  });

  it("Esc through the input route stops it, and a number answers what it waits on", async () => {
    const route = io.route!("a") as InputRoute;
    expect(io.route!("nobody")).toBeNull();
    feed("a", permission("r3", "Bash", { command: "ls" }));
    await route.key("1");
    expect(last().response.request_id).toBe("r3");
    await sendToStream("a", "go");
    void route.key("\x1b");
    expect(last()).toMatchObject({ type: "control_request", request: { subtype: "interrupt" } });
  });

  it("stopping answers what it waited on with no", async () => {
    await sendToStream("a", "go");
    feed("a", permission("r4", "Bash", { command: "ls" }));
    void interruptStream("a");
    const responses = sent().filter((m) => m.type === "control_response");
    expect(responses[responses.length - 1].response.response).toMatchObject({ behavior: "deny", interrupt: true });
  });

  it("an error result and stderr say what went wrong; an exit clears what it waited on", async () => {
    await sendToStream("a", "go");
    feed("a", { type: "result", is_error: true, stop_reason: "end_turn", result: "API Error: overloaded" });
    expect(streamFacts("a")?.problem).toBe("API Error: overloaded");
    feed("a", { type: "maestro_stderr", text: "Error: not logged in" }, permission("r5", "Bash", { command: "ls" }));
    closeStream("a", 1);
    expect(streamFacts("a")).toMatchObject({ working: false, ask: null, problem: "API Error: overloaded" });
  });

  it("a usage limit says when it lifts", () => {
    feed("a", { type: "rate_limit_event", rate_limit_info: { status: "rejected", resetsAt: 1791140400 } });
    expect(streamFacts("a")?.limit).toEqual({ what: "Claude's usage limit is reached", resetsAt: 1791140400000 });
  });

  it("logs the exchange for its Terminal tab", async () => {
    await sendToStream("a", "hello");
    feed("a", { type: "assistant", message: { content: [{ type: "text", text: "Hi\nthere" }, { type: "tool_use", name: "Bash", input: { command: "ls -la" } }] } });
    const all = log.join("");
    expect(all).toContain("› hello");
    expect(all).toContain("Hi\r\nthere");
    expect(all).toContain("● Bash ls -la");
  });
});

describe("askOf", () => {
  it("a tool that isn't a command or an edit is named", () => {
    expect(askOf("WebFetch", { url: "x" }, "", [])).toMatchObject({ kind: "run", detail: "WebFetch", prompt: "Claude wants to use WebFetch" });
  });
});
