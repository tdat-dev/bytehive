// @vitest-environment happy-dom
import { describe, it, expect, vi } from "vitest";

vi.mock("./xtermpause", () => ({ skipIdleSelectionRedraw: vi.fn(() => true) }));

import { mountTerminal } from "./terminal";
import { skipIdleSelectionRedraw } from "./xtermpause";

describe("mountTerminal", () => {
  it("puts every terminal behind the idle selection redraw guard", () => {
    const div = document.createElement("div");
    document.body.appendChild(div);
    const handle = mountTerminal(div, () => {}, () => {}, { webgl: false });
    expect(skipIdleSelectionRedraw).toHaveBeenCalledTimes(1);
    handle.dispose();
  });
});
