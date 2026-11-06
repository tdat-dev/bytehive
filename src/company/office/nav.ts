/* Where the owner is in the office, as data. One axis: Building, Floor, Desk. */
import type { NeedRow } from "../needs";
import { FOUNDER_ID } from "../types";
import type { OfficeModel, OfficePerson } from "./model";

export type DeskTab = "terminal" | "changes" | "report";
export type Level =
  | { kind: "building" }
  | { kind: "floor"; floorId: string }
  | { kind: "desk"; personId: string; floorId: string | null; tab: DeskTab };

export interface Crumb { label: string; level: Level; current: boolean }

export const BUILDING: Level = { kind: "building" };
/** 30 frames a second, rounded up so a slow timer never makes it 31. */
export const MOVE_MS = 34;
/** Working people move at 8 frames a second: toy-like, and cheap enough to leave on. */
export const AMBIENT_MS = 125;

const everyone = (m: OfficeModel): OfficePerson[] => [...(m.founder ? [m.founder] : []), ...m.floors.flatMap((f) => f.people)];

export function navUp(l: Level): Level {
  if (l.kind === "desk" && l.floorId) return { kind: "floor", floorId: l.floorId };
  return BUILDING;
}

export function crumbs(l: Level, m: OfficeModel): Crumb[] {
  const path: Array<{ label: string; level: Level }> = [{ label: "ByteHive", level: BUILDING }];
  const floorId = l.kind === "building" ? null : l.floorId;
  if (floorId) path.push({ label: m.floors.find((f) => f.id === floorId)?.name ?? floorId, level: { kind: "floor", floorId } });
  if (l.kind === "desk") path.push({ label: everyone(m).find((p) => p.id === l.personId)?.name ?? l.personId, level: l });
  return path.map((c, i) => ({ ...c, current: i === path.length - 1 }));
}

/** Esc is the terminal's while it has focus, and a dialog's, the dock's or a text field's while they are up. */
export function escapeGoesUp(o: { level: Level; terminal: boolean; dialog: boolean; dock: boolean; typing: boolean }): boolean {
  return o.level.kind !== "building" && !o.terminal && !o.dialog && !o.dock && !o.typing;
}

export function rowTarget(row: NeedRow, m: OfficeModel): Level {
  const p = row.personId ? everyone(m).find((x) => x.id === row.personId) : undefined;
  if (!p) return BUILDING;
  return { kind: "desk", personId: p.id, floorId: p.id === FOUNDER_ID ? null : p.floorId, tab: row.kind === "merge" ? "changes" : "terminal" };
}

/** People leave and projects close while the owner looks at them. */
export function clampLevel(l: Level, m: OfficeModel): Level {
  if (l.kind === "desk" && everyone(m).some((p) => p.id === l.personId)) return l;
  const floorId = l.kind === "building" ? null : l.floorId;
  if (floorId && m.floors.some((f) => f.id === floorId)) return l.kind === "floor" ? l : { kind: "floor", floorId };
  return BUILDING;
}

export function homeView(o: { used: boolean; webgl: boolean; elsewhere: boolean }): "office" | "inbox" {
  return o.used && o.webgl && !o.elsewhere ? "office" : "inbox";
}

/** How long until the next frame, or null to stop drawing. */
export function frameDelay(a: { visible: boolean; moving: boolean; ambient: boolean; reduced: boolean }): number | null {
  if (!a.visible) return null;
  if (a.moving) return MOVE_MS;
  return a.ambient && !a.reduced ? AMBIENT_MS : null;
}
