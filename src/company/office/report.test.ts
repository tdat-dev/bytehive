import { describe, expect, it } from "vitest";
import type { Board } from "../../board";
import type { Job } from "../types";
import { deskReport } from "./report";

const card = (id: string, over: object) => ({ id, title: id, desc: "", labels: [], due: null, checklist: [], ...over });
const board: Board = { lists: [
  { id: "1", title: "Doing", cards: [card("mine", { assigneeId: "p1", job: "J-1", checklist: [{ id: "i", text: "Test", done: true }] }), card("other", { assigneeId: "p2" })] },
  { id: "2", title: "Done", cards: [card("done", { done: { repoRoot: "x", files: ["a.ts"], summary: "Fixed", at: 1, by: "Ana" } })] },
] } as Board;

describe("deskReport", () => {
  const me = { id: "p1", name: "Ana", branch: "bytehive/ana" };
  it("lists this person's cards, by id or by who finished them", () => {
    expect(deskReport(me, board, []).cards.map((c) => c.title)).toEqual(["mine", "done"]);
  });
  it("carries the evidence, the checklist and the Job", () => {
    const r = deskReport(me, board, [{ id: "J-1", title: "Add login" } as Job]);
    expect(r.cards[0]).toMatchObject({ list: "Doing", job: "J-1", jobTitle: "Add login", checklist: [{ text: "Test", done: true }] });
    expect(r.cards[1]).toMatchObject({ summary: "Fixed", files: ["a.ts"] });
  });
  it("is empty with no board", () => {
    expect(deskReport(me, null, [])).toEqual({ name: "Ana", branch: "bytehive/ana", cards: [] });
  });
});
