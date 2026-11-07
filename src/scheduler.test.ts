import { beforeEach, describe, expect, it, vi } from "vitest";

const KEY = "maestro.schedules";
const page = `
  <div id="schedModal"><select id="schedTpl"></select><input id="schedTime"><select id="schedRepeat"></select>
  <div id="schedList"></div><div id="schedEmpty"></div></div>
  <button id="setOpenSched"></button>`;

async function open() {
  vi.resetModules();
  document.body.innerHTML = page; // the module looks its elements up as it loads
  const m = await import("./scheduler");
  m.configureScheduler({ closeSettings: () => {}, loadTemplates: () => [], launchPreset: () => {} } as never);
  m.initScheduler();
  document.getElementById("setOpenSched")!.click();
}

describe("Scheduled agents", () => {
  beforeEach(() => {
    localStorage.clear();
    localStorage.setItem(KEY, JSON.stringify([
      { id: "a", templateId: "t", time: "09:00", repeat: "daily", enabled: true },
      { id: "b", templateId: "t", time: "10:00", repeat: "daily", enabled: true },
    ]));
  });

  it("deletes the schedule whose delete button you press", async () => {
    await open();
    const rows = document.querySelectorAll("#schedList .sched-row");
    expect(rows).toHaveLength(2);
    rows[0].querySelector<HTMLElement>('[aria-label="Delete schedule"]')!.click();
    expect(JSON.parse(localStorage.getItem(KEY)!).map((s: { id: string }) => s.id)).toEqual(["b"]);
    expect(document.querySelectorAll("#schedList .sched-row")).toHaveLength(1);
  });

  it("puts the keyboard on the first field when it opens", async () => {
    await open();
    expect(document.activeElement?.id).toBe("schedTpl");
  });
});
