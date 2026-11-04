import os from "node:os";
import path from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { hubFile } from "./browser.js";

describe("hubFile", () => {
  afterEach(() => { delete process.env.BYTEHIVE_HUB_FILE; });

  it("is the hub of the ByteHive that started this agent", () => {
    // a test build of ByteHive keeps its own hub; its agents must not dial the installed one
    process.env.BYTEHIVE_HUB_FILE = "C:\\Users\\me\\.bytehive\\browser-hub.dev.json";
    expect(hubFile()).toBe("C:\\Users\\me\\.bytehive\\browser-hub.dev.json");
  });

  it("falls back to the installed ByteHive's hub for an agent started elsewhere", () => {
    expect(hubFile()).toBe(path.join(os.homedir(), ".bytehive", "browser-hub.json"));
  });
});
