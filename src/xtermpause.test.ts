// @vitest-environment happy-dom
import { describe, it, expect, vi } from "vitest";
import { Terminal } from "@xterm/xterm";
import { skipIdleSelectionRedraw } from "./xtermpause";

/** A stand-in with the same private shape as xterm's RenderService. */
function fakeTerm(paused = false) {
  const draw = vi.fn();
  const rs = {
    _isPaused: paused,
    _needsSelectionRefresh: false,
    _needsFullRefresh: false,
    _selectionState: {} as { start?: unknown; end?: unknown; columnSelectMode?: boolean },
    handleSelectionChanged: draw,
  };
  const term = { _core: { _renderService: rs } } as unknown as Terminal;
  return { term, rs, draw };
}

describe("skipIdleSelectionRedraw", () => {
  it("does not redraw when nothing is selected and nothing was", () => {
    const { term, rs, draw } = fakeTerm();
    expect(skipIdleSelectionRedraw(term)).toBe(true);
    rs.handleSelectionChanged(undefined, undefined, false);
    rs.handleSelectionChanged([3, 4], [3, 4], false); // an empty range
    expect(draw).not.toHaveBeenCalled();
  });

  it("redraws a real selection, and the clearing of it, then goes quiet again", () => {
    const { term, rs, draw } = fakeTerm();
    skipIdleSelectionRedraw(term);
    rs.handleSelectionChanged([0, 1], [5, 1], false);
    rs.handleSelectionChanged(undefined, undefined, false); // the overlay has to go
    rs.handleSelectionChanged(undefined, undefined, false);
    expect(draw).toHaveBeenCalledTimes(2);
    expect(draw).toHaveBeenNthCalledWith(1, [0, 1], [5, 1], false);
  });

  it("while out of sight owes the redraw instead of doing it, as refreshRows does", () => {
    const { term, rs, draw } = fakeTerm(true);
    skipIdleSelectionRedraw(term);
    rs.handleSelectionChanged([0, 1], [5, 1], true);
    expect(draw).not.toHaveBeenCalled();
    expect(rs._needsSelectionRefresh).toBe(true);
    expect(rs._needsFullRefresh).toBe(true);
    expect(rs._selectionState).toEqual({ start: [0, 1], end: [5, 1], columnSelectMode: true });
    rs._isPaused = false; // back on screen: the next change goes through
    rs.handleSelectionChanged([0, 1], [6, 1], true);
    expect(draw).toHaveBeenCalledTimes(1);
  });

  it("a hidden terminal with no selection owes nothing", () => {
    const { term, rs, draw } = fakeTerm(true);
    skipIdleSelectionRedraw(term);
    for (let i = 0; i < 100; i++) rs.handleSelectionChanged(undefined, undefined, false);
    expect(draw).not.toHaveBeenCalled();
    expect(rs._needsFullRefresh).toBe(false);
  });

  it("leaves xterm alone when its internals are not what it expects", () => {
    expect(skipIdleSelectionRedraw({} as unknown as Terminal)).toBe(false);
    expect(skipIdleSelectionRedraw({ _core: { _renderService: { handleSelectionChanged() {} } } } as unknown as Terminal)).toBe(false);
  });
});

// The private fields above belong to xterm. If an upgrade renames them, the
// guard quietly stops working and a streaming fleet is back to 1.5 GB in the
// renderer; this is the test that notices.
describe("the real xterm", () => {
  function open() {
    const div = document.createElement("div");
    document.body.appendChild(div);
    const term = new Terminal({ allowProposedApi: true });
    term.open(div);
    return term;
  }

  it("still has the render service shape the guard hooks into", () => {
    expect(skipIdleSelectionRedraw(open())).toBe(true);
  });

  it("keeps a paused terminal from rebuilding its rows, and lets a shown one", () => {
    const term = open();
    skipIdleSelectionRedraw(term);
    const rs = (term as unknown as { _core: { _renderService: Record<string, any> } })._core._renderService;
    const renderer = vi.spyOn(rs._renderer.value, "handleSelectionChanged");
    rs._isPaused = true;
    rs.handleSelectionChanged([0, 0], [4, 0], false);
    expect(renderer).not.toHaveBeenCalled();
    rs._isPaused = false;
    rs.handleSelectionChanged([0, 0], [4, 0], false);
    expect(renderer).toHaveBeenCalledTimes(1);
  });
});
