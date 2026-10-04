import { describe, expect, it } from "vitest";
import { landedIn, liveReply, liveReplyOf, unwrap } from "./livereply";

// Screens as TerminalHandle.snapshot() gives them, captured from Claude Code v2
// in an 80-column pane: the composer between two rules, status lines under it.
const RULE = "─".repeat(80);
const FOOT = [
  RULE,
  "❯",
  RULE,
  "  [Opus 5.5 (1M context)] | # [#--------------] | 8.0% used | ~920.0k left | …",
  "  ⏵⏵ auto mode on (shift+tab to cycle) · ← 19 agents",
];
const SPIN = ["✢ Schlepping… (running PreToolUse hooks… 1/6 · 3s · ↓ 163 tokens ·", "thinking with high effort)"];
const screen = (...rows: string[]) => [...rows, ...FOOT].join("\n");

const REPLY = [
  "● I added mul(a, b) to math.js and one line to README.md, and changed nothing",
  "  else.",
  "  - math.js:9: export function mul(a, b) { return a * b; }, written the same way",
  "    as add and sub.",
  '  - README.md:4: "It can also multiply two numbers with mul(a, b)."',
];

describe("the reply Claude Code is writing, off its screen", () => {
  it("reads the last block above the spinner and joins the lines the terminal wrapped", () => {
    expect(liveReplyOf(screen("> add mul", "", ...REPLY, "", ...SPIN))).toEqual({
      text: [
        "I added mul(a, b) to math.js and one line to README.md, and changed nothing else.",
        "- math.js:9: export function mul(a, b) { return a * b; }, written the same way as add and sub.",
        '- README.md:4: "It can also multiply two numbers with mul(a, b)."',
      ].join("\n"),
      cut: false,
    });
  });

  it("is null once the turn is over: the spinner gave way to its done line", () => {
    expect(liveReply(screen(...REPLY, "✻ Baked for 14s · done 1:37 AM"))).toBeNull();
    // and null with no spinner at all (an idle prompt)
    expect(liveReply(screen(...REPLY))).toBeNull();
  });

  it("is null while the last block is a tool call, running or folded", () => {
    expect(liveReply(screen(...REPLY, "", "● Reading 1 file, running 1 shell command…", "  ⎿  $ rtk ls -la", "", ...SPIN))).toBeNull();
    expect(liveReply(screen("● Bash(npm test)", "", ...SPIN))).toBeNull();
    expect(liveReply(screen("● Reading 1 file…", "", ...SPIN))).toBeNull();
    expect(liveReply(screen("● Read 2 files, searched for 1 pattern (ctrl+o to expand)", "", ...SPIN))).toBeNull();
    expect(liveReply(screen("● maestro - board_get (MCP)(list: \"Doing\")", "", ...SPIN))).toBeNull();
    expect(liveReply(screen("● Update(src/app.ts)", "  ⎿  Updated src/app.ts with 2 additions", "", ...SPIN))).toBeNull();
  });

  it("reads text written after a tool call, and not the tool call above it", () => {
    const s = screen("● Bash(npm test)", "  ⎿  ok", "", "● The tests pass. Now the build.", "", ...SPIN);
    expect(liveReply(s)).toBe("The tests pass. Now the build.");
  });

  it("keeps the agent's own line ends and paragraphs", () => {
    const s = screen(
      "● Three things:",
      "  1. the parser",
      "  2. the view",
      "",
      "  Short line.",
      "  Another short line.",
      "",
      ...SPIN,
    );
    expect(liveReply(s)).toBe("Three things:\n1. the parser\n2. the view\n\nShort line.\nAnother short line.");
  });

  it("keeps code lines as they are", () => {
    const s = screen(
      "● Like this:",
      "  ```ts",
      "  const x = 1;",
      "    return x;",
      "  ```",
      "",
      ...SPIN,
    );
    expect(liveReply(s)).toBe("Like this:\n```ts\nconst x = 1;\n  return x;\n```");
  });

  it("shows what is visible of a long reply cut at the top, and says it is cut", () => {
    const s = screen(
      "  first visible line of a long answer that started well above the top of the",
      "  screen.",
      "  - a point",
      "",
      ...SPIN,
    );
    expect(liveReplyOf(s)).toEqual({ text: "first visible line of a long answer that started well above the top of the screen.\n- a point", cut: true });
    // a tool's output cut at the top is not text
    expect(liveReply(screen("     some output line", "     more output", "", ...SPIN))).toBeNull();
  });

  it("is null when the last thing above the spinner is your message or thinking", () => {
    expect(liveReply(screen(...REPLY, "✻ Baked for 14s · done", "", "> now add div", "", ...SPIN))).toBeNull();
    expect(liveReply(screen("> now add div", "", "∴ Thinking…", "", ...SPIN))).toBeNull();
  });

  it("reads the ⏺ bullet other systems draw", () => {
    expect(liveReply(screen("⏺ Sure.", "", "✻ Pondering… (3s · esc to interrupt)"))).toBe("Sure.");
  });

  it("is null for Codex and opencode screens, not garbage", () => {
    const codex = [
      "• I'll look at the tests first.",
      "",
      "• Ran npm test",
      "  └ ok",
      "",
      "• Working (3s • esc to interrupt)",
      "",
      "› Ask Codex to do anything",
      "",
      "  ⏎ send   ⌃J newline   ⌃T transcript",
    ].join("\n");
    expect(liveReply(codex)).toBeNull();
    const opencode = [
      "┃  Build  claude-opus-5-5",
      "┃",
      "┃  I'll add the function now.",
      "┃",
      "  ⬝⬝⬝■■■  esc interrupt",
      "┃ >",
      "  enter send",
    ].join("\n");
    expect(liveReply(opencode)).toBeNull();
    expect(liveReply("")).toBeNull();
  });
});

describe("unwrap", () => {
  it("joins a line only when its first word would not have fitted above", () => {
    expect(unwrap(["  aaaa bbbb", "  cc"], 12)).toBe("aaaa bbbb cc"); // 11 + 1 + 2 > 12
    expect(unwrap(["  aaaa", "  cc"], 12)).toBe("aaaa\ncc"); // 6 + 1 + 2 fits: a real break
  });
  it("counts wide characters as two columns", () => {
    expect(unwrap(["  中文中文", "  ab"], 12)).toBe("中文中文 ab"); // 10 + 1 + 2 > 12
  });
});

describe("landedIn", () => {
  it("finds the screen's words in the transcript's markdown", () => {
    const md = "I added `mul(a, b)` to **math.js** and see [the docs](https://example.com/docs) for more.";
    expect(landedIn("I added mul(a, b) to math.js and see the docs for more.", [md])).toBe(true);
    expect(landedIn("I added mul(a, b) to math.js and", ["Something else entirely."])).toBe(false);
    expect(landedIn("I added mul", [])).toBe(false);
  });
});
