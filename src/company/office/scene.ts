/* The office in three.js. It draws the model and holds nothing else that matters: drop
 * it and call setModel again and the picture is the same. It reads nothing from the
 * page but the canvas it is given, so it can move to a Worker with an OffscreenCanvas. */
import {
  ACESFilmicToneMapping, BoxGeometry, Color, DirectionalLight, Group, HemisphereLight, Mesh, MeshBasicMaterial, PCFSoftShadowMap, PerspectiveCamera,
  Raycaster, Scene, SRGBColorSpace, Vector2, Vector3, WebGLRenderer,
} from "three";
import { FLOOR_BOX, FLOOR_EDGE, buildFloor, buildTop, createKit, disposeBuilt } from "./build";
import { FOUNDER_DESK, FOV, HEAD_Y, chairSpot, mixShots, shotFor, towerLayout, type FloorSpot, type Shot, type Tower } from "./layout";
import { sceneKey, type OfficeFloor, type OfficeModel } from "./model";
import { BUILDING, frameDelay, type Level } from "./nav";
import { OFFICE } from "./palette";

export interface SceneOptions {
  reducedMotion: boolean;
  dpr: number;
  onLost(): void;       // context lost
  onRestored(): void;   // context back, picture drawn unless the scene is hidden
}
export interface Anchor { id: string; kind: "floor" | "person" | "top"; x: number; y: number; visible: boolean } // CSS pixels in the canvas
export interface SceneStats { calls: number; triangles: number; frames: number; lost: number; restored: number; lastRestoreMs: number | null; drawW: number; drawH: number }
export interface OfficeScene {
  setModel(m: OfficeModel): void;
  setLevel(l: Level, instant: boolean): void;
  resize(w: number, h: number): void;
  setVisible(on: boolean): void;
  pick(x: number, y: number): { personId?: string; floorId?: string } | null;
  anchors(): Anchor[];
  onMoved(fn: () => void): () => void;  // the camera came to rest: labels may be placed. Gives back the way to stop hearing it
  stats(): SceneStats;
  dispose(): void;
}

const MOVE_MS = 900;
const PICK_LAYER = 1;
const MAX_DPR = 1.5;
const SHADOW_MAP = 1024;
const PERSON_BOX = { w: 0.9, h: 1.7, d: 0.9 };
const TAG_Y = 1.5;
const SUN = new Vector3(0.25, 0.45, 1).normalize();

export const goodRenderer = (name: string): boolean => !/swiftshader|llvmpipe|basic render|software/i.test(name);

/** Can this machine draw the office? A throwaway context, given back at once. */
export function webglProbe(allowSoftware = false): { ok: boolean; renderer: string } {
  if (typeof OffscreenCanvas === "undefined") return { ok: false, renderer: "" };
  const gl = new OffscreenCanvas(1, 1).getContext("webgl2");
  if (!gl) return { ok: false, renderer: "" };
  const info = gl.getExtension("WEBGL_debug_renderer_info");
  const renderer = String(gl.getParameter(info ? info.UNMASKED_RENDERER_WEBGL : gl.RENDERER));
  gl.getExtension("WEBGL_lose_context")?.loseContext();
  return { ok: allowSoftware || goodRenderer(renderer), renderer };
}

export const easeInOutCubic = (k: number): number => (k < 0.5 ? 4 * k ** 3 : 1 - (-2 * k + 2) ** 3 / 2);

/** From normalised device coordinates to CSS pixels. Past z = 1 is behind the camera. */
export function cssPoint(p: { x: number; y: number; z: number }, w: number, h: number): { x: number; y: number; visible: boolean } {
  return { x: ((p.x + 1) / 2) * w, y: ((1 - p.y) / 2) * h, visible: Math.abs(p.x) <= 1 && Math.abs(p.y) <= 1 && Math.abs(p.z) <= 1 };
}

export interface PickHit { personId?: string; floorId?: string; distance: number }

/** What a ray means. Everyone stands inside a floor's box, so the first thing it enters
 *  is the floor it can see, and a person on any other floor is behind a slab. */
export function pickFrom(hits: PickHit[]): { personId?: string; floorId?: string } | null {
  const near = [...hits].sort((a, b) => a.distance - b.distance);
  const first = near[0];
  if (!first) return null;
  if (first.personId && !first.floorId) return { personId: first.personId };
  const person = near.find((h) => h.personId && h.floorId === first.floorId);
  return person ? { personId: person.personId, floorId: person.floorId } : { floorId: first.floorId };
}

interface Tween { from: Shot; to: Shot; at: number | null }

/** A move given a new end: it goes on from where the camera is, so nothing jumps. */
export const retargeted = (t: Tween, now: Shot, to: Shot): Tween => ({ ...t, from: now, to, at: null });

const sameShot = (a: Shot, b: Shot) => (Object.keys(a) as Array<keyof Shot>).every((k) => Math.abs(a[k] - b[k]) < 1e-6);

/** What a floor's meshes depend on. A floor whose key stands still is left alone. */
const floorKey = (f: OfficeFloor, spot: FloorSpot) => JSON.stringify([spot.y, f.lit, f.board, f.people.map((p) => [p.id, p.state, p.seat]), spot.desks]);

export function createScene(canvas: HTMLCanvasElement, o: SceneOptions): OfficeScene {
  const renderer = new WebGLRenderer({ canvas, antialias: true, powerPreference: "low-power", preserveDrawingBuffer: false });
  renderer.setPixelRatio(Math.min(o.dpr, MAX_DPR));
  renderer.outputColorSpace = SRGBColorSpace;
  renderer.toneMapping = ACESFilmicToneMapping;
  renderer.toneMappingExposure = 1.05;
  renderer.shadowMap.enabled = true;
  renderer.shadowMap.type = PCFSoftShadowMap;
  renderer.shadowMap.autoUpdate = false;

  const scene = new Scene();
  scene.background = new Color(OFFICE.sky);
  const camera = new PerspectiveCamera(FOV, 1, 1, 800);
  scene.add(new HemisphereLight(0xe4ebf5, 0x343a48, 2.8));
  const sun = new DirectionalLight(0xffe2b0, 2);
  sun.castShadow = true;
  sun.shadow.mapSize.set(SHADOW_MAP, SHADOW_MAP);
  sun.shadow.bias = -0.0006;
  sun.shadow.normalBias = 0.03;
  scene.add(sun, sun.target);

  const kit = createKit();
  scene.add(kit.ground);
  const floors = new Map<string, { key: string; group: Group }>();
  let top: { key: string; group: Group } | null = null;

  const proxies = new Group();
  scene.add(proxies);
  const proxyBox = new BoxGeometry(1, 1, 1);
  const proxyMaterial = new MeshBasicMaterial();
  const ray = new Raycaster();
  ray.layers.set(PICK_LAYER);

  let tower: Tower | null = null;
  let model: OfficeModel | null = null;
  let lastKey: string | null = null;
  let level: Level = BUILDING;
  let width = 1;
  let height = 1;
  let shot: Shot | null = null;
  let tween: Tween | null = null;
  let anyoneWorking = false;

  let visible = true;
  let disposed = false;
  let dirty = false;
  let lost = false;
  let lostAt = 0;
  let timer: ReturnType<typeof setTimeout> | null = null;
  let raf: number | null = null;
  let lastDraw = 0;
  let resting = false;
  let frames = 0;
  let lostCount = 0;
  let restored = 0;
  let lastRestoreMs: number | null = null;
  const moved = new Set<() => void>();

  const moving = () => tween !== null;

  /* The whole of the on-demand rule: a frame only when something asks for one. */
  function schedule(): void {
    if (disposed || timer !== null || raf !== null || !visible || lost) return;
    const delay = frameDelay({ visible, moving: moving(), ambient: anyoneWorking, reduced: o.reducedMotion });
    if (delay === null && !dirty) return;
    const wait = Math.max(0, (delay ?? 0) - (performance.now() - lastDraw));
    timer = setTimeout(() => {
      timer = null;
      raf = requestAnimationFrame(frame);
    }, wait);
  }

  function halt(): void {
    if (timer !== null) clearTimeout(timer);
    if (raf !== null) cancelAnimationFrame(raf);
    timer = null;
    raf = null;
  }

  function draw(): void {
    renderer.render(scene, camera);
    lastDraw = performance.now();
    frames += 1;
    dirty = false;
  }

  function frame(t: number): void {
    raf = null;
    if (lost || !visible) return;
    advance(t);
    draw();
    if (resting) {
      resting = false;
      for (const fn of [...moved]) fn();
    }
    schedule();
  }

  function invalidate(): void {
    if (disposed) return;
    dirty = true;
    schedule();
  }

  function look(s: Shot): void {
    shot = s;
    camera.position.set(s.px, s.py, s.pz);
    camera.fov = s.fov;
    camera.updateProjectionMatrix();
    camera.lookAt(s.tx, s.ty, s.tz);
    camera.updateMatrixWorld();
  }

  /** Everything that follows the clock. Nothing kept here matters: it is all from time. */
  function advance(t: number): void {
    if (!tween) return;
    tween.at ??= t;
    const k = Math.min(1, (t - tween.at) / MOVE_MS);
    look(mixShots(tween.from, tween.to, easeInOutCubic(k)));
    if (k === 1) {
      tween = null;
      resting = true;
    }
  }

  /** Point the camera where the level says. */
  function aim(instant: boolean): void {
    if (!tower) return;
    const to = shotFor(level, tower, width / height);
    if (instant || o.reducedMotion || !shot || sameShot(to, shot)) {
      tween = null;
      look(to);
      resting = true;
    } else {
      tween = { from: shot, to, at: null };
    }
    invalidate();
  }

  /** Low, from the camera's left: any higher and the slabs keep the light out of the rooms. */
  function fitSun(t: Tower): void {
    const mid = t.roofY / 2;
    const reach = Math.max(11, t.roofY / 2 + 6);
    const far = reach * 3;
    sun.target.position.set(FLOOR_BOX.x, mid, 0);
    sun.position.set(FLOOR_BOX.x + SUN.x * far, mid + SUN.y * far, SUN.z * far);
    const c = sun.shadow.camera;
    c.left = -reach;
    c.right = reach;
    c.top = reach;
    c.bottom = -reach;
    c.near = 1;
    c.far = far * 2;
    c.updateProjectionMatrix();
  }

  function proxy(x: number, y: number, z: number, w: number, h: number, d: number, data: { personId?: string; floorId?: string }): void {
    const m = new Mesh(proxyBox, proxyMaterial);
    m.position.set(x, y, z);
    m.scale.set(w, h, d);
    m.layers.set(PICK_LAYER);
    m.userData = data;
    proxies.add(m);
  }

  function placeProxies(m: OfficeModel, t: Tower): void {
    proxies.clear();
    for (const spot of t.floors) {
      proxy(FLOOR_BOX.x, spot.y + FLOOR_BOX.h / 2, 0, FLOOR_BOX.w, FLOOR_BOX.h, FLOOR_BOX.d, { floorId: spot.id });
      for (const d of spot.desks) {
        const c = chairSpot(d);
        proxy(c.x, spot.y + PERSON_BOX.h / 2, c.z, PERSON_BOX.w, PERSON_BOX.h, PERSON_BOX.d, { personId: d.personId, floorId: spot.id });
      }
    }
    if (m.founder) {
      const c = chairSpot(FOUNDER_DESK);
      proxy(c.x, t.roofY + PERSON_BOX.h / 2, c.z, PERSON_BOX.w, PERSON_BOX.h, PERSON_BOX.d, { personId: m.founder.id });
    }
    proxies.updateMatrixWorld(true);
  }

  function drop(group: Group): void {
    scene.remove(group);
    disposeBuilt(group);
  }

  function setModel(m: OfficeModel): void {
    const key = sceneKey(m);
    if (disposed || key === lastKey) return;
    lastKey = key;
    model = m;
    const t = towerLayout(m);
    tower = t;
    let rebuilt = false;

    for (const [id, built] of floors) {
      if (m.floors.some((f) => f.id === id)) continue;
      drop(built.group);
      floors.delete(id);
      rebuilt = true;
    }
    m.floors.forEach((f, i) => {
      const spot = t.floors[i];
      const k = floorKey(f, spot);
      const built = floors.get(f.id);
      if (built?.key === k) return;
      if (built) drop(built.group);
      const group = buildFloor(f, spot, kit);
      scene.add(group);
      floors.set(f.id, { key: k, group });
      rebuilt = true;
    });
    const topKey = JSON.stringify([t.roofY, t.floors.map((f) => f.y), m.founder?.state]);
    if (top?.key !== topKey) {
      if (top) drop(top.group);
      top = { key: topKey, group: buildTop(m, t, kit) };
      scene.add(top.group);
      rebuilt = true;
    }

    placeProxies(m, t);
    kit.groundLight.visible = m.floors.some((f) => f.lit);
    anyoneWorking = [...(m.founder ? [m.founder] : []), ...m.floors.flatMap((f) => f.people)].some((p) => p.state === "working");
    if (rebuilt) {
      fitSun(t);
      renderer.shadowMap.needsUpdate = true;
    }
    const to = shotFor(level, t, width / height);
    if (!shot || !sameShot(to, tween?.to ?? shot)) aim(!shot);
    else invalidate();
  }

  function setLevel(l: Level, instant: boolean): void {
    level = l;
    aim(instant);
  }

  function resize(w: number, h: number): void {
    if (disposed || w <= 0 || h <= 0) return;
    width = w;
    height = h;
    renderer.setSize(w, h, false);
    camera.aspect = w / h;
    camera.updateProjectionMatrix();
    if (tween && tower && shot) tween = retargeted(tween, shot, shotFor(level, tower, w / h));
    else if (tower) {
      look(shotFor(level, tower, w / h));
      resting = true;
    }
    invalidate();
  }

  function setVisible(on: boolean): void {
    visible = on;
    if (on) invalidate();
    else halt();
  }

  function pick(x: number, y: number): { personId?: string; floorId?: string } | null {
    ray.setFromCamera(new Vector2((x / width) * 2 - 1, 1 - (y / height) * 2), camera);
    return pickFrom(ray.intersectObjects(proxies.children, false).map((h) => ({ ...(h.object.userData as Omit<PickHit, "distance">), distance: h.distance })));
  }

  function anchors(): Anchor[] {
    if (!tower || !model) return [];
    const at = (id: string, kind: Anchor["kind"], x: number, y: number, z: number): Anchor => ({ id, kind, ...cssPoint(new Vector3(x, y, z).project(camera), width, height) });
    const out = tower.floors.flatMap((spot) => [
      at(spot.id, "floor", FLOOR_EDGE.x, spot.y + TAG_Y, FLOOR_EDGE.z),
      ...spot.desks.map((d) => {
        const c = chairSpot(d);
        return at(d.personId, "person", c.x, spot.y + HEAD_Y, c.z);
      }),
    ]);
    if (model.founder) {
      const c = chairSpot(FOUNDER_DESK);
      out.push(at(model.founder.id, "person", c.x, tower.roofY + HEAD_Y, c.z));
    }
    out.push(at("top", "top", FLOOR_EDGE.x, tower.roofY + TAG_Y, FLOOR_EDGE.z));
    return out;
  }

  function stats(): SceneStats {
    const size = renderer.getDrawingBufferSize(new Vector2());
    const { calls, triangles } = renderer.info.render;
    return { calls, triangles, frames, lost: lostCount, restored, lastRestoreMs, drawW: size.x, drawH: size.y };
  }

  function onContextLost(e: Event): void {
    e.preventDefault();
    lost = true;
    lostAt = performance.now();
    lostCount += 1;
    halt();
    o.onLost();
  }

  function onContextRestored(): void {
    lost = false;
    renderer.shadowMap.needsUpdate = true;
    renderer.setSize(width, height, false);
    // Hidden, the picture waits: setVisible asks for it.
    if (visible) draw();
    restored += 1;
    lastRestoreMs = performance.now() - lostAt;
    o.onRestored();
    schedule();
  }

  canvas.addEventListener("webglcontextlost", onContextLost);
  canvas.addEventListener("webglcontextrestored", onContextRestored);

  return {
    setModel, setLevel, resize, setVisible, pick, anchors, stats,
    onMoved(fn) {
      moved.add(fn);
      return () => { moved.delete(fn); };
    },
    dispose() {
      if (disposed) return;
      disposed = true;
      halt();
      moved.clear();
      canvas.removeEventListener("webglcontextlost", onContextLost);
      canvas.removeEventListener("webglcontextrestored", onContextRestored);
      for (const { group } of floors.values()) disposeBuilt(group);
      if (top) disposeBuilt(top.group);
      floors.clear();
      proxyBox.dispose();
      proxyMaterial.dispose();
      kit.dispose();
      sun.shadow.dispose();
      renderer.dispose();
      // dispose leaves the context alive, and Chrome drops the oldest past about 16.
      renderer.forceContextLoss();
    },
  };
}
