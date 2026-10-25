// How the phone dashboard lists the fleet: grouped by project, the projects
// waiting on you first, and a stable order inside each so rows don't jump
// every second as agents flip between active and idle.

import { type FleetStatus } from "./fleet";
import { basename, sameFolder } from "./workspaces";

export interface DashProject {
  key: string; // groups agents of the same folder, whatever workspace they sit in
  name: string;
  dir: string | null;
}

/** The project an agent works on: the folder a resumed conversation carries
 *  on in, else its workspace's folder, else just the workspace. */
export function projectOf(
  spec: { resumeIn?: string },
  ws: { id: string; name: string; dir: string | null },
): DashProject {
  const dir = spec.resumeIn && !sameFolder(spec.resumeIn, ws.dir) ? spec.resumeIn : ws.dir;
  if (!dir) return { key: `ws:${ws.id}`, name: ws.name, dir: null };
  const key = dir.replace(/[\\/]+$/, "").replace(/\//g, "\\").toLowerCase();
  return { key, name: dir === ws.dir ? ws.name : basename(dir), dir };
}

interface Orderable {
  name: string;
  status: FleetStatus;
  projectKey: string;
  project: string;
}

// Inside a project: needs you, then the rest still running, then stopped.
// Active and idle share a rank on purpose (they swap all the time).
const rank = (s: FleetStatus) => (s === "needs" ? 0 : s === "stopped" ? 2 : 1);

/** Agents grouped by project (consecutive), projects that need you first,
 *  then ones with something running, then by name. */
export function orderForDash<T extends Orderable>(agents: T[]): T[] {
  const best = new Map<string, number>();
  for (const a of agents) best.set(a.projectKey, Math.min(best.get(a.projectKey) ?? 3, rank(a.status)));
  return [...agents].sort(
    (a, b) =>
      (best.get(a.projectKey) ?? 3) - (best.get(b.projectKey) ?? 3) ||
      a.project.localeCompare(b.project) ||
      a.projectKey.localeCompare(b.projectKey) ||
      rank(a.status) - rank(b.status) ||
      a.name.localeCompare(b.name),
  );
}
