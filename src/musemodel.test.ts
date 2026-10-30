import { describe, expect, it } from "vitest";
import { createMuseChat, describeMuseTool, museQuestions } from "./musemodel";
import type { StepItem } from "./chatmodel";

/** One envelope record, shaped like the real session log. */
const rec = (id: string, ms: number, event: Record<string, unknown>): string =>
  JSON.stringify({ schema_version: 1, id, recorded_at: ms * 1000, record_type: "event", payload: { kind: "run", event } });

const macro = [
  rec("e1", 1000, { kind: "started", prompt: "fix the bug" }),
  rec("e2", 1100, { kind: "started", task_id: "t-1" }),
  rec("e3", 1200, {
    kind: "assistant_tool_calls_committed", message_id: "mm1",
    tool_calls: [{ id: "fc1", call_id: "call1", name: "read_file", args: JSON.stringify({ path: "D:/app/a.ts" }) }],
  }),
  rec("e4", 1300, {
    kind: "tool_result_batch_committed", batch_id: "mm1",
    results: [{ tool_call_index: 0, tool_call_id: "call1", text: "const x = 1;\n" }],
  }),
  rec("e5", 1400, { kind: "assistant_message_committed", message_id: "mm2", text: "Found it." }),
  rec("e6", 1500, { kind: "assistant_message_committed", message_id: "mm2", text: "Found it, fixing." }),
  rec("e7", 1600, { kind: "model_completed", usage: { input_tokens: 100, output_tokens: 20 }, model: "muse-spark-x" }),
  rec("e7", 1600, { kind: "model_completed", usage: { input_tokens: 100, output_tokens: 20 }, model: "muse-spark-x" }),
].join("\n");

describe("createMuseChat", () => {
  it("reads prompts, steps with results, replies, usage; skips run noise", () => {
    const chat = createMuseChat();
    chat.feed(macro);
    expect(chat.items.map((i) => i.kind)).toEqual(["user", "step", "text"]);
    const [u, s, t] = chat.items;
    expect(u.kind === "user" && u.text).toBe("fix the bug");
    expect(u.at).toBe(1000);
    expect(s.kind === "step" && [s.verb, s.target, s.done]).toEqual(["Read", "a.ts", true]);
    expect((s as StepItem).output).toBe("const x = 1;");
    // same message twice: updated in place, not doubled
    expect(t.kind === "text" && t.text).toBe("Found it, fixing.");
    expect(chat.items).toHaveLength(3);
    // same envelope twice: usage counted once
    expect(chat.meta.model).toBe("muse-spark-x");
    expect(chat.meta.context).toBe(100);
    expect(chat.meta.output).toBe(20);
    expect(chat.meta.turn).toBe(1);
    expect(chat.meta.started).toBe(1000);
  });

  it("shows mid-run steers but not background deliveries", () => {
    const chat = createMuseChat();
    chat.feed([
      rec("q1", 100, { kind: "inbox_item_queued", source: { source: "user_steer" }, body: "wait", payload: { prompt: "hold on", reasoning_effort: "max" } }),
      rec("q2", 200, { kind: "inbox_item_queued", source: { source: "cron" }, body: "tick", payload: { prompt: "tick" } }),
    ].join("\n"));
    expect(chat.items.map((i) => i.kind)).toEqual(["user"]);
    expect(chat.items[0].kind === "user" && chat.items[0].text).toBe("hold on");
    expect(chat.meta.effort).toBe("max");
  });

  it("skips results for calls from before the tail", () => {
    const chat = createMuseChat();
    chat.feed(rec("r1", 100, {
      kind: "tool_result_batch_committed", batch_id: "mm9",
      results: [{ tool_call_index: 0, tool_call_id: "call-old", text: "orphan" }],
    }));
    expect(chat.items).toEqual([]);
  });

  it("answers asked questions when the prompt settles", () => {
    const chat = createMuseChat();
    const questions = [{ id: "q1", header: "NEXT", question: "Which?", options: [{ label: "A", description: "first" }, { label: "B" }] }];
    chat.feed([
      rec("c1", 100, {
        kind: "assistant_tool_calls_committed", message_id: "mm3",
        tool_calls: [{ id: "fc2", call_id: "call2", name: "request_user_input", args: JSON.stringify({ questions }) }],
      }),
      rec("s1", 200, {
        kind: "user_input_prompt_settled", prompt_id: "p1", tool_call_id: "call2",
        outcome: "answered", answers: [{ id: "q1", selected_label: "B" }],
      }),
    ].join("\n"));
    const step = chat.items[0];
    expect(step.kind === "step" && step.verb).toBe("Asked you");
    expect(step.kind === "step" && step.questions?.[0].answer).toBe("B");
  });

  it("files edits under Files changed and keeps the plan", () => {
    const chat = createMuseChat();
    chat.feed([
      rec("c1", 100, {
        kind: "assistant_tool_calls_committed", message_id: "mm4",
        tool_calls: [{ id: "fc3", call_id: "call3", name: "edit_file", args: JSON.stringify({ path: "D:/app/b.ts", find: "a", replace: "b" }) }],
      }),
      rec("r1", 200, {
        kind: "tool_result_batch_committed", batch_id: "mm4",
        results: [{ tool_call_index: 0, tool_call_id: "call3", text: "edited" }],
      }),
      rec("t1", 300, { kind: "todo_snapshot_updated", revision: 2, items: [{ text: "Ship", status: "in_progress" }, { text: "Mop", status: "weird" }] }),
      "not json at all",
    ].join("\n"));
    expect(chat.meta.files.map((f) => [f.name, f.added, f.removed])).toEqual([["b.ts", 1, 1]]);
    expect(chat.meta.todos).toEqual([{ text: "Ship", state: "in_progress" }, { text: "Mop", state: "pending" }]);
  });
});

describe("describeMuseTool", () => {
  it("reads the common tools like the chat does", () => {
    expect(describeMuseTool("powershell", { command: "ls foo", description: "List foo" })).toMatchObject({ verb: "Ran", target: "List foo" });
    expect(describeMuseTool("powershell", { command: "ls foo" })).toMatchObject({ verb: "Ran", target: "ls foo", code: true });
    expect(describeMuseTool("write_file", { path: "D:/a/b.md", content: "x\ny" })).toMatchObject({ verb: "Wrote", target: "b.md", added: 2 });
    expect(describeMuseTool("subagent_spawn", { task_name: "Scout" })).toMatchObject({ verb: "Asked a helper agent to", target: "Scout" });
    expect(describeMuseTool("mcp__maestro__card_add", {})).toMatchObject({ verb: "Used", target: "maestro · card add" });
    expect(describeMuseTool("something_new", {})).toMatchObject({ verb: "Used", target: "something_new" });
  });

  it("reads questions with multi-select", () => {
    expect(museQuestions([{ id: "a", header: "H", question: "Q?", selection: { mode: "multiple" }, options: [{ label: "L" }] }]))
      .toEqual([{ question: "Q?", header: "H", multi: true, options: [{ label: "L", description: undefined }] }]);
    expect(museQuestions(null)).toEqual([]);
  });
});
