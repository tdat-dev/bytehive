import { describe, expect, it } from "vitest";
import { createChat } from "./chatmodel";
import { handoffBrief } from "./handoff";

const T = "2026-09-27T10:00:00.000Z";
const j = (o: object) => JSON.stringify({ timestamp: T, ...o }) + "\n";
const user = (text: string) => j({ type: "user", origin: { kind: "human" }, message: { content: text } });
const said = (text: string, id = "m" + Math.random()) => j({ type: "assistant", message: { id, content: [{ type: "text", text }] } });

describe("a handoff brief", () => {
  it("carries the task, the plan, the files and where it got to", () => {
    const chat = createChat();
    chat.feed(
      user("Make the header sticky and add a dark mode toggle") +
      j({ type: "assistant", message: { id: "a1", content: [{ type: "tool_use", id: "t1", name: "TodoWrite", input: { todos: [
        { content: "Sticky header", status: "completed", activeForm: "x" },
        { content: "Dark mode toggle", status: "in_progress", activeForm: "x" },
        { content: "Tests", status: "pending", activeForm: "x" },
      ] } }] } }) +
      j({ type: "assistant", message: { id: "a2", content: [{ type: "tool_use", id: "t2", name: "Edit", input: { file_path: "D:/app/src/header.css", old_string: "a", new_string: "b" } }] } }) +
      j({ type: "user", message: { content: [{ type: "tool_result", tool_use_id: "t2", content: "ok" }] } }) +
      said("The header is sticky now. Next I'll wire the toggle into the settings menu."),
    );
    const brief = handoffBrief({ name: "Ana", cli: "Claude Code", why: "it hit its usage limit" }, chat);
    expect(brief).toContain("taking over a task from Ana (Claude Code)");
    expect(brief).toContain("It stopped because it hit its usage limit");
    expect(brief).toContain("1. Make the header sticky and add a dark mode toggle");
    expect(brief).toContain("- [done] Sticky header\n- [doing] Dark mode toggle\n- [to do] Tests");
    expect(brief).toContain("- D:/app/src/header.css");
    expect(brief).toContain("> The header is sticky now.");
    expect(brief).toContain("Don't redo what's already done");
  });

  it("keeps the first ask and the latest ones of a long conversation", () => {
    const chat = createChat();
    chat.feed(Array.from({ length: 10 }, (_, k) => user(`ask number ${k + 1}`)).join(""));
    const brief = handoffBrief({ name: "Bob", cli: "Codex", why: "you asked" }, chat);
    expect(brief).toContain("(the first, then the latest 5 of 10)");
    expect(brief).toContain("1. ask number 1\n2. ask number 6");
    expect(brief).not.toContain("ask number 5\n");
  });

  it("works from the task alone when the conversation can't be read", () => {
    const brief = handoffBrief({ name: "Cid", cli: "Aider", why: "it hit its usage limit", title: "Fix the login redirect" }, null);
    expect(brief).toContain("Its task: Fix the login redirect");
    expect(brief).toContain("look at the folder first");
  });

  it("adds the files git says changed, capped", () => {
    const brief = handoffBrief({ name: "Ana", cli: "Claude Code", why: "it stopped", title: "T", gitFiles: ["a.ts", "b.ts"] }, null);
    expect(brief).toContain("Changed on disk (git, 2 files)");
    expect(brief).toContain("- a.ts");
    const many = handoffBrief({ name: "A", cli: "C", why: "x", gitFiles: Array.from({ length: 40 }, (_, i) => `f${i}.ts`) }, null);
    expect(many).toContain("and 10 more");
    expect(many).not.toContain("f39.ts");
  });
});
