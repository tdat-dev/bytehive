// A scene's life on a fake renderer: headless has no WebGL, and what matters here is
// which calls reach the renderer, and in what order.
import { Group } from "three";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { demoModel } from "./fixtures";
import { createScene, retargeted, type OfficeScene } from "./scene";
import { BUILDING } from "./nav";
import { buildFloor, buildTop } from "./build";
import type { Shot } from "./layout";

const calls: string[] = [];
const lights: Array<{ shadow: { dispose(): void } }> = [];

vi.mock("three", async (real) => {
  const three = await real<typeof import("three")>();
  class WebGLRenderer {
    shadowMap = { enabled: false, type: 0, autoUpdate: true, needsUpdate: false };
    info = { render: { calls: 0, triangles: 0 } };
    setPixelRatio() {}
    getDrawingBufferSize(v: { x: number; y: number }) { return v; }
    setSize(w: number, h: number) { calls.push(`size ${w}x${h}`); }
    render() { calls.push("render"); }
    dispose() { calls.push("dispose"); }
    forceContextLoss() { calls.push("contextLoss"); }
  }
  class DirectionalLight extends three.DirectionalLight {
    constructor(...a: ConstructorParameters<typeof three.DirectionalLight>) {
      super(...a);
      lights.push(this);
    }
  }
  return { ...three, WebGLRenderer, DirectionalLight };
});

vi.mock("./build", async (real) => ({
  ...(await real<typeof import("./build")>()),
  createKit: () => ({ ground: new Group(), groundLight: new Group(), dispose: () => { calls.push("kit"); } }),
  buildFloor: vi.fn(() => new Group()),
  buildTop: vi.fn(() => new Group()),
}));

let queued: FrameRequestCallback[];
const runFrames = (n = 1) => {
  for (let i = 0; i < n; i += 1) {
    vi.runOnlyPendingTimers();
    for (const fn of queued.splice(0)) fn(performance.now());
  }
};

function mount() {
  const canvas = document.createElement("canvas");
  const onRestored = vi.fn();
  const scene = createScene(canvas, { reducedMotion: true, dpr: 1, onLost: () => {}, onRestored });
  return { canvas, scene, onRestored };
}

const settle = (s: OfficeScene) => {
  s.resize(800, 600);
  s.setModel(demoModel({ quiet: true }));
  runFrames(2);
  calls.length = 0;
};

beforeEach(() => {
  vi.useFakeTimers();
  calls.length = 0;
  lights.length = 0;
  queued = [];
  vi.mocked(buildFloor).mockClear();
  vi.mocked(buildTop).mockClear();
  vi.stubGlobal("requestAnimationFrame", (fn: FrameRequestCallback) => queued.push(fn));
  vi.stubGlobal("cancelAnimationFrame", () => { queued = []; });
});

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

describe("dispose", () => {
  it("frees the shadow map and gives the context back", () => {
    const { scene } = mount();
    settle(scene);
    const shadow = vi.spyOn(lights[0].shadow, "dispose");
    scene.dispose();
    expect(shadow).toHaveBeenCalledTimes(1);
    expect(calls).toContain("dispose");
    expect(calls).toContain("contextLoss");
  });

  it("gives the context back after the renderer has let go of it", () => {
    const { scene } = mount();
    settle(scene);
    scene.dispose();
    expect(calls.indexOf("dispose")).toBeLessThan(calls.indexOf("contextLoss"));
  });

  it("draws and builds nothing for what arrives late", () => {
    const { scene } = mount();
    settle(scene);
    scene.dispose();
    calls.length = 0;
    vi.mocked(buildFloor).mockClear();
    vi.mocked(buildTop).mockClear();

    scene.resize(640, 400);
    scene.setModel(demoModel());
    scene.setLevel(BUILDING, true);
    scene.setVisible(true);
    runFrames(3);

    expect(calls).toEqual([]);
    expect(buildFloor).not.toHaveBeenCalled();
    expect(buildTop).not.toHaveBeenCalled();
  });

  it("forgets who wanted to hear the camera stop", () => {
    const { scene } = mount();
    const heard = vi.fn();
    scene.onMoved(heard);
    settle(scene);
    heard.mockClear();
    scene.dispose();
    scene.setLevel(BUILDING, true);
    runFrames(3);
    expect(heard).not.toHaveBeenCalled();
  });

  it("can be called twice", () => {
    const { scene } = mount();
    settle(scene);
    scene.dispose();
    calls.length = 0;
    scene.dispose();
    expect(calls).toEqual([]);
  });
});

describe("onMoved", () => {
  it("stops calling once unsubscribed, and leaves the others alone", () => {
    const { scene } = mount();
    const gone = vi.fn();
    const stays = vi.fn();
    const off = scene.onMoved(gone);
    scene.onMoved(stays);
    settle(scene);
    gone.mockClear();
    stays.mockClear();
    off();
    scene.resize(640, 400);
    runFrames(2);
    expect(gone).not.toHaveBeenCalled();
    expect(stays).toHaveBeenCalled();
  });
});

describe("a restored context", () => {
  it("sets the size before it draws", () => {
    const { canvas, scene } = mount();
    settle(scene);
    canvas.dispatchEvent(new Event("webglcontextlost", { cancelable: true }));
    canvas.dispatchEvent(new Event("webglcontextrestored"));
    expect(calls.slice(0, 2)).toEqual(["size 800x600", "render"]);
  });

  it("does not draw while hidden, and draws when shown again", () => {
    const { canvas, scene, onRestored } = mount();
    settle(scene);
    scene.setVisible(false);
    canvas.dispatchEvent(new Event("webglcontextlost", { cancelable: true }));
    canvas.dispatchEvent(new Event("webglcontextrestored"));
    runFrames(2);
    expect(calls).not.toContain("render");
    expect(onRestored).toHaveBeenCalledTimes(1);
    scene.setVisible(true);
    runFrames(2);
    expect(calls).toContain("render");
  });
});

describe("retargeted", () => {
  const shot = (px: number): Shot => ({ px, py: 0, pz: 0, tx: 0, ty: 0, tz: 0, fov: 15 });
  it("carries on from where the camera is now, not from where the move began", () => {
    const moving = { from: shot(0), to: shot(10), at: 100 };
    expect(retargeted(moving, shot(4), shot(20))).toEqual({ from: shot(4), to: shot(20), at: null });
  });
});
