// ByteHive's own settings: how agents start, how the inbox gets your attention,
// what Split and Review do, and what happens at startup. One typed store in
// localStorage; every value has a default so a fresh install behaves well.
// Settings → Agents / Inbox / Review render from PREF_META, and each feature
// reads its value through getPref at the moment it acts.

export interface Prefs {
  /** CLI a New agent starts with: a preset id, or "last" for the last one used. */
  defaultCli: string;
  /** How many agents New agent offers to start on one job. */
  defaultCount: 1 | 2 | 3;
  /** Seconds to wait for a new agent's CLI to reach its prompt before typing its job. */
  jobDelay: number;
  /** Give every agent its own git worktree and branch (projects that are git repos). */
  worktree: boolean;
  /** Put the Director's rules into new projects' first agent. */
  directorFirst: boolean;
  /** Windows notification when an agent starts waiting on you. */
  notifyNeeds: boolean;
  /** Windows notification when an agent finishes (done, ready to review, or crashed). */
  notifyDone: boolean;
  /** When an agent needs you and the one on screen is not busy, show it. */
  jumpToNeeds: boolean;
  /** Most agents in the Grid: one terminal, the rest as cards. */
  splitMax: SplitMax;
  /** Open Changes on its own when the agent on screen finishes with changes. */
  reviewOnDone: boolean;
  /** Reopen last session's projects and agents (stopped) when ByteHive starts. */
  restore: boolean;
  /** One-time tips when a feature first becomes useful. */
  tips: boolean;
  /** Tell an agent that stopped at its plan's usage limit to carry on once the limit resets. */
  autoResume: boolean;
  /** An agent that runs out of usage hands its task to another agent whose plan has room, at once. */
  autoHandover: boolean;
  /** Windows notification when a plan passes 90% of a limit, and when an agent runs out. */
  limitNotify: boolean;
}

/** How many agents the Grid may hold. */
export const SPLIT_SIZES = [2, 3, 4, 6, 9] as const;
export type SplitMax = (typeof SPLIT_SIZES)[number];

export const DEFAULTS: Prefs = {
  defaultCli: "last",
  defaultCount: 1,
  jobDelay: 4,
  // Agents work in the project folder; a worktree each is opt-in.
  worktree: false,
  directorFirst: false,
  notifyNeeds: true,
  notifyDone: true,
  jumpToNeeds: false,
  splitMax: 9,
  reviewOnDone: false,
  restore: true,
  tips: true,
  autoResume: true,
  autoHandover: true,
  limitNotify: true,
};

const KEY = "maestro.prefs";
const listeners = new Set<(p: Prefs) => void>();

function clamp(p: Partial<Prefs>): Prefs {
  const out = { ...DEFAULTS };
  for (const k of Object.keys(DEFAULTS) as Array<keyof Prefs>) {
    const v = p[k];
    if (v === undefined || typeof v !== typeof DEFAULTS[k]) continue;
    (out as Record<string, unknown>)[k] = v;
  }
  out.defaultCount = ([1, 2, 3] as const).includes(out.defaultCount) ? out.defaultCount : 1;
  out.splitMax = SPLIT_SIZES.includes(out.splitMax) ? out.splitMax : DEFAULTS.splitMax;
  out.jobDelay = Math.min(20, Math.max(1, Math.round(out.jobDelay)));
  return out;
}

/** Split used to stop at 4, the old default, which a saved copy of the prefs
 *  keeps; lift it to the new default once. */
const SPLIT_LIFTED = "maestro.prefs.split6";
/** A worktree per agent used to be on by default, which a saved copy keeps;
 *  agents work in the project folder now, so turn it off once. */
const IN_PLACE = "maestro.prefs.inplace";
/** The Grid holds one terminal and cards now, not a terminal each: a saved 4
 *  or 6 (the old defaults) is lifted to the new one once. */
const SPLIT_CARDS = "maestro.prefs.split9";
function lift(raw: Partial<Prefs>): Partial<Prefs> {
  try {
    if (!localStorage.getItem(SPLIT_LIFTED)) {
      localStorage.setItem(SPLIT_LIFTED, "1");
      if ((raw.splitMax as number) === 4) {
        raw = { ...raw, splitMax: DEFAULTS.splitMax };
        localStorage.setItem(KEY, JSON.stringify(raw));
      }
    }
    if (!localStorage.getItem(SPLIT_CARDS)) {
      localStorage.setItem(SPLIT_CARDS, "1");
      if ((raw.splitMax as number) === 4 || (raw.splitMax as number) === 6) {
        raw = { ...raw, splitMax: DEFAULTS.splitMax };
        localStorage.setItem(KEY, JSON.stringify(raw));
      }
    }
    if (!localStorage.getItem(IN_PLACE)) {
      localStorage.setItem(IN_PLACE, "1");
      if (raw.worktree === true) {
        raw = { ...raw, worktree: false };
        localStorage.setItem(KEY, JSON.stringify(raw));
      }
    }
  } catch { /* storage blocked */ }
  return raw;
}

export function getPrefs(): Prefs {
  try {
    return clamp(lift(JSON.parse(localStorage.getItem(KEY) || "{}")));
  } catch {
    return { ...DEFAULTS };
  }
}

export function getPref<K extends keyof Prefs>(k: K): Prefs[K] {
  return getPrefs()[k];
}

export function setPref<K extends keyof Prefs>(k: K, v: Prefs[K]): void {
  const next = clamp({ ...getPrefs(), [k]: v });
  try { localStorage.setItem(KEY, JSON.stringify(next)); } catch { /* storage full or blocked */ }
  for (const cb of listeners) cb(next);
}

export function resetPrefs(): void {
  try { localStorage.removeItem(KEY); } catch { /* ignore */ }
  for (const cb of listeners) cb(getPrefs());
}

/** Be told when any setting changes. */
export function onPrefs(cb: (p: Prefs) => void): () => void {
  listeners.add(cb);
  return () => listeners.delete(cb);
}
