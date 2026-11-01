import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { hubFile } from "./browser.js";

describe("hubFile", () => {
  let home: string;
  // An agent started by ByteHive (or the old Maestro) has these set; start clean.
  const saved = { a: process.env.BYTEHIVE_HUB_FILE, b: process.env.MAESTRO_HUB_FILE };
  beforeEach(() => {
    delete process.env.BYTEHIVE_HUB_FILE;
    delete process.env.MAESTRO_HUB_FILE;
    home = fs.mkdtempSync(path.join(os.tmpdir(), "hub-home-"));
  });
  afterEach(() => {
    fs.rmSync(home, { recursive: true, force: true });
    if (saved.a !== undefined) process.env.BYTEHIVE_HUB_FILE = saved.a;
    if (saved.b !== undefined) process.env.MAESTRO_HUB_FILE = saved.b;
  });

  it("is the hub of the ByteHive that started this agent", () => {
    // a test build of ByteHive keeps its own hub; its agents must not dial the installed one
    process.env.BYTEHIVE_HUB_FILE = "C:\\Users\\me\\.bytehive\\browser-hub.dev.json";
    expect(hubFile(home)).toBe("C:\\Users\\me\\.bytehive\\browser-hub.dev.json");
    delete process.env.BYTEHIVE_HUB_FILE;
    process.env.MAESTRO_HUB_FILE = "C:\\old.json";
    expect(hubFile(home)).toBe("C:\\old.json");
  });

  it("falls back to the installed ByteHive's hub, else the old Maestro's", () => {
    expect(hubFile(home)).toBe(path.join(home, ".bytehive", "browser-hub.json"));
    fs.mkdirSync(path.join(home, ".maestro"));
    fs.writeFileSync(path.join(home, ".maestro", "browser-hub.json"), "{}");
    expect(hubFile(home)).toBe(path.join(home, ".maestro", "browser-hub.json"));
    fs.mkdirSync(path.join(home, ".bytehive"));
    fs.writeFileSync(path.join(home, ".bytehive", "browser-hub.json"), "{}");
    expect(hubFile(home)).toBe(path.join(home, ".bytehive", "browser-hub.json"));
  });
});
