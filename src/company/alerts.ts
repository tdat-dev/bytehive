/* What the owner has to hear that no watch will bring up again: a file that won't
 * read, a member who starts blank. These stand until they are dismissed or stop
 * being true. No imports from the engine, so a pane or the store can raise one. */
import type { Alert } from "./watch";

const standing = new Map<string, Alert>();
const listeners = new Set<() => void>();
const changed = () => listeners.forEach((fn) => fn());

export const onStandingChange = (fn: () => void) => { listeners.add(fn); };
export const standingAlerts = (): Alert[] => [...standing.values()];

export function raiseAlert(key: string, text: string, personId: string | null = null): void {
  standing.set(key, { key, job: null, personId, text, level: "warn", standing: true });
  changed();
}

export function clearAlert(key: string): void {
  if (standing.delete(key)) changed();
}
