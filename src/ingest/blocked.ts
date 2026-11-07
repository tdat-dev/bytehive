/** The messages the gate held back, kept with their text so they can be sent
 *  again. Writes run one at a time: a block landing while the panel deletes a
 *  row must not lose either change. */
import type { BlockedEntry } from "./types";

export interface BlockedIO {
  read(): Promise<string | null>;
  write(text: string): Promise<void>;
}

export interface BlockedList {
  entries: BlockedEntry[];
  /** The file exists but can't be read as a list, so an empty list would be a lie. */
  damaged: boolean;
}

const CAP = 100;
/** Small talk nobody rescued in a week is not going to be. */
const KEEP_MS = 7 * 86_400_000;

/** The pane to deliver a held message to now: the one it was meant for, or the
 *  same agent under a new pane after a restart. Null when that agent isn't
 *  running, so nothing claims a delivery that can't happen. */
export function liveTarget(panes: Array<{ id: string; name: string; running: boolean }>, entry: BlockedEntry): string | null {
  const running = panes.filter((p) => p.running);
  return (running.find((p) => p.id === entry.to) ?? running.find((p) => p.name === entry.toName))?.id ?? null;
}

export class BlockedStore {
  private tail: Promise<unknown> = Promise.resolve();

  constructor(
    private readonly io: BlockedIO,
    private readonly now: () => number = Date.now,
  ) {}

  private turn<T>(work: () => Promise<T>): Promise<T> {
    const run = this.tail.then(work, work);
    this.tail = run.catch(() => {});
    return run;
  }

  private async read(): Promise<BlockedList> {
    const text = await this.io.read();
    if (!text) return { entries: [], damaged: false };
    try {
      const file = JSON.parse(text) as { entries?: BlockedEntry[] };
      // A held card stays: it still sits on the board waiting for this message.
      if (Array.isArray(file.entries)) return { entries: file.entries.filter((e) => e.card || this.now() - e.at < KEEP_MS), damaged: false };
    } catch {
      /* reported below */
    }
    return { entries: [], damaged: true };
  }

  private write(entries: BlockedEntry[]): Promise<void> {
    return this.io.write(JSON.stringify({ version: 1, entries: entries.slice(-CAP) }));
  }

  list(): Promise<BlockedList> {
    return this.turn(() => this.read());
  }

  add(entry: BlockedEntry): Promise<void> {
    return this.turn(async () => {
      const { entries } = await this.read();
      await this.write([...entries.filter((e) => e.id !== entry.id), entry].sort((a, b) => a.at - b.at));
    });
  }

  /** Take one entry out and hand it back, or null when it is already gone. */
  remove(id: string): Promise<BlockedEntry | null> {
    return this.turn(async () => {
      const { entries } = await this.read();
      const hit = entries.find((e) => e.id === id) ?? null;
      if (hit) await this.write(entries.filter((e) => e.id !== id));
      return hit;
    });
  }
}
