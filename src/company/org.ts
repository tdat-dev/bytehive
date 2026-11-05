/* The company's rules, with no DOM and no IPC: the app and the MCP bundle
 * both import this, so what an agent is told and what the app enforces
 * cannot drift apart. */
import { DEFAULT_LIMITS, FOUNDER_ID, type Company, type JobPlan, type Person, type RoleDef } from "./types";

const same = (a: string, b: string) => a.trim().toLowerCase() === b.trim().toLowerCase();

/** JSON from a file the owner may have edited: Windows editors can start it with a byte order mark. */
export const parseOwned = <T>(text: string): T => JSON.parse(text.replace(/^﻿/, "")) as T;

export function emptyCompany(): Company {
  const founder: Person = {
    id: FOUNDER_ID, name: "Founder", role: "founder", rank: "founder", floor: null, boss: null,
    cli: "claude", look: { shirt: "#f5b83d", hair: "#2a2e3a", prop: "none" },
  };
  return { version: 1, floors: [], people: [founder], limits: { ...DEFAULT_LIMITS }, nextJob: 1 };
}

/** A role id becomes a folder name, so the app and the MCP tool hold it to this.
 *  Windows keeps some names for devices, and a folder can't have them. */
export const validRoleId = (id: unknown): id is string =>
  typeof id === "string" && /^[a-z][a-z0-9-]{1,30}$/.test(id) && !/^(con|prn|aux|nul|com[1-9]|lpt[1-9])$/.test(id);

/** A floor's id, from its project's name: two projects can share a name. */
export function floorId(name: string, taken: string[]): string {
  const slug = name.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "") || "floor";
  let id = slug;
  for (let n = 2; taken.includes(id); n += 1) id = `${slug}-${n}`;
  return id;
}

export const personById = (c: Company, id: string) => c.people.find((p) => p.id === id);
export const personByName = (c: Company, name: string) => c.people.find((p) => same(p.name, name));
export const reportsOf = (c: Company, id: string) => c.people.filter((p) => p.boss === id);

/** A person talks to their boss and to their own reports, nobody else. */
export function canMessage(c: Company, fromId: string, toId: string): boolean {
  const from = personById(c, fromId);
  const to = personById(c, toId);
  if (!from || !to || from.id === to.id) return false;
  return from.boss === to.id || to.boss === from.id;
}

export function resolveTargets(c: Company, fromId: string, to: string | null): { people: Person[] } | { error: string } {
  const from = personById(c, fromId);
  if (!from) return { error: `You are not on the company roster (${fromId}).` };
  if (to === null) {
    const boss = from.boss ? personById(c, from.boss) : undefined;
    const people = boss ? [boss] : reportsOf(c, from.id);
    return people.length ? { people } : { error: "You have nobody to message yet: no boss and no reports." };
  }
  const target = personByName(c, to);
  if (!target) return { error: `Nobody named ${to} works here. See company_status.` };
  if (canMessage(c, from.id, target.id)) return { people: [target] };
  const boss = from.boss ? personById(c, from.boss) : undefined;
  return { error: `You cannot message ${target.name} directly. Send it to ${boss?.name ?? "your reports"} and they will pass it on.` };
}

const floorPeople = (c: Company, floor: string) => c.people.filter((p) => p.floor === floor);

export function validatePlan(c: Company, plan: JobPlan, roles: RoleDef[]): string[] {
  const errors: string[] = [];
  const known = new Set(c.floors.map((f) => f.id));
  const roleOf = (id: string) => roles.find((r) => r.id === id);
  if (plan.hires.length > c.limits.peoplePerJob) errors.push(`A plan may hire at most ${c.limits.peoplePerJob} people.`);
  if (c.people.length + plan.hires.length > c.limits.peopleTotal) errors.push(`The company is limited to ${c.limits.peopleTotal} people.`);
  for (const h of plan.hires) {
    if (!known.has(h.floor)) errors.push(`Unknown floor ${h.floor}.`);
    if (h.role === FOUNDER_ID || roleOf(h.role)?.rank === "founder") errors.push("A plan cannot hire a Founder: the company has one.");
    else if (!roleOf(h.role)) errors.push(`Unknown role ${h.role}. See role_list or propose one with role_propose.`);
  }
  for (const card of plan.cards) {
    if (!known.has(card.floor)) errors.push(`Card "${card.title}" is on unknown floor ${card.floor}.`);
    else if (!floorPeople(c, card.floor).length && !plan.hires.some((h) => h.floor === card.floor)) {
      errors.push(`Card "${card.title}" is on floor ${card.floor}, where nobody works: hire someone there.`);
    }
  }
  for (const floor of known) {
    const hires = plan.hires.filter((h) => h.floor === floor);
    if (!hires.length) continue;
    const now = floorPeople(c, floor);
    const leads = now.filter((p) => p.rank === "lead").length + hires.filter((h) => roleOf(h.role)?.rank === "lead").length;
    const total = now.length + hires.length;
    if (total >= 2 && leads === 0) errors.push(`Floor ${floor} will have ${total} people and needs a Lead: add a hire with role lead.`);
    if (leads > 1) errors.push(`Floor ${floor} can only have one Lead.`);
    if (total - leads > c.limits.peoplePerLead) errors.push(`A Lead can have at most ${c.limits.peoplePerLead} people.`);
  }
  return errors;
}

export function applyHires(
  c: Company,
  plan: JobPlan,
  roles: RoleDef[],
  pick: { name: (taken: string[]) => string; id: () => string },
): { company: Company; hired: Person[] } {
  const people = c.people.map((p) => ({ ...p }));
  const hired: Person[] = [];
  const add = (roleId: string, floor: string, boss: string) => {
    const role = roles.find((r) => r.id === roleId)!;
    const person: Person = {
      id: pick.id(), name: pick.name(people.map((p) => p.name)), role: role.id,
      rank: role.rank === "lead" ? "lead" : "staff", floor, boss, cli: role.cli, look: role.look,
    };
    people.push(person);
    hired.push(person);
    return person;
  };
  const isLead = (roleId: string) => roles.find((r) => r.id === roleId)?.rank === "lead";
  for (const h of plan.hires.filter((x) => isLead(x.role))) {
    const lead = add(h.role, h.floor, FOUNDER_ID);
    for (const p of people) if (p.floor === h.floor && p.id !== lead.id) p.boss = lead.id;
  }
  for (const h of plan.hires.filter((x) => !isLead(x.role))) {
    const lead = people.find((p) => p.floor === h.floor && p.rank === "lead");
    add(h.role, h.floor, lead?.id ?? FOUNDER_ID);
  }
  return { company: { ...c, people }, hired };
}
