import { beforeEach, describe, expect, it, vi } from "vitest";

const ipc = vi.hoisted(() => ({ fsReadDir: vi.fn(), fsDelete: vi.fn(), recordRead: vi.fn() }));
vi.mock("./ipc", () => ipc);
vi.mock("./terminal", () => ({ mountTerminal: () => ({ dispose() {}, write() {}, fit() {}, clear() {} }) }));
vi.mock("./appstate", () => ({ activeWs: () => null }));

const tick = () => new Promise((r) => setTimeout(r, 0));

async function open() {
  vi.resetModules();
  document.body.innerHTML = `<div id="replayModal"><div id="replayList"></div><p id="replayEmpty"></p><div id="replayPlayer"><div id="replayTerm"></div></div><button id="replayBack"></button><span id="replayCrumb"></span></div>`;
  const m = await import("./replay");
  m.openReplays({ dir: "D:/p", panes: new Map() } as never);
  await tick();
}

describe("Replays", () => {
  beforeEach(() => {
    ipc.fsDelete.mockReset().mockResolvedValue(undefined);
    ipc.recordRead.mockReset().mockResolvedValue("");
    ipc.fsReadDir.mockReset().mockResolvedValue([{ name: "Ana-1760000000000.jsonl", is_dir: false, size: 2048 }]);
  });

  it("deletes a recording from its delete button, and does not open it", async () => {
    await open();
    document.querySelector<HTMLElement>('[aria-label="Delete recording"]')!.click();
    await tick();
    expect(ipc.fsDelete).toHaveBeenCalledWith("D:/p", ".bytehive/recordings/Ana-1760000000000.jsonl");
    expect(ipc.recordRead).not.toHaveBeenCalled();
  });

  it("opens the recording when the row is pressed", async () => {
    await open();
    document.querySelector<HTMLElement>(".replay-row")!.click();
    await tick();
    expect(ipc.recordRead).toHaveBeenCalled();
    expect(ipc.fsDelete).not.toHaveBeenCalled();
  });
});
