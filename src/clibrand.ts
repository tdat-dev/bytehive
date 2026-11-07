// Each agent wears the logo of the CLI it runs, worked out from what it runs:
// its preset's badge, else the command itself, so `npx @google/gemini-cli` or
// `C:\tools\grok.exe` typed as a custom agent find their logo too. A new CLI is
// one row here and one 128px PNG in assets/clis. Logos are lobe-icons (MIT),
// Aider's its own. Pure: returns markup, no DOM.

export interface Brand {
  /** The logo's file name in assets/clis, and the badge of a CLI preset. */
  key: string;
  /** Program and package names that mean this CLI. */
  names: string[];
  /** The tile's tint. */
  color: string;
  /** The logo brings its own square, so it fills the tile. */
  tile?: boolean;
}

const MONO = "#d8d8dc";

export const BRANDS: Brand[] = [
  { key: "claude", names: ["claude", "claude-code"], color: "#d97757" },
  { key: "codex", names: ["codex"], color: "#10a37f", tile: true },
  { key: "opencode", names: ["opencode", "opencode-ai"], color: "#f0883e" },
  { key: "muse", names: ["muse"], color: "#1d7cf2" },
  { key: "gemini", names: ["gemini"], color: "#4e8cf7" },
  { key: "aider", names: ["aider", "aider-chat"], color: "#0f9d58", tile: true },
  { key: "grok", names: ["grok"], color: MONO },
  { key: "cursor", names: ["cursor-agent", "cursor"], color: MONO },
  { key: "ollama", names: ["ollama"], color: MONO },
  { key: "qwen", names: ["qwen", "qwen-code"], color: "#615ced" },
  { key: "copilot", names: ["copilot"], color: MONO },
  { key: "kimi", names: ["kimi"], color: "#1e8bff" },
  { key: "deepseek", names: ["deepseek"], color: "#4d6bfe" },
  { key: "minimax", names: ["minimax"], color: "#f23f5d" },
  { key: "hermes", names: ["hermes"], color: MONO },
  { key: "antigravity", names: ["agy", "antigravity"], color: "#4285f4" },
  { key: "amp", names: ["amp"], color: MONO },
  { key: "cline", names: ["cline"], color: MONO },
  { key: "kilo", names: ["kilo", "kilocode"], color: MONO },
  { key: "goose", names: ["goose"], color: MONO },
  { key: "mistral", names: ["vibe", "mistral"], color: "#fa500f" },
  { key: "trae", names: ["trae", "trae-agent"], color: "#32f08c" },
  { key: "junie", names: ["junie"], color: "#47e06a" },
  { key: "roo", names: ["roo", "roocode"], color: MONO },
  { key: "kiro", names: ["kiro"], color: "#795af8", tile: true },
  { key: "qoder", names: ["qoder", "qodercli"], color: "#2adb5c" },
  { key: "codebuddy", names: ["codebuddy"], color: "#6c5cf5", tile: true },
];

const LOGOS = import.meta.glob<string>("./assets/clis/*.png", { eager: true, query: "?url", import: "default" });

/** Programs that only launch the CLI, and the words they take before naming it. */
const LAUNCHERS = new Set(["npx", "bunx", "pnpx", "uvx", "pipx", "pnpm", "yarn", "bun", "npm", "uv", "wsl", "cmd", "gh"]);
const LAUNCH_WORDS = new Set(["run", "dlx", "x", "exec", "tool", "/c"]);

/** `C:\x\Gemini.CMD` or `@google/gemini-cli@1` as `gemini-cli`. */
function stem(word: string): string {
  const base = (word.split(/[\\/]/).pop() ?? "").toLowerCase();
  const name = base.split("@").find(Boolean) ?? "";
  return name.replace(/\.(exe|cmd|bat|ps1)$/, "");
}

/** The word of a command that names the CLI, past anything that only launches it. */
function programWord(words: string[]): string | undefined {
  return words.find((w) => !LAUNCHERS.has(stem(w)) && !LAUNCH_WORDS.has(w.toLowerCase()) && !w.startsWith("-"));
}

/** The brand of a command, split into words. */
export function commandBrand(words: string[]): Brand | null {
  const word = programWord(words);
  if (!word) return null;
  const s = stem(word).replace(/-cli$/, "");
  return BRANDS.find((b) => b.names.includes(s)) ?? null;
}

/** What an agent's look is worked out from; old sessions may lack any of it. */
export interface BrandSource { badge?: string; program?: string; args?: string[] }

export function brandOf(a: BrandSource): Brand | null {
  return BRANDS.find((b) => b.key === a.badge) ?? commandBrand([a.program ?? "", ...(a.args ?? [])].filter(Boolean));
}

/** A CLI's logo file, for drawing it somewhere other than HTML. */
export function cliLogoUrl(a: BrandSource): string | null {
  const b = brandOf(a);
  return b ? LOGOS[`./assets/clis/${b.key}.png`] ?? null : null;
}

const svg = (inner: string) =>
  `<svg class="cli-glyph" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${inner}</svg>`;

const SHELL_GLYPHS: Record<string, string> = {
  shell: svg('<path d="M6 8l4 4-4 4M13 16h5"/>'),
  cmd: svg('<rect x="3" y="5" width="18" height="14" rx="2"/><path d="M7 10l2.5 2.5L7 15"/>'),
};

/** The first letter of the program, the only thing known about a CLI without a logo. */
function letterOf(a: BrandSource | null | undefined): string {
  const first = stem(programWord([a?.program ?? "", ...(a?.args ?? [])]) ?? "")[0] ?? "";
  return /^[\p{L}\p{N}]$/u.test(first) ? first.toUpperCase() : "?";
}

/** What goes on an agent's tile: its CLI's logo, a prompt for a plain shell,
 *  a letter for a CLI nobody has a logo for. */
export function cliMark(a: BrandSource | null | undefined): string {
  const b = a ? brandOf(a) : null;
  if (b) return `<img class="cli-logo${b.tile ? " tile" : ""}" src="${LOGOS[`./assets/clis/${b.key}.png`]}" alt="" draggable="false">`;
  return SHELL_GLYPHS[a?.badge ?? ""] ?? `<span class="cli-letter">${letterOf(a)}</span>`;
}
