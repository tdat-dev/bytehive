// Each CLI's brand mark for the pane header and inbox rows: a small inline SVG
// in front of the badge text (the text stays — a glyph alone needs a legend).
// Simplified geometric marks, not trademark artwork: a starburst for Claude,
// a six-petal blossom for Codex, a sparkle for Gemini, an arrow for Cursor,
// prompt glyphs for terminals, and a bold letter for the rest. Pure: badge in,
// markup out. The badge is only a lookup key, never interpolated, so unknown
// or hostile input falls back to the generic mark.

const open = (label: string) =>
  `<svg class="cli-mark" viewBox="0 0 16 16" aria-hidden="true"><title>${label}</title>`;

function spokes(color: string): string {
  const c = 8;
  const r = 6.5;
  const lines: string[] = [];
  for (let deg = 0; deg < 180; deg += 30) {
    const a = (deg * Math.PI) / 180;
    const dx = r * Math.sin(a);
    const dy = r * Math.cos(a);
    const f = (n: number) => n.toFixed(2);
    lines.push(`<line x1="${f(c - dx)}" y1="${f(c - dy)}" x2="${f(c + dx)}" y2="${f(c + dy)}"/>`);
  }
  return `<g stroke="${color}" stroke-width="1.6" stroke-linecap="round">${lines.join("")}</g>`;
}

function petals(color: string): string {
  const rings: string[] = [];
  for (let deg = 0; deg < 360; deg += 60) {
    rings.push(`<ellipse cx="8" cy="4.3" rx="2.1" ry="3.3" transform="rotate(${deg} 8 8)"/>`);
  }
  return `<g fill="none" stroke="${color}" stroke-width="1.4">${rings.join("")}</g>`;
}

function letter(ch: string, color: string): string {
  return `<text x="8" y="12.5" text-anchor="middle" font-size="11" font-weight="700" ` +
    `font-family="ui-monospace,SFMono-Regular,Menlo,monospace" fill="${color}">${ch}</text>`;
}

/** badge → [tooltip label, inner SVG]. Keys are CliPreset badge ids. */
const MARKS: Record<string, [string, string]> = {
  claude: ["Claude Code", spokes("#D97757")],
  codex: ["Codex", petals("#E8E8E8")],
  gemini: [
    "Gemini",
    `<path fill="#8AB4F8" d="M8 1 C8.7 5.2 9.6 6.6 15 8 C9.6 9.4 8.7 10.8 8 15 C7.3 10.8 6.4 9.4 1 8 C6.4 6.6 7.3 5.2 8 1 Z"/>` +
    `<path fill="#C58AF9" d="M12.6 10.4 C12.8 11.7 13.1 12.1 14.4 12.4 C13.1 12.7 12.8 13.1 12.6 14.4 C12.4 13.1 12.1 12.7 10.8 12.4 C12.1 12.1 12.4 11.7 12.6 10.4 Z"/>`,
  ],
  cursor: ["Cursor", `<path fill="#FFFFFF" d="M4.5 2 L12 8.2 L8.2 8.7 L9.6 13.2 L8 13.8 L6.6 9.3 L4.5 11 Z"/>`],
  opencode: ["opencode", `<text x="8" y="12" text-anchor="middle" font-size="8.5" font-weight="700" font-family="ui-monospace,Menlo,monospace" fill="#FFFFFF">&lt;/&gt;</text>`],
  copilot: ["Muse", letter("C", "#A371F7")],
  qwen: ["Qwen Code", letter("Q", "#7C7CF4")],
  aider: ["Aider", letter("A", "#4CC38A")],
  goose: ["Goose", letter("G", "#00C781")],
  shell: ["Terminal", `<g fill="none" stroke="#9AA0A6" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><polyline points="3.5,5 7,8 3.5,11"/><line x1="8.5" y1="12" x2="13" y2="12"/></g>`],
  cmd: ["Terminal", `<g fill="none" stroke="#9AA0A6" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><polyline points="3.5,5 7,8 3.5,11"/><line x1="8.5" y1="12" x2="13" y2="12"/></g>`],
  custom: ["Custom CLI", `<polyline points="6,3.5 10.5,8 6,12.5" fill="none" stroke="#9AA0A6" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>`],
};

const FALLBACK: [string, string] = ["Agent CLI", `<circle cx="8" cy="8" r="5.5" fill="none" stroke="#9AA0A6" stroke-width="1.6"/>`];

/** Inline SVG mark for a CLI badge id. Unknown or missing badges get the
 *  generic mark — panes in tests and old sessions don't always carry one. */
export function cliLogo(badge: string | null | undefined): string {
  const [label, inner] = MARKS[(badge ?? "").trim().toLowerCase()] ?? FALLBACK;
  return `${open(label)}${inner}</svg>`;
}
