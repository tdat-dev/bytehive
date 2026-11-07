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
const gpu = { name: "ANGLE (Intel, Intel(R) UHD Graphics 630 Direct3D11 vs_5_0 ps_5_0)", lost: false, noContext: false };
const glStub = {
  RENDERER: 1,
  getExtension: () => ({ UNMASKED_RENDERER_WEBGL: 2 }),
  getParameter: () => gpu.name,
  isContextLost: () => gpu.lost,
};
const lights: Array<{ shadow: { dispose(): void } }> = [];

vi.mock("three", async (real) => {
  const three = await real<typeof import("three")>();
  class WebGLRenderer {
    shadowMap = { enabled: false, type: 0, autoUpdate: true, needsUpdate: false };
    debug = { checkShaderErrors: true };
    constructor() {
      calls.push("context");
      if (gpu.noContext) throw new Error("Error creating WebGL context.");
    }
    getContext() { return glStub; }
    info = { render: { calls: 0, triangles: 0 } };
    getDrawingBufferSize(v: { x: number; y: number }) { return v; }
    setSize(w: number, h: number) { calls.push(`size ${w}x${h}`); }
    render() { calls.push("render"); }
    setPixelRatio() {}
    dispose() { calls.push("dispose"); }
    forceContextLoss() { calls.push("contextLoss"); }
    forceContextRestore() { calls.push("contextRestore"); }
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
  // eslint-disable-next-line require-yield
  prepareFloor: function* () {},
  // eslint-disable-next-line require-yield
  prepareTop: function* () {},
}));

let queued: FrameRequestCallback[];
const runFrames = (n = 1) => {
  for (let i = 0; i < n; i += 1) {
    vi.advanceTimersByTime(140);
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
  Object.assign(gpu, { name: "ANGLE (Intel, Intel(R) UHD Graphics 630 Direct3D11 vs_5_0 ps_5_0)", lost: false, noContext: false });
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

describe("going away", () => {
  const AWAY = 120_000;
  const lostAndBack = (canvas: HTMLCanvasElement) => {
    canvas.dispatchEvent(new Event("webglcontextlost"));
    runFrames(1);
  };

  it("draws nothing while hidden", () => {
    const { scene } = mount();
    settle(scene);
    scene.setVisible(false);
    vi.advanceTimersByTime(5000);
    runFrames(3);
    expect(calls).not.toContain("render");
  });
  it("gives back the GPU after two minutes away, and not before", () => {
    const { scene } = mount();
    settle(scene);
    scene.setVisible(false);
    vi.advanceTimersByTime(AWAY - 1000);
    expect(calls).not.toContain("contextLoss");
    vi.advanceTimersByTime(1000);
    expect(calls).toContain("contextLoss");
  });
  it("stays awake if the owner comes back in time", () => {
    const { scene } = mount();
    settle(scene);
    scene.setVisible(false);
    vi.advanceTimersByTime(AWAY / 2);
    scene.setVisible(true);
    vi.advanceTimersByTime(AWAY);
    expect(calls).not.toContain("contextLoss");
  });
  it("asks for the context back on return, and draws once it has it", () => {
    const { scene, canvas } = mount();
    settle(scene);
    scene.setVisible(false);
    vi.advanceTimersByTime(AWAY);
    lostAndBack(canvas);
    calls.length = 0;
    scene.setVisible(true);
    expect(calls).toContain("contextRestore");
    canvas.dispatchEvent(new Event("webglcontextrestored"));
    runFrames(1);
    expect(calls).toContain("render");
  });
  it("does not tell the shell the picture is lost when it let go itself", () => {
    const canvas = document.createElement("canvas");
    const onLost = vi.fn();
    const onRestored = vi.fn();
    const scene = createScene(canvas, { reducedMotion: true, dpr: 1, onLost, onRestored });
    settle(scene);
    scene.setVisible(false);
    vi.advanceTimersByTime(AWAY);
    lostAndBack(canvas);
    scene.setVisible(true);
    canvas.dispatchEvent(new Event("webglcontextrestored"));
    expect(onLost).not.toHaveBeenCalled();
    expect(onRestored).not.toHaveBeenCalled();
  });
});

describe("the one context", () => {
  const make = (over: object = {}) => createScene(document.createElement("canvas"), { reducedMotion: true, dpr: 1, onLost: () => {}, onRestored: () => {}, ...over });

  it("is made once, and is the one the scene draws with", () => {
    make();
    expect(calls.filter((c) => c === "context")).toHaveLength(1);
  });
  it("fails as it always did with no WebGL at all", () => {
    gpu.noContext = true;
    expect(() => make()).toThrow(/WebGL/);
  });
  it.each(["Google SwiftShader", "ANGLE (Microsoft, Microsoft Basic Render Driver Direct3D11)", "llvmpipe (LLVM 15.0.7)"])("refuses %s, and gives its context back", (name) => {
    gpu.name = name;
    expect(() => make()).toThrow(`No usable WebGL (${name}).`);
    expect(calls).toContain("dispose");
    expect(calls).toContain("contextLoss");
  });
  it("takes a software renderer when told to", () => {
    gpu.name = "Google SwiftShader";
    expect(() => make({ allowSoftware: true })).not.toThrow();
    expect(calls).not.toContain("contextLoss");
  });
  it("refuses a context that was lost as it was made, even a software one that is allowed", () => {
    gpu.lost = true;
    expect(() => make({ allowSoftware: true })).toThrow("No usable WebGL (context lost).");
  });
  it("builds nothing when it refuses", () => {
    gpu.name = "Google SwiftShader";
    expect(() => make()).toThrow();
    expect(calls).not.toContain("kit");
  });
});

describe("opening", () => {
  it("makes rooms between frames, never inside setModel", () => {
    const { scene } = mount();
    scene.resize(800, 600);
    scene.setModel(demoModel({ quiet: true }));
    expect(buildFloor).not.toHaveBeenCalled();
    runFrames(2);
    expect(buildFloor).toHaveBeenCalled();
    expect(buildTop).toHaveBeenCalled();
  });
  it("goes on with the newest model if one comes while it works", () => {
    const { scene } = mount();
    scene.resize(800, 600);
    scene.setModel(demoModel({ quiet: true }));
    scene.setModel(demoModel());
    runFrames(2);
    const last = vi.mocked(buildFloor).mock.calls.slice(-1)[0][0];
    expect(demoModel().floors.map((f) => f.id)).toContain(last.id);
    expect(() => runFrames(3)).not.toThrow();
  });
  it("stops making rooms once it is gone", () => {
    const { scene } = mount();
    scene.resize(800, 600);
    scene.setModel(demoModel({ quiet: true }));
    scene.dispose();
    runFrames(2);
    expect(buildFloor).not.toHaveBeenCalled();
  });
});

describe("what keeps it drawing", () => {
  it("stops the ambient loop at a Desk: the terminal comes first", () => {
    const canvas = document.createElement("canvas");
    const scene = createScene(canvas, { reducedMotion: false, dpr: 1, onLost: () => {}, onRestored: () => {} });
    settle(scene);
    vi.advanceTimersByTime(500);
    runFrames(2);
    expect(calls).toContain("render");
    scene.setLevel({ kind: "desk", personId: "linh", floorId: "bytehive", tab: "terminal" }, true);
    runFrames(2);
    calls.length = 0;
    vi.advanceTimersByTime(2000);
    runFrames(3);
    expect(calls).not.toContain("render");
  });
  it("has no ambient loop on Low", () => {
    const canvas = document.createElement("canvas");
    const scene = createScene(canvas, { reducedMotion: false, dpr: 1, onLost: () => {}, onRestored: () => {}, quality: "low" });
    settle(scene);
    vi.advanceTimersByTime(2000);
    runFrames(3);
    expect(calls).not.toContain("render");
  });
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
