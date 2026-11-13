// This page holds keys. GitHub Pages can't send frame-ancestors and a meta CSP
// can't either, so the page refuses to run inside another one.

export function isFramed(w: { top: unknown; self: unknown }): boolean {
  try {
    return w.top !== w.self;
  } catch {
    return true;
  }
}
