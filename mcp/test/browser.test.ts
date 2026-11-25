// @vitest-environment node
import { describe, it, expect } from "vitest";
import { HubClient } from "../src/browser.js";

describe("browser tools", () => {
  it("passes on why no hub could be found", async () => {
    const hub = new HubClient("Ana", async () => {
      throw new Error("The ByteHive that started this agent is closed");
    });
    const r = await hub.call("tabs", {});
    expect(r.isError).toBe(true);
    expect(r.content[0]).toMatchObject({ type: "text", text: expect.stringContaining("started this agent is closed") });
  });

  it("reports a closed port as a connection problem, not a hang", async () => {
    const hub = new HubClient("Ana", async () => ({ port: 1, token: "t" }));
    const r = await hub.call("tabs", {});
    expect(r.isError).toBe(true);
  }, 15_000); // Windows takes ~2s to refuse a localhost connection
});

import { uploadPaths, recordingPath, saveRecording } from "../src/browser.js";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

describe("upload and recording paths", () => {
  it("resolves uploads from the workspace and refuses files that aren't there", () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), "mcp-up-"));
    fs.writeFileSync(path.join(dir, "a.png"), "x");
    expect(uploadPaths(["a.png"], dir)).toEqual([path.join(dir, "a.png")]);
    expect(() => uploadPaths(["missing.png"], dir)).toThrow(/No such file/);
  });

  it("saves recordings under .bytehive/recordings, named after the agent and the time", () => {
    const at = new Date("2026-09-25T10:20:30Z");
    expect(recordingPath("D:/w", "Ana Lê", ".gif", at)).toBe(path.join("D:/w", ".bytehive", "recordings", "Ana_L_-2026-09-25T10-20-30.gif"));
    expect(recordingPath("D:/w", "Ana Lê", "", at)).toBe(path.join("D:/w", ".bytehive", "recordings", "Ana_L_-2026-09-25T10-20-30"));
  });
});

describe("saving a video recording", () => {
  /** A browser holding files, answering parts of at most `part` bytes. */
  const fakeHub = (files: Record<string, Buffer>, part: number) => {
    const calls: Record<string, unknown>[] = [];
    return {
      calls,
      call: async (_tool: string, a: Record<string, unknown>) => {
        calls.push(a);
        if (a.action === "done") return { content: [] };
        const all = files[a.name as string];
        if (!all) return { content: [{ type: "text" as const, text: "No such file" }], isError: true };
        const o = a.offset as number;
        return { content: [], part: all.subarray(o, o + part).toString("base64") };
      },
    };
  };

  it("pulls each file in parts into the folder, frames/ included, then frees it", async () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), "mcp-rec-"));
    const video = Buffer.from(Array.from({ length: 10 }, (_, i) => i));
    const hub = fakeHub({ "video.mp4": video, "timeline.md": Buffer.from("# t\n"), "frames/001-0m00.0s-start.jpg": Buffer.from("jpg") }, 4);
    const total = await saveRecording(hub, dir, [
      { name: "video.mp4", size: 10 },
      { name: "timeline.md", size: 4 },
      { name: "frames/001-0m00.0s-start.jpg", size: 3 },
    ]);
    expect(total).toBe(17);
    expect(fs.readFileSync(path.join(dir, "video.mp4"))).toEqual(video);
    expect(fs.readFileSync(path.join(dir, "frames", "001-0m00.0s-start.jpg"), "utf8")).toBe("jpg");
    expect(hub.calls.filter((c) => c.name === "video.mp4").map((c) => c.offset)).toEqual([0, 4, 8]);
    expect(hub.calls.at(-1)).toEqual({ action: "done" });
  });

  it("refuses a name that leaves the folder, and still frees the recording", async () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), "mcp-rec-"));
    const hub = fakeHub({}, 4);
    await expect(saveRecording(hub, dir, [{ name: "../evil.txt", size: 1 }])).rejects.toThrow(/Bad file name/);
    expect(hub.calls).toEqual([{ action: "done" }]);
  });
});
