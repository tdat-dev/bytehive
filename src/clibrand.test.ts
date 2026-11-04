import { readdirSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { BRANDS, brandOf, cliMark, commandBrand } from "./clibrand";
import { CLI_PRESETS, customPreset } from "./crew";

const key = (cmd: string) => commandBrand(cmd.split(" "))?.key ?? null;

describe("which CLI a command is", () => {
  it("knows it by its program, however it is written", () => {
    expect(key("gemini -m pro")).toBe("gemini");
    expect(key("C:\\tools\\Grok.EXE")).toBe("grok");
    expect(key("D:/bin/muse.cmd")).toBe("muse");
    expect(key("cursor-agent")).toBe("cursor");
    expect(key("ollama run llama3.1")).toBe("ollama");
    expect(key("agy")).toBe("antigravity");
  });

  it("looks through what only launches it", () => {
    expect(key("npx -y @google/gemini-cli")).toBe("gemini");
    expect(key("npx @anthropic-ai/claude-code@latest")).toBe("claude");
    expect(key("bunx @openai/codex")).toBe("codex");
    expect(key("pnpm dlx opencode-ai")).toBe("opencode");
    expect(key("uv tool run aider-chat")).toBe("aider");
    expect(key("cmd /c claude")).toBe("claude");
    expect(key("wsl codex")).toBe("codex");
    expect(key("gh copilot")).toBe("copilot");
  });

  it("does not guess", () => {
    expect(key("my-tool --x")).toBeNull();
    expect(key("gh pr list")).toBeNull();
    expect(key("npx")).toBeNull();
    expect(key("npm test")).toBeNull();
    expect(commandBrand([])).toBeNull();
  });
});

describe("an agent's brand", () => {
  it("comes from its badge, or its command when it is a custom one", () => {
    expect(brandOf({ badge: "claude", program: "whatever" })?.key).toBe("claude");
    expect(brandOf(customPreset("grok --fast")!)?.key).toBe("grok");
    expect(brandOf(customPreset("my-tool")!)).toBeNull();
  });

  it("survives a session saved without a badge", () => {
    expect(brandOf({ program: "codex" })?.key).toBe("codex");
    expect(brandOf({})).toBeNull();
  });

  it("covers every CLI the app offers, shells aside", () => {
    for (const p of CLI_PRESETS.filter((c) => !c.shell)) expect(brandOf(p), p.id).not.toBeNull();
  });
});

describe("an agent's mark", () => {
  it("is the logo of its CLI", () => {
    const mark = cliMark({ badge: "claude" });
    expect(mark).toMatch(/^<img class="cli-logo" src="[^"]*claude[^"]*" alt="" draggable="false">$/);
  });

  it("fills the tile when the logo brings its own square", () => {
    expect(cliMark({ badge: "codex" })).toContain('class="cli-logo tile"');
  });

  it("is a prompt for a shell, never a face", () => {
    for (const badge of ["shell", "cmd"]) expect(cliMark({ badge })).toContain('class="cli-glyph"');
  });

  it("is the program's first letter for a CLI nobody has a logo for", () => {
    expect(cliMark(customPreset("my-tool --x"))).toBe('<span class="cli-letter">M</span>');
    expect(cliMark(customPreset("C:\\bin\\zed.exe"))).toBe('<span class="cli-letter">Z</span>');
    expect(cliMark(customPreset("npx my-tool"))).toBe('<span class="cli-letter">M</span>');
  });

  it("copes with no agent, and keeps markup out", () => {
    expect(cliMark(undefined)).toBe('<span class="cli-letter">?</span>');
    expect(cliMark({ badge: "custom", program: '<img src=x onerror=alert(1)>' })).toBe('<span class="cli-letter">?</span>');
  });
});

describe("the logo files", () => {
  it("match the brands, one for one", () => {
    const files = readdirSync("src/assets/clis").map((f) => f.replace(/\.png$/, "")).sort();
    expect(files).toEqual(BRANDS.map((b) => b.key).sort());
  });
});
