// Grid filters (pure): which pinned agents the Grid shows. Two independent
// filters: project (one of the pinned projects, or all) and status group.
// "Needs you" covers needs + blocked + review — all wait on the user; "Quiet"
// covers idle + stopped + merged — nothing asks for attention. inbox.ts keeps
// the chosen values and applies them to the pins before laying out.

export type GridStatus = "all" | "needs" | "working" | "quiet";

export type GridState = "needs" | "blocked" | "review" | "working" | "idle" | "stopped" | "merged";

export interface GridRow {
  id: string;
  project: string;
  state: GridState;
}

/** Task state → the status chip it lights up. */
export function statusGroup(state: GridState): Exclude<GridStatus, "all"> {
  if (state === "needs" || state === "blocked" || state === "review") return "needs";
  if (state === "working") return "working";
  return "quiet";
}

/** Pins in order, minus whatever the filters hide. */
export function filterGrid(rows: GridRow[], project: string | null, status: GridStatus): GridRow[] {
  return rows.filter(
    (r) =>
      (project === null || r.project === project) &&
      (status === "all" || statusGroup(r.state) === status),
  );
}

/** Chip counts over the (project-filtered) rows. */
export function gridCounts(rows: GridRow[]): Record<GridStatus, number> {
  const counts: Record<GridStatus, number> = { all: 0, needs: 0, working: 0, quiet: 0 };
  for (const r of rows) {
    counts.all += 1;
    counts[statusGroup(r.state)] += 1;
  }
  return counts;
}

/** Project names present, sorted, for the dropdown. */
export function gridProjects(rows: GridRow[]): string[] {
  return [...new Set(rows.map((r) => r.project))].sort((a, b) => a.localeCompare(b));
}

/** Per-project agent counts, sorted by name, for the project menu. */
export function projectCounts(rows: GridRow[]): Array<{ project: string; n: number }> {
  const byProject = new Map<string, number>();
  for (const r of rows) byProject.set(r.project, (byProject.get(r.project) ?? 0) + 1);
  return [...byProject.entries()]
    .map(([project, n]) => ({ project, n }))
    .sort((a, b) => a.project.localeCompare(b.project));
}
