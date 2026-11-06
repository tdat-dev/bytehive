/* The Report tab: what a person did, from the board and the Jobs. */
import type { Board } from "../../board";
import type { Job } from "../types";

export interface ReportCard {
  title: string;
  list: string;
  job: string | null;
  jobTitle: string | null;
  summary: string | null;
  files: string[];
  checklist: Array<{ text: string; done: boolean }>;
  stopped: boolean;
}
export interface DeskReport { name: string; branch: string | null; cards: ReportCard[] }

export function deskReport(p: { id: string; name: string; branch: string | null }, board: Board | null, jobs: Job[]): DeskReport {
  const cards = (board?.lists ?? []).flatMap((l) => l.cards
    .filter((c) => c.assigneeId === p.id || c.done?.by === p.name)
    .map((c): ReportCard => ({
      title: c.title, list: l.title, job: c.job ?? null, jobTitle: jobs.find((j) => j.id === c.job)?.title ?? null,
      summary: c.done?.summary ?? null, files: c.done?.files ?? [],
      checklist: c.checklist.map(({ text, done }) => ({ text, done })), stopped: !!c.stopped,
    })));
  return { name: p.name, branch: p.branch, cards };
}
