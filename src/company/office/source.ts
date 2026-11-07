/* What the office can see and do. The engine stands behind it in the app, fixtures in the demo. */
import type { OfficeModel } from "./model";
import type { DeskReport } from "./report";

export interface Snapshot {
  model: OfficeModel | null;
  /** Why there is no model: "The Company is running in another ByteHive window." */
  note: string | null;
  /** A load failure, in words. */
  error: string | null;
}

/** All the office can see or do. The engine behind it in the app, fixtures in the demo. */
export interface OfficeSource {
  snapshot(now: number): Snapshot;
  onChange(fn: () => void): () => void;
  giveJob(ask: string): Promise<void>;
  messageFloor(floorId: string, text: string): Promise<void>;
  approve(job: string): Promise<void>;
  reject(job: string, note: string): Promise<void>;
  raiseCap(job: string, extra: number): Promise<void>;
  finishJob(job: string): Promise<void>;
  stopJob(job: string): Promise<void>;
  dismiss(key: string): void;
  /** Put a person's real pane on the stage. False when they have none. */
  showPerson(id: string): boolean;
  wakePerson(id: string): Promise<void>;
  report(personId: string): Promise<DeskReport | null>;
  /** Tell the source the office is on screen: it reads boards only then. */
  watching(on: boolean): void;
}
