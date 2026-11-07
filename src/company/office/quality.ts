/* How much the office draws. Auto starts with everything on and gives things up, one
 * step per level change that fell under 30 frames a second: first shadows, then glow,
 * then ambient motion. Low is the bottom step, and battery means Low. */
export type OfficeQuality = "auto" | "low" | "high";

export interface Look { shadows: boolean; glow: boolean; ambient: boolean; maxDpr: number }

const STEPS: Look[] = [
  { shadows: true, glow: true, ambient: true, maxDpr: 1.5 },
  { shadows: false, glow: true, ambient: true, maxDpr: 1.5 },
  { shadows: false, glow: false, ambient: true, maxDpr: 1.25 },
  { shadows: false, glow: false, ambient: false, maxDpr: 1 },
];

export const LOW_TIER = STEPS.length - 1;

/** Slower than this, at the middle of a level change, and it is not 30 frames a second. */
const SLOW_MS = 40;
const HITCH_MS = 100;
const MIN_FRAMES = 6;
const KEY = "bytehive.officeQuality";

export const lookOf = (tier: number): Look => STEPS[Math.min(tier, LOW_TIER)];

export function startTier(q: OfficeQuality, battery: boolean): number {
  if (q === "high") return 0;
  return q === "low" || battery ? LOW_TIER : 0;
}

/** After a level change: the tier to go on with, from the gaps between its frames. */
export function steppedDown(tier: number, q: OfficeQuality, gaps: number[]): number {
  if (q !== "auto" || tier >= LOW_TIER || gaps.length < MIN_FRAMES) return tier;
  const middle = [...gaps].sort((a, b) => a - b)[Math.floor(gaps.length / 2)];
  const hitches = gaps.filter((g) => g > HITCH_MS).length;
  return middle > SLOW_MS || hitches > 1 ? tier + 1 : tier;
}

export function readQuality(): OfficeQuality {
  try {
    const v = localStorage.getItem(KEY);
    return v === "low" || v === "high" ? v : "auto";
  } catch {
    return "auto";
  }
}

const listeners = new Set<(q: OfficeQuality) => void>();

/** Tells a running scene the setting changed. Gives back the way to stop. */
export function onQuality(fn: (q: OfficeQuality) => void): () => void {
  listeners.add(fn);
  return () => { listeners.delete(fn); };
}

export function saveQuality(q: OfficeQuality): void {
  try {
    localStorage.setItem(KEY, q);
  } catch { /* the setting just does not stick */ }
  for (const fn of [...listeners]) fn(q);
}

interface Battery extends EventTarget { charging: boolean }

/** Says whether the machine runs on battery, now and on every change. Gives back the way to stop. */
export function watchBattery(fn: (onBattery: boolean) => void): () => void {
  const nav = (typeof navigator === "undefined" ? null : navigator) as (Navigator & { getBattery?: () => Promise<Battery> }) | null;
  if (!nav?.getBattery) return () => {};
  let battery: Battery | null = null;
  let stopped = false;
  const tell = () => { if (battery && !stopped) fn(!battery.charging); };
  void nav.getBattery().then((b) => {
    if (stopped) return;
    battery = b;
    b.addEventListener("chargingchange", tell);
    tell();
  }).catch(() => {});
  return () => {
    stopped = true;
    battery?.removeEventListener("chargingchange", tell);
  };
}
