// src/company/panel.ts
/* Goes in Task 9 of the phases 2 to 4 plan. */
/* The dock's company panel: the job box and the Needs you rail from the office,
 * and the list of who works where. It keeps Ctrl+Shift+O working until the office
 * replaces it. */
import "./panel.css";
import { liveSource } from "./office/live";
import type { OfficePerson } from "./office/model";
import { createJobBox, createRail } from "./office/rail";

const esc = (s: string) => s.replace(/[&<>"']/g, (c) => `&#${c.charCodeAt(0)};`);
/** A colour from a role file may be anything: only a plain hex goes in a style. */
const shirt = (c: string) => (/^#[0-9a-f]{3,8}$/i.test(c) ? c : "var(--idle)");

const personHtml = (p: OfficePerson) =>
  `<li><button type="button" class="co-person" data-person="${esc(p.id)}"><i style="background:${shirt(p.shirt)}"></i>${esc(p.name)}<span>${esc(p.title)}</span></button></li>`;

export function createCompanyPanel() {
  const source = liveSource();
  const rail = createRail(source, (level) => { if (level.kind === "desk") source.showPerson(level.personId); });
  const box = createJobBox(source);
  let people: HTMLElement | null = null;

  const drawPeople = () => {
    if (!people) return;
    const { model, note, error } = source.snapshot(Date.now());
    if (!model) {
      people.innerHTML = `<p class="co-none">${esc(note ?? (error ? "Not available." : "Starting..."))}</p>`;
      return;
    }
    const floors = model.floors.map((f) =>
      `<h4>${esc(f.name)}</h4><ul>${f.people.map(personHtml).join("") || `<li class="co-none">Nobody here yet</li>`}</ul>`).join("");
    people.innerHTML = `<h4>Top floor</h4><ul>${model.founder ? personHtml(model.founder) : `<li class="co-none">No Founder yet</li>`}</ul>${floors}`;
  };

  return {
    mount(el: HTMLElement) {
      el.innerHTML = `<div data-box></div><div data-rail></div><h3>People</h3><div class="co-org" data-org></div>`;
      box.mount(el.querySelector<HTMLElement>("[data-box]")!);
      rail.mount(el.querySelector<HTMLElement>("[data-rail]")!);
      people = el.querySelector<HTMLElement>("[data-org]");
      people?.addEventListener("click", (e) => {
        const b = (e.target as HTMLElement).closest<HTMLElement>("[data-person]");
        if (b) source.showPerson(b.dataset.person ?? "");
      });
      source.onChange(drawPeople);
      drawPeople();
    },
    show() { box.draw(); rail.draw(); drawPeople(); },
  };
}
