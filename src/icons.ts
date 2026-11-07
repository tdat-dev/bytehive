// One icon set: Lucide, 16px, stroke 1.5, from the sprite in index.html. State has
// its own 12px glyphs, because a state must never rest on colour alone.

/** An icon by its Lucide name; add the name to the sprite first. */
export const icon = (name: string, cls = "") => `<svg class="i${cls ? ` ${cls}` : ""}" aria-hidden="true"><use href="#i-${name}"/></svg>`;

/** Close / take out: a cross. */
export const ICON_CLOSE = icon("x");
/** Open full size: arrows out to the corners. */
export const ICON_EXPAND = icon("maximize");
/** The side panel of a conversation. */
export const ICON_PANEL = icon("panel-right");

/** The state glyphs, by what the list calls a state. */
export type GlyphName = "needs" | "review" | "work" | "idle" | "stopped" | "out" | "error" | "merged";

const GLYPH_OF: Record<string, GlyphName> = { needs: "needs", review: "review", working: "work", idle: "idle", stopped: "stopped", merged: "merged", error: "error", out: "out" };

/** A state's glyph. The word beside it carries the meaning for a screen reader. */
export const glyph = (state: string) => {
  const g = GLYPH_OF[state] ?? "idle";
  return `<svg class="g g-${g}" aria-hidden="true"><use href="#s-${g}"/></svg>`;
};
