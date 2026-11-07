import { describe, expect, it, vi } from "vitest";
import { keyProblem } from "./settingsingest";

vi.mock("./ipc", () => ({}));
vi.mock("./prefs", () => ({}));

describe("keyProblem", () => {
  it("tells a refused key from an unreachable classifier, in the shape Rust sends", () => {
    expect(keyProblem({ Failed: "http_401" })).toBe("That key was refused.");
    expect(keyProblem({ Failed: "http_403" })).toBe("That key was refused.");
    expect(keyProblem({ Failed: "timeout" })).toBe("Couldn't reach the classifier.");
    expect(keyProblem({ Failed: "network" })).toBe("Couldn't reach the classifier.");
  });

  it("never shows a raw error", () => {
    expect(keyProblem({ Failed: "http_500" })).toBe("The classifier didn't answer.");
    expect(keyProblem(new Error("bad_response"))).toBe("The classifier didn't answer.");
  });
});
