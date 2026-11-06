import { FOUNDER_ID } from "./company/types";
import type { AgentSpec, Workspace } from "./panetypes";

/** Where an agent's CLI starts: its worktree, else the project folder, else
 *  the home folder (a project without a folder). Never left to the PTY's own
 *  default, so ByteHive always knows it; trailing separators dropped so the
 *  path reads the way the CLI will report its own cwd. */
export function runDir(spec: { worktree?: string; cwd: string | null }, home: string): string {
  const dir = spec.worktree || spec.cwd || home;
  return dir.replace(/(?<=[^:\\/])[\\/]+$/, "");
}

/** Whether this agent is the company's Founder. There is one, with this id, and
 *  it runs in a project without being a worker there. */
export const isFounder = (spec: Pick<AgentSpec, "personId">) => spec.personId === FOUNDER_ID;

/** Whether an agent gets a new worktree when it boots. A member of the company
 *  always does on a git repo, two people must not share a checkout; everyone
 *  else follows the project's setting. The Founder never does: it writes nothing. */
export function needsWorktree(ws: Pick<Workspace, "isolated" | "repoRoot">, spec: Pick<AgentSpec, "worktree" | "personId">): boolean {
  return !isFounder(spec) && !!ws.repoRoot && !spec.worktree && (ws.isolated || !!spec.personId);
}

/** Whether ByteHive says yes to Claude's "trust this folder?" for this agent. The
 *  owner approved a member's hire, and nobody stands by its pane to answer. */
export function trustsFolder(spec: Pick<AgentSpec, "personId" | "badge">): boolean {
  return !!spec.personId && spec.badge === "claude";
}

/** Whether two folder paths name the same folder (case and trailing separator aside). */
export function sameFolder(a: string | null | undefined, b: string | null | undefined): boolean {
  const norm = (p: string) => p.replace(/[\\/]+$/, "").replace(/\//g, "\\").toLowerCase();
  return !!a && !!b && norm(a) === norm(b);
}

/** Last path segment, tolerant of trailing and mixed slashes. */
export function basename(p: string): string {
  const parts = p.replace(/[/\\]+$/, "").split(/[/\\]/);
  return parts[parts.length - 1] || p;
}

/** Workspace label: the dir basename, else the first free "Project N". */
export function nextWorkspaceName(dir: string | null, taken: string[]): string {
  if (dir) return basename(dir);
  let n = 1;
  while (taken.includes(`Project ${n}`)) n++;
  return `Project ${n}`;
}

/** Which workspace id to activate after `closingId` is removed (neighbour to
 *  the right, else the last; null if it was the only one). */
export function pickNextActive(ids: string[], closingId: string): string | null {
  const rest = ids.filter((x) => x !== closingId);
  if (rest.length === 0) return null;
  const i = ids.indexOf(closingId);
  return rest[Math.min(i, rest.length - 1)];
}

/** Confirm before quitting only when at least one terminal is live. */
export function needsCloseConfirm(total: number): boolean {
  return total > 0;
}

/** The variables every agent process gets. BYTEHIVE_WORKSPACE is the project,
 *  never the worktree: the app reads the board and the outbox in the project.
 *  BYTEHIVE_RUN_DIR is where the agent really works (its worktree), which is
 *  where card_done looks for what it changed. */
export function agentEnv(spec: Pick<AgentSpec, "name" | "cwd" | "env">, ranIn: string | null): Array<[string, string]> {
  return [
    ["BYTEHIVE_AGENT", spec.name],
    ["BYTEHIVE_WORKSPACE", spec.cwd ?? ranIn ?? ""],
    ["BYTEHIVE_RUN_DIR", ranIn ?? spec.cwd ?? ""],
    ...(spec.env ?? []),
  ];
}
