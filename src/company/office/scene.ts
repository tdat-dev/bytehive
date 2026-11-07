/* The office in three.js. It draws the model and holds nothing else that matters: drop
 * it and call setModel again and the picture is the same. It reads nothing from the
 * page but the canvas it is given, so it can move to a Worker with an OffscreenCanvas. */
import {
  ACESFilmicToneMapping, BoxGeometry, Color, DirectionalLight, Group, HemisphereLight, Mesh, MeshBasicMaterial, PCFSoftShadowMap, PerspectiveCamera,
  Raycaster, Scene, SRGBColorSpace, Vector2, Vector3, WebGLRenderer, type Object3D,
} from "three";
import { BOARD_EDGE, FLOOR_BOX, FLOOR_EDGE, buildFloor, buildTop, createKit, disposeBuilt } from "./build";
import { FOUNDER_DESK, FOV, mixShots, shotFor, towerLayout, type FloorSpot, type Seat, type Shot, type Tower } from "./layout";
import { sceneKey, type OfficeFloor, type OfficeModel, type OfficePerson, type OfficeState } from "./model";
import { BUILDING, frameDelay, type Level } from "./nav";
import { OFFICE } from "./palette";
import { animate, isPerson, planeAt, pose, spotOf, type PlaneSpot } from "./people";

export interface SceneOptions {
  reducedMotion: boolean; // read at every use: the owner can change it while the scene runs
  dpr: number;
  onLost(): void;       // context lost
  onRestored(): void;   // context back, picture drawn unless the scene is hidden
  clock?(): number;     // what `Flight.at` is measured against. Date.now, unless a demo holds time still
}
/** CSS pixels in the canvas. `clear`: a bubble hangs this far below the point, and no tag should cover it. */
export interface Anchor { id: string; kind: "floor" | "person" | "top" | "board"; x: number; y: number; visible: boolean; clear?: number }
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
const PERSON_BOX = { w: 0.9, d: 0.9 };
/** A pick box reaches this far over the head. */
const HEAD_ROOM = 0.4;
const BUBBLE_BOX = { w: 1.1, h: 1 };
const TAG_Y = 1.5;
const SUN = new Vector3(0.25, 0.45, 1).normalize();
const VIEWER = new Vector3(1, 0.165, 0.84).normalize();

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

export interface PickHit { personId?: string; floorId?: string; over?: boolean; distance: number }

/** What a ray means. Everyone stands inside a floor's box, so the first thing it enters
 *  is the floor it can see, and a person on any other floor is behind a slab. A bubble is
 *  drawn over everything, so it takes the click over everything too. */
export function pickFrom(hits: PickHit[]): { personId?: string; floorId?: string } | null {
  const near = [...hits].sort((a, b) => a.distance - b.distance);
  const over = near.find((h) => h.over);
  if (over) return { personId: over.personId, floorId: over.floorId };
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
const lookKey = (p: OfficePerson) => [p.id, p.state, p.seat, p.rank, p.shirt, p.hair, p.prop, p.cli];
const floorKey = (f: OfficeFloor, spot: FloorSpot) => JSON.stringify([spot.y, f.lit, f.board, f.papers, f.lead, f.people.map(lookKey), spot.desks]);

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

  const clock = o.clock ?? Date.now;
  const kit = createKit(() => invalidate());
  scene.add(kit.ground);
  const floors = new Map<string, { key: string; group: Group }>();
  let top: { key: string; group: Group } | null = null;

  let cast: Object3D[] = [];
  const planes = new Group();
  scene.add(planes);

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

  const floorOf = (id: string): string | null | undefined => {
    if (model?.founder?.id === id) return null;
    return model?.floors.find((f) => f.people.some((p) => p.id === id))?.id;
  };

  const flying = (now: number): PlaneSpot[] =>
    (tower && model ? model.flights.flatMap((f) => planeAt(tower!, f, floorOf, now, o.reducedMotion) ?? []) : []);

  /** A plane on screen counts too: it takes one more frame to clear it after it lands. */
  const moving = () => tween !== null || flying(clock()).length > 0 || planes.children.some((p) => p.visible);

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
    const now = clock();
    for (const who of cast) animate(who, o.reducedMotion ? 0 : now);
    fly(now);
    if (!tween) return;
    tween.at ??= t;
    const k = Math.min(1, (t - tween.at) / MOVE_MS);
    look(mixShots(tween.from, tween.to, easeInOutCubic(k)));
    if (k === 1) {
      tween = null;
      resting = true;
    }
  }

  function fly(now: number): void {
    const spots = flying(now);
    while (planes.children.length < spots.length) {
      const plane = new Mesh(kit.plane, kit.paper);
      // lookAt rolls it to this, so its wings stay open to the viewer, even flying straight up the lift.
      plane.up.copy(VIEWER);
      planes.add(plane);
    }
    planes.children.forEach((plane, i) => {
      const s = spots[i];
      plane.visible = !!s;
      if (!s) return;
      plane.position.set(s.x, s.y, s.z);
      plane.lookAt(s.x + s.dx, s.y + s.dy, s.z + s.dz);
    });
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

  function proxy(x: number, y: number, z: number, w: number, h: number, d: number, data: Omit<PickHit, "distance">): void {
    const m = new Mesh(proxyBox, proxyMaterial);
    m.position.set(x, y, z);
    m.scale.set(w, h, d);
    m.layers.set(PICK_LAYER);
    m.userData = data;
    proxies.add(m);
  }

  function personProxy(d: Seat, y: number, data: { personId: string; floorId?: string }): void {
    const state = stateOf(data.personId);
    const s = spotOf(d, state);
    const h = s.head + HEAD_ROOM;
    proxy(s.x, y + h / 2, s.z, PERSON_BOX.w, h, PERSON_BOX.d, data);
    if (pose(state, 0, 0).bubble) proxy(s.x, y + s.bubble, s.z, BUBBLE_BOX.w, BUBBLE_BOX.h, BUBBLE_BOX.w, { ...data, over: true });
  }

  const stateOf = (id: string): OfficeState => (model?.founder?.id === id ? model.founder : model?.floors.flatMap((f) => f.people).find((p) => p.id === id))?.state ?? "stopped";

  function placeProxies(m: OfficeModel, t: Tower): void {
    proxies.clear();
    for (const spot of t.floors) {
      proxy(FLOOR_BOX.x, spot.y + FLOOR_BOX.h / 2, 0, FLOOR_BOX.w, FLOOR_BOX.h, FLOOR_BOX.d, { floorId: spot.id });
      for (const d of spot.desks) personProxy(d, spot.y, { personId: d.personId, floorId: spot.id });
    }
    // All of the top is the Founder's: there is no floor up there to stop at.
    if (m.founder) proxy(FLOOR_BOX.x, t.roofY + FLOOR_BOX.h / 2, 0, FLOOR_BOX.w, FLOOR_BOX.h, FLOOR_BOX.d, { personId: m.founder.id });
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
    const topKey = JSON.stringify([t.roofY, t.floors.map((f) => f.y), m.founder && lookKey(m.founder)]);
    if (top?.key !== topKey) {
      if (top) drop(top.group);
      top = { key: topKey, group: buildTop(m, t, kit) };
      scene.add(top.group);
      rebuilt = true;
    }

    cast = [...floors.values(), ...(top ? [top] : [])].flatMap(({ group }) => group.children.filter(isPerson));
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

  /** A name goes over the head, and over the bubble when there is one: that is what says it needs you. */
  const tagHeight = (s: { head: number; bubble: number }, state: OfficeState) => (pose(state, 0, 0).bubble ? s.bubble + BUBBLE_BOX.h / 2 : s.head + HEAD_ROOM);

  function anchors(): Anchor[] {
    if (!tower || !model) return [];
    const at = (id: string, kind: Anchor["kind"], x: number, y: number, z: number): Anchor => ({ id, kind, ...cssPoint(new Vector3(x, y, z).project(camera), width, height) });
    const someone = (id: string, s: { x: number; z: number; head: number; bubble: number }, y: number): Anchor => {
      const a = at(id, "person", s.x, y + tagHeight(s, stateOf(id)), s.z);
      if (!pose(stateOf(id), 0, 0).bubble) return a;
      return { ...a, clear: at(id, "person", s.x, y + s.head + HEAD_ROOM, s.z).y - a.y };
    };
    const out = tower.floors.flatMap((spot) => [
      at(spot.id, "floor", FLOOR_EDGE.x, spot.y + TAG_Y, FLOOR_EDGE.z),
      ...(model!.floors.find((f) => f.id === spot.id)?.board ? [at(spot.id, "board", BOARD_EDGE.x, spot.y + BOARD_EDGE.y, BOARD_EDGE.z)] : []),
      ...spot.desks.map((d) => someone(d.personId, spotOf(d, stateOf(d.personId)), spot.y)),
    ]);
    if (model.founder) {
      out.push(someone(model.founder.id, spotOf(FOUNDER_DESK, model.founder.state), tower.roofY));
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
      cast = [];
      planes.clear();
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
