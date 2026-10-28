import { describe, expect, it } from "vitest";
import { cliLogo } from "./clilogos";
import { CLI_PRESETS } from "./crew";

describe("cliLogo", () => {
  it("gives every built-in CLI a titled svg mark", () => {
    for (const p of CLI_PRESETS) {
      const svg = cliLogo(p.badge);
      expect(svg).toContain('<svg class="cli-mark"');
      expect(svg.endsWith("</svg>")).toBe(true);
      expect(svg).toMatch(/<title>[^<]+<\/title>/);
    }
  });

  it("covers the custom row too", () => {
    expect(cliLogo("custom")).toContain("<title>Custom CLI</title>");
  });

  it("falls back to the generic mark for unknown badges", () => {
    const svg = cliLogo("deepseek-future");
    expect(svg).toContain("<title>Agent CLI</title>");
    expect(svg).toContain("<circle");
  });

  it("falls back instead of crashing when the badge is missing", () => {
    expect(cliLogo(undefined)).toContain("<title>Agent CLI</title>");
    expect(cliLogo(null)).toContain("<title>Agent CLI</title>");
    expect(cliLogo("  ")).toContain("<title>Agent CLI</title>");
  });

  it("never interpolates the badge (injection-safe by construction)", () => {
    const svg = cliLogo('"><script>alert(1)</script>');
    expect(svg).not.toContain("<script>");
    expect(svg).not.toContain("alert(1)");
    expect(svg).toContain("<title>Agent CLI</title>");
  });

  it("differs per agent CLI (the two shells share one)", () => {
    const agents = ["claude", "codex", "gemini", "cursor", "opencode", "copilot", "qwen", "aider", "goose", "custom"];
    expect(new Set(agents.map(cliLogo)).size).toBe(agents.length);
    expect(cliLogo("shell")).toBe(cliLogo("cmd"));
  });
});
