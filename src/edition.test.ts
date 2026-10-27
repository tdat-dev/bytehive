import { describe, expect, it } from "vitest";
import { APP_NAME, EDITION, isSupervisor } from "./edition";

describe("edition", () => {
  it("defaults to regular Maestro when VITE_EDITION is unset", () => {
    expect(EDITION).toBe("maestro");
    expect(isSupervisor).toBe(false);
    expect(APP_NAME).toBe("Maestro");
  });
});
