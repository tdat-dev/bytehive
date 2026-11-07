// src/company/office/textures.test.ts
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { LOGO_SVG, badgeInk, badgeLetter } from "./textures";

describe("LOGO_SVG", () => {
  it("stays the logo, byte for byte", () => {
    expect(Buffer.from(LOGO_SVG, "utf8").equals(readFileSync("design/logo.svg"))).toBe(true);
  });
});

describe("badgeLetter", () => {
  it("is the CLI's first letter, in capitals", () => {
    expect(badgeLetter("shell")).toBe("S");
    expect(badgeLetter("my-tool")).toBe("M");
  });
  it("looks past what is not a letter", () => {
    expect(badgeLetter("@acme/tool")).toBe("A");
  });
  it("is a question mark with nothing to go on", () => {
    expect(badgeLetter("")).toBe("?");
    expect(badgeLetter("---")).toBe("?");
  });
});

describe("badgeInk", () => {
  it("darkens a logo drawn in white, which the card would hide", () => {
    expect(badgeInk("grok")).toBe("#14161c");
  });
  it("leaves a logo with its own colours alone", () => {
    expect(badgeInk("claude")).toBe("#ffffff");
  });
});
