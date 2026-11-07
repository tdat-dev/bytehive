import { beforeEach, describe, expect, it, vi } from "vitest";
import { emptyCompany } from "./org";
import { officeModel, type OfficeModel, type OfficePerson } from "./office/model";
import type { Snapshot } from "./office/source";
import type { Company, Person } from "./types";

const live = vi.hoisted(() => ({ snap: null as unknown, showPerson: vi.fn(() => true) }));
vi.mock("./office/live", () => ({
  liveSource: () => ({ snapshot: () => live.snap, onChange: () => () => {}, showPerson: live.showPerson }),
}));

import { createCompanyPanel } from "./panel";

const evil = `<img src=x onerror=alert(1)>"'`;
const someone = (over: Partial<OfficePerson>): OfficePerson => ({
  id: "p1", name: "Ana", rank: "staff", title: "Developer", department: "Engineering", cli: "claude", shirt: "#5fd08a", hair: "#2a2e3a",
  prop: "laptop", state: "working", doing: null, floorId: "a", seat: 0, paneId: null, wsId: null, ...over,
});
const withPerson = (p: OfficePerson): Snapshot => ({
  model: { founder: null, floors: [{ id: "a", name: "alpha", people: [p] }], needs: [], jobs: [], flights: [], counts: { people: 1, working: 0, needs: 0 } } as unknown as OfficeModel,
  note: null, error: null,
});
const mount = () => {
  const el = document.createElement("div");
  document.body.appendChild(el);
  createCompanyPanel().mount(el);
  return el;
};

describe("the people list", () => {
  beforeEach(() => { document.body.innerHTML = ""; live.showPerson.mockClear(); });

  it("escapes names and roles", () => {
    live.snap = withPerson(someone({ id: evil, name: evil, title: evil }));
    const el = mount();
    expect(el.querySelector("img")).toBeNull();
    expect(el.querySelector("[onerror]")).toBeNull();
    expect(el.querySelector(".co-person")?.textContent).toBe(`${evil}${evil}`);
    expect(el.querySelector(".co-person span")?.textContent).toBe(evil);
    expect(el.querySelector<HTMLElement>(".co-person")?.dataset.person).toBe(evil);
  });
  it("escapes a floor's name", () => {
    const snap = withPerson(someone({}));
    snap.model!.floors[0].name = evil;
    live.snap = snap;
    const el = mount();
    expect(el.querySelector("img")).toBeNull();
    expect([...el.querySelectorAll("h4")].map((h) => h.textContent)).toContain(evil);
  });
  it.each(["red;position:fixed", "url(x)", "#12", "#zzzzzz", ""])("puts no shirt colour in a style unless it is a plain hex: %s", (shirt) => {
    live.snap = withPerson(someone({ shirt }));
    expect(mount().querySelector<HTMLElement>(".co-person i")?.getAttribute("style")).toBe("background:var(--idle)");
  });
  it("keeps a real shirt colour", () => {
    live.snap = withPerson(someone({ shirt: "#5fd08a" }));
    expect(mount().querySelector<HTMLElement>(".co-person i")?.getAttribute("style")).toBe("background:#5fd08a");
  });
  it("stays safe from what a person's file says, through the real model", () => {
    const hired: Person = { id: "p1", name: evil, role: evil, rank: "staff", floor: "a", boss: "founder", cli: "claude", look: { shirt: "red;position:fixed", hair: "x", prop: "none", edited: true } };
    const base = emptyCompany();
    const company: Company = { ...base, floors: [{ id: "a", dir: "D:/a", name: "alpha" }], people: [...base.people, hired] };
    live.snap = { model: officeModel(company, [], [], [], { openDirs: ["D:/a"] }), note: null, error: null };
    const el = mount();
    const row = [...el.querySelectorAll<HTMLElement>(".co-person")].find((b) => b.dataset.person === "p1")!;
    expect(row.textContent).toContain(evil);
    expect(row.querySelector("i")?.getAttribute("style")).toMatch(/^background:#[0-9a-f]{3,8}$/i);
    expect(el.querySelector("img")).toBeNull();
  });
  it("shows the person when their row is clicked", () => {
    live.snap = withPerson(someone({}));
    mount().querySelector<HTMLElement>(".co-person")!.click();
    expect(live.showPerson).toHaveBeenCalledWith("p1");
  });
  it("says why there is no list", () => {
    live.snap = { model: null, note: "The Company is running in another ByteHive window.", error: null };
    expect(mount().querySelector("[data-org]")?.textContent).toContain("another ByteHive window");
  });
});
