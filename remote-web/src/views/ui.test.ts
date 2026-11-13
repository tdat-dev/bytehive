import { beforeEach, describe, expect, it } from "vitest";
import { themeButton } from "./ui";

describe("the theme", () => {
  beforeEach(() => {
    localStorage.clear();
    document.documentElement.dataset.theme = "dark";
  });

  it("changes only when pressed, and is kept for the next visit", () => {
    const b = themeButton();
    expect(b.textContent).toBe("Light theme");
    b.click();
    expect(document.documentElement.dataset.theme).toBe("light");
    expect(localStorage.getItem("bytehive.theme")).toBe("light");
    expect(b.textContent).toBe("Dark theme");
    b.click();
    expect(document.documentElement.dataset.theme).toBe("dark");
  });
});
