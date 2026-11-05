// src/company/types.ts
export const FOUNDER_ID = "founder";

export type Rank = "founder" | "lead" | "staff";

export interface Look { shirt: string; hair: string; prop: string }

export interface Floor { id: string; dir: string; name: string }

export interface Person {
  id: string;
  name: string;
  role: string;
  rank: Rank;
  floor: string | null; // null for the Founder
  boss: string | null; // Person id, null for the Founder
  cli: string;
  look: Look;
  session?: string; // what their pane runs on, so a closed pane comes back on it
  worktree?: string;
  branch?: string;
}

export interface Limits {
  peoplePerJob: number;
  peoplePerLead: number;
  peopleTotal: number;
  tokensPerJob: number;
  stallMinutes: number;
  maxReturns: number;
}

export const DEFAULT_LIMITS: Limits = {
  peoplePerJob: 5,
  peoplePerLead: 5,
  peopleTotal: 20,
  tokensPerJob: 5_000_000,
  stallMinutes: 10,
  maxReturns: 3,
};

export interface Company {
  version: 1;
  floors: Floor[];
  people: Person[];
  limits: Limits;
  nextJob: number;
}

export interface RoleDef {
  id: string;
  title: string;
  department: string;
  rank: Rank;
  cli: string;
  denyTools: string[];
  look: Look;
  prompt: string;
  skills: string[]; // folder names under the role's skills/
}

export interface Hire { role: string; floor: string }

export interface PlanCard {
  floor: string;
  title: string;
  desc: string;
  checklist: string[];
  assignee?: string; // Person name, an existing person or left for the Lead
}

/** A role the Founder drafts in a plan. The rest of a RoleDef is the app's to set. */
export interface NewRole { id: string; title: string; department: string; prompt: string }

export interface JobPlan {
  summary: string;
  cards: PlanCard[];
  hires: Hire[];
  tokenCap?: number;
  newRoles?: NewRole[]; // saved only when the owner approves the plan
}

export type JobStatus = "planning" | "awaiting-plan" | "running" | "awaiting-merge" | "paused" | "stopped" | "done" | "rejected";

export interface Job {
  id: string;
  title: string;
  ask: string;
  status: JobStatus;
  plan: JobPlan | null;
  tokenCap: number;
  tokensUsed: number;
  tokenBase: Record<string, number>; // Person id to the fresh-token figure last seen for them, to count what they add
  people: string[]; // Person ids working this Job
  summary?: string;
  createdAt: number;
}

export type CompanyRequest =
  | { type: "message"; ts: number; from: string; to: string | null; message: string }
  | { type: "plan"; ts: number; from: string; job: string; plan: JobPlan }
  | { type: "job-done"; ts: number; from: string; job: string; summary: string };
