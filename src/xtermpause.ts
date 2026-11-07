import type { Terminal } from "@xterm/xterm";

// xterm 6.0.0 stops drawing a terminal that is out of sight: its render service
// is "paused" (an IntersectionObserver says so) and refreshRows() only notes
// that a redraw is owed. The selection path forgets to ask. Every time output
// scrolls, SelectionService.refresh() fires on the next animation frame and
// RenderService.handleSelectionChanged() calls the DOM renderer, which rebuilds
// EVERY row of the terminal, hidden or not. With 12 shells printing and 11 of
// them off screen that is 11 full DOM rebuilds per frame, each glyph measured
// by a forced layout (a hidden element is 0 px wide, so nothing is ever cached),
// and the discarded rows pile up as garbage the renderer cannot collect while
// it is this busy: measured 1.5 GB in the renderer in 150 s (docs, G5).
//
// Two rules put it right, both limited to the selection path:
//  - nothing selected before and nothing now: there is no overlay to move, and
//    the rows themselves are redrawn by the normal paths;
//  - paused: remember the state and owe a redraw, exactly as refreshRows() does.

/** The few private fields of xterm's RenderService this relies on. */
interface RenderServiceLike {
  _isPaused: boolean;
  _needsSelectionRefresh: boolean;
  _needsFullRefresh: boolean;
  _selectionState: { start?: unknown; end?: unknown; columnSelectMode?: boolean };
  handleSelectionChanged(start: unknown, end: unknown, columnSelectMode: boolean): void;
}

function renderServiceOf(term: Terminal): RenderServiceLike | null {
  const rs = (term as unknown as { _core?: { _renderService?: Partial<RenderServiceLike> } })._core?._renderService;
  if (
    !rs ||
    typeof rs.handleSelectionChanged !== "function" ||
    typeof rs._isPaused !== "boolean" ||
    typeof rs._needsSelectionRefresh !== "boolean" ||
    typeof rs._needsFullRefresh !== "boolean" ||
    typeof rs._selectionState !== "object" ||
    rs._selectionState === null
  ) {
    return null;
  }
  return rs as RenderServiceLike;
}

/** Whether a selection range `start..end` selects anything. */
function selects(start: unknown, end: unknown): boolean {
  if (!Array.isArray(start) || !Array.isArray(end)) return false;
  return !(start[0] === end[0] && start[1] === end[1]);
}

/**
 * Make `term` skip the selection redraw while it has no selection or is out of
 * sight. Returns false, and leaves xterm alone, when its internals no longer
 * look the way this expects (a newer xterm): the pane then only costs what it
 * did before. The mount test fails in that case, so an upgrade is noticed.
 */
export function skipIdleSelectionRedraw(term: Terminal): boolean {
  const rs = renderServiceOf(term);
  if (!rs) return false;
  const original = rs.handleSelectionChanged.bind(rs);
  let drawn = false; // a selection overlay is on the rows now
  rs.handleSelectionChanged = (start, end, columnSelectMode) => {
    const has = selects(start, end);
    if (!has && !drawn) return;
    if (rs._isPaused) {
      rs._selectionState.start = start;
      rs._selectionState.end = end;
      rs._selectionState.columnSelectMode = columnSelectMode;
      rs._needsSelectionRefresh = true;
      rs._needsFullRefresh = true;
      drawn = has || drawn; // the overlay is cleared once it is drawn again
      return;
    }
    drawn = has;
    original(start, end, columnSelectMode);
  };
  return true;
}
