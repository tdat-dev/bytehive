// Early warnings while agents still work: two agents changing the same
// files, or two agents on the same task in one project. Pure: feeds on the
// file lists tasks.ts already polls, no extra git calls. A race (several
// agents given the same job on purpose) is not a dupe.

export type ClashKind = "file" | "task";

export interface ClashNote {
  kind: ClashKind;
  /** Stable identity for change detection. */
  key: string;
  /** Names of the other agents involved, sorted. */
  with: string[];
  /** Shared paths in the owner's own spelling (file clashes). */
  files: string[];
}

export interface ClashInput {
  paneId: string;
  name: string;
  wsId: string;
  state: string;
  running: boolean;
  title: string | null;
  /** Race id when it runs a job shared on purpose, else null. */
  race: string | null;
  /** Changed paths, relative to the repo. */
  paths: string[];
}

const normPath = (p: string) => p.replace(/\\/g, "/").toLowerCase();
const normTitle = (t: string) => t.trim().toLowerCase().replace(/\s+/g, " ");

/** Agents that can still surprise each other: stopped ones are frozen and
 *  merged work is already in, so only running, unmerged agents count. */
function live(inputs: ClashInput[]): ClashInput[] {
  return inputs.filter((i) => i.running && i.state !== "merged");
}

function fileNotes(inputs: ClashInput[]): Map<string, ClashNote> {
  const out = new Map<string, ClashNote>();
  const byWs = new Map<string, ClashInput[]>();
  for (const i of live(inputs)) {
    const list = byWs.get(i.wsId) ?? [];
    list.push(i);
    byWs.set(i.wsId, list);
  }
  for (const group of byWs.values()) {
    // normalized path -> panes touching it
    const touches = new Map<string, ClashInput[]>();
    for (const i of group)
      for (const p of new Set(i.paths.map(normPath))) {
        const list = touches.get(p) ?? [];
        list.push(i);
        touches.set(p, list);
      }
    const shared = new Map<string, ClashInput[]>();
    for (const [p, who] of touches) if (who.length > 1) shared.set(p, who);
    if (!shared.size) continue;
    for (const i of group) {
      const mine = new Set(i.paths.map(normPath));
      const hit = [...shared.keys()].filter((p) => mine.has(p));
      if (!hit.length) continue;
      const others = [...new Set([...hit.flatMap((p) => shared.get(p)!)])].filter((o) => o.paneId !== i.paneId);
      const withNames = [...new Set(others.map((o) => o.name))].sort();
      const own = new Map(i.paths.map((p) => [normPath(p), p]));
      const files = hit.map((p) => own.get(p)!).slice(0, 20);
      const ids = [i.paneId, ...others.map((o) => o.paneId)].sort();
      out.set(i.paneId, { kind: "file", key: `file:${ids.join("+")}:${[...hit].sort().join("+")}`, with: withNames, files });
    }
  }
  return out;
}

function taskNotes(inputs: ClashInput[]): Map<string, ClashNote> {
  const out = new Map<string, ClashNote>();
  const byWsTitle = new Map<string, ClashInput[]>();
  for (const i of live(inputs)) {
    const t = (i.title ?? "").trim();
    if (!t) continue;
    const k = `${i.wsId}‖${normTitle(t)}`;
    const list = byWsTitle.get(k) ?? [];
    list.push(i);
    byWsTitle.set(k, list);
  }
  for (const group of byWsTitle.values()) {
    if (group.length < 2) continue;
    const title = normTitle(group[0].title ?? "");
    for (const i of group) {
      // same race, same job on purpose
      const others = group.filter((o) => o.paneId !== i.paneId && !(i.race !== null && o.race === i.race));
      if (!others.length) continue;
      const withNames = [...new Set(others.map((o) => o.name))].sort();
      const ids = [i.paneId, ...others.map((o) => o.paneId)].sort();
      out.set(i.paneId, { kind: "task", key: `task:${ids.join("+")}:${title}`, with: withNames, files: [] });
    }
  }
  return out;
}

/** One early-warning note per agent: a file clash wins over a task dupe,
 *  concrete files beat a bare same title. */
export function clashNotes(inputs: ClashInput[]): Map<string, ClashNote> {
  const out = taskNotes(inputs);
  for (const [id, note] of fileNotes(inputs)) out.set(id, note);
  return out;
}

function capped(items: string[]): string {
  return items.slice(0, 2).join(", ") + (items.length > 2 ? ` +${items.length - 2} more` : "");
}

/** Row suffix ("overlaps Bo in src/app.ts", "same task as Bo"). */
export function clashShort(note: ClashNote): string {
  return note.kind === "file" ? `overlaps ${capped(note.with)} in ${capped(note.files)}` : `same task as ${capped(note.with)}`;
}

/** History line: the same, capitalized. */
export function clashHistory(note: ClashNote): string {
  const s = clashShort(note);
  return s.charAt(0).toUpperCase() + s.slice(1);
}
