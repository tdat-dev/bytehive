/* The office in three.js. It draws the model and holds nothing else that matters: drop
 * it and call setModel again and the picture is the same. It reads nothing from the
 * page but the canvas it is given, so it can move to a Worker with an OffscreenCanvas. */
import {
  ACESFilmicToneMapping, BoxGeometry, Color, DirectionalLight, Group, HemisphereLight, Mesh, MeshBasicMaterial, PCFSoftShadowMap, PerspectiveCamera,
  Raycaster, Scene, SRGBColorSpace, Vector2, Vector3, WebGLRenderer, type Object3D,
} from "three";
import { BOARD_EDGE, FLOOR_BOX, FLOOR_EDGE, buildFloor, buildTop, createKit, disposeBuilt } from "./build";
import { FLIGHT_MS, FOUNDER_DESK, FOV, VIEW, liftAt, mixShots, shotFor, towerLayout, type FloorSpot, type Seat, type Shot, type Tower } from "./layout";
import { sceneKey, type OfficeFloor, type OfficeModel, type OfficePerson, type OfficeState } from "./model";
import { risingAt, risingGeometry, risingMaterial, risingMesh } from "./life";
import { BUILDING, frameDelay, type Level } from "./nav";
import { OFFICE } from "./palette";
import { HOP_MS, MARK_UNITS, animate, hopAt, isPerson, planeAt, spotOf, type PlaneSpot } from "./people";
import { lookOf, readQuality, startTier, steppedDown, watchBattery, type OfficeQuality } from "./quality";

export interface SceneOptions {
  reducedMotion: boolean; // read at every use: the owner can change it while the scene runs
  dpr: number;
  onLost(): void;       // context lost
  onRestored(): void;   // context back, picture drawn unless the scene is hidden
  clock?(): number;     // what `Flight.at` is measured against. Date.now, unless a demo holds time still
  quality?: OfficeQuality; // the setting, unless the owner's stored one
}
/** CSS pixels in the canvas. `clear`: a bubble hangs this far below the point, and no tag should cover it. */
export interface Anchor { id: string; kind: "floor" | "person" | "top" | "board"; x: number; y: number; visible: boolean; clear?: number }
export interface SceneStats { calls: number; triangles: number; frames: number; lost: number; restored: number; lastRestoreMs: number | null; drawW: number; drawH: number; tier: number; geometries: number; textures: number; asleep: boolean }
export interface OfficeScene {
  setModel(m: OfficeModel): void;
  setLevel(l: Level, instant: boolean): void;
  resize(w: number, h: number): void;
  setVisible(on: boolean): void;
  setQuality(q: OfficeQuality): void;
  /** The person under the pointer rises a little. */
  hover(personId: string | null): void;
  /** Where the pointer is on the canvas, -1 to 1: sky and town shift behind the tower. */
  parallax(x: number, y: number): void;
  pick(x: number, y: number): { personId?: string; floorId?: string } | null;
  anchors(): Anchor[];
  onMoved(fn: () => void): () => void;  // the camera came to rest: labels may be placed. Gives back the way to stop hearing it
  stats(): SceneStats;
  dispose(): void;
}

/** A level change: the app's --m-level, on its entrance curve. */
const MOVE_MS = 333;
/** How long the scene keeps its GPU memory once nobody can see it. */
const AWAY_MS = 120_000;
const HOVER_RISE = 0.14;
/** How far sky and town shift at the edge of the canvas, in scene units. */
const PARALLAX = { x: 1.6, y: 0.7 };
const PICK_LAYER = 1;
const SHADOW_MAP = 1024;
const PERSON_BOX = { w: 0.9, d: 0.9 };
/** A pick box reaches this far over the head. */
const HEAD_ROOM = 0.5;
const BUBBLE_BOX = { w: 1.1, h: 1 };
/** How tall a mark over someone who needs you stands on screen. */
const MARK_PX = 36;
const HAZE_FROM = 75;
const HAZE_OVER = 60;
const TAG_Y = 1.5;
const SUN = new Vector3(0.25, 0.45, 1).normalize();
const VIEWER = new Vector3(VIEW.x, VIEW.y, VIEW.z).normalize();

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

/** cubic-bezier(0, 0, 0, 1). Its x is s cubed, so it needs no solving. */
export const easeEnter = (k: number): number => {
  const s = Math.cbrt(k);
  return s * s * (3 - 2 * s);
};

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
const floorKey = (f: OfficeFloor, spot: FloorSpot, marked: ReadonlySet<string>) => JSON.stringify([spot.y, f.lit, f.board, f.papers, f.lead, f.people.map(lookKey), spot.desks, f.people.filter((p) => marked.has(p.id)).map((p) => p.id)]);

/** Everyone a Needs you row points at, and everyone asking something. */
const markedIn = (m: OfficeModel): Set<string> => new Set([
  ...m.needs.flatMap((r) => (r.personId ? [r.personId] : [])),
  ...[...(m.founder ? [m.founder] : []), ...m.floors.flatMap((f) => f.people)].filter((p) => p.state === "needs").map((p) => p.id),
]);

export function createScene(canvas: HTMLCanvasElement, o: SceneOptions): OfficeScene {
  const renderer = new WebGLRenderer({ canvas, antialias: true, powerPreference: "low-power", preserveDrawingBuffer: false });
  let setting: OfficeQuality = o.quality ?? readQuality();
  let battery = false;
  let tier = startTier(setting, battery);
  let seen = lookOf(tier);
  renderer.setPixelRatio(Math.min(o.dpr, seen.maxDpr));
  renderer.outputColorSpace = SRGBColorSpace;
  renderer.toneMapping = ACESFilmicToneMapping;
  renderer.toneMappingExposure = 1.05;
  renderer.shadowMap.enabled = true;
  renderer.shadowMap.type = PCFSoftShadowMap;
  renderer.shadowMap.autoUpdate = false;

  const scene = new Scene();
  scene.background = new Color(OFFICE.skyTop);
  const camera = new PerspectiveCamera(FOV, 1, 1, 900);
  scene.add(new HemisphereLight(0xb9c8f0, 0x4a3a34, 1.1));
  const sun = new DirectionalLight(0xffe2c0, 1.25);
  sun.shadow.mapSize.set(SHADOW_MAP, SHADOW_MAP);
  sun.shadow.bias = -0.0006;
  sun.shadow.normalBias = 0.03;
  sun.shadow.intensity = 0.62;
  sun.castShadow = seen.shadows;
  scene.add(sun, sun.target);

  const clock = o.clock ?? Date.now;
  const kit = createKit(() => invalidate());
  scene.add(kit.ground);
  const floors = new Map<string, { key: string; group: Group }>();
  let top: { key: string; group: Group } | null = null;

  let cast: Object3D[] = [];
  const planes = new Group();
  scene.add(planes);
  const risingLook = risingMaterial();
  let rising: ReturnType<typeof risingMesh> | null = null;
  let risingKey = "";
  let liftCar: Object3D | null = null;
  let hovered: string | null = null;
  const hops = new Map<string, number>();
  let shownStates = new Map<string, OfficeState>();
  let gaps: number[] = [];
  let lastFrameAt = 0;

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
  let marked: ReadonlySet<string> = new Set();

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
  let awayTimer: ReturnType<typeof setTimeout> | null = null;
  let asleep = false;
  const moved = new Set<() => void>();

  const floorOf = (id: string): string | null | undefined => {
    if (model?.founder?.id === id) return null;
    return model?.floors.find((f) => f.people.some((p) => p.id === id))?.id;
  };

  const flying = (now: number): PlaneSpot[] =>
    (tower && model ? model.flights.flatMap((f) => planeAt(tower!, f, floorOf, now, o.reducedMotion) ?? []) : []);

  const hopping = () => {
    const now = clock();
    for (const [id, at] of hops) if (now - at >= HOP_MS) hops.delete(id);
    return hops.size > 0;
  };

  /** A plane on screen counts too: it takes one more frame to clear it after it lands. */
  const moving = () => tween !== null || hopping() || (!o.reducedMotion && (flying(clock()).length > 0 || planes.children.some((p) => p.visible)));

  /** Reduced motion holds a plane still at its destination: one frame to show it, and one when it lands. */
  const landsIn = (): number | null => {
    if (!o.reducedMotion || !model?.flights.length) return null;
    const left = model.flights.map((f) => f.at + FLIGHT_MS - clock()).filter((ms) => ms > 0);
    return left.length ? Math.min(...left) : planes.children.some((p) => p.visible) ? 0 : null;
  };

  /** Ambient motion runs where it can be seen, and never over a terminal. */
  const ambient = () => anyoneWorking && seen.ambient && level.kind !== "desk";

  /* The whole of the on-demand rule: a frame only when something asks for one. */
  function schedule(): void {
    if (disposed || timer !== null || raf !== null || !visible || lost) return;
    // A level change takes every frame the screen offers: it is short, and timers would round it down to 20.
    const delay = tween ? 0 : frameDelay({ visible, moving: moving(), ambient: ambient(), reduced: o.reducedMotion }) ?? landsIn();
    if (delay === null && !dirty) return;
    const wait = dirty ? 0 : Math.max(0, delay! - (performance.now() - lastDraw));
    if (wait === 0) {
      raf = requestAnimationFrame(frame);
      return;
    }
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
    if (tween) {
      if (lastFrameAt) gaps.push(t - lastFrameAt);
      lastFrameAt = t;
    }
    advance(t);
    draw();
    if (resting) {
      resting = false;
      lastFrameAt = 0;
      judge();
      for (const fn of [...moved]) fn();
    }
    schedule();
  }

  /** A level change just ended: did it hold 30 frames a second? */
  function judge(): void {
    const next = steppedDown(tier, setting, gaps);
    gaps = [];
    if (next !== tier) applyTier(next);
  }

  function applyTier(next: number): void {
    tier = next;
    seen = lookOf(tier);
    const shadowsChanged = sun.castShadow !== seen.shadows;
    sun.castShadow = seen.shadows;
    if (shadowsChanged) {
      renderer.shadowMap.needsUpdate = seen.shadows;
      if (!seen.shadows) sun.shadow.dispose();
    }
    if (kit.deskLight) kit.deskLight.visible = seen.glow;
    if (kit.hazeLight) kit.hazeLight.visible = seen.glow;
    renderer.setPixelRatio(Math.min(o.dpr, seen.maxDpr));
    renderer.setSize(width, height, false);
    invalidate();
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
    sizeMarks();
  }

  /** A mark is the same size on screen at every level, the far ones most of all. */
  function sizeMarks(): void {
    if (!shot) return;
    const dist = Math.hypot(shot.px - shot.tx, shot.py - shot.ty, shot.pz - shot.tz);
    const perPixel = (2 * Math.tan((shot.fov * Math.PI) / 360) * dist) / height;
    for (const who of cast) who.getObjectByName("bubble")?.scale.setScalar((MARK_PX * perPixel) / MARK_UNITS);
    // The glow of a lit room is for the far view: close to, it would only grey the picture.
    if (kit.hazeLight) kit.hazeLight.opacity = Math.min(1, Math.max(0, (dist - HAZE_FROM) / HAZE_OVER));
  }

  /** Everything that follows the clock. Nothing kept here matters: it is all from time. */
  function advance(t: number): void {
    const now = clock();
    const still = o.reducedMotion || !ambient();
    for (const who of cast) {
      const { id } = who.userData.person as { id: string };
      const hop = hops.get(id);
      animate(who, still ? 0 : now, (hop === undefined ? 0 : hopAt(now - hop)) + (id === hovered ? HOVER_RISE : 0));
    }
    if (rising) {
      rising.visible = !still;
      if (!still) risingLook.uniforms.time.value = (now % 3_600_000) / 1000;
    }
    if (liftCar && !still) liftCar.position.y = liftAt(now, liftCar.userData.stops as number[]);
    fly(now);
    if (!tween) return;
    tween.at ??= t;
    const k = Math.min(1, (t - tween.at) / MOVE_MS);
    look(mixShots(tween.from, tween.to, easeEnter(k)));
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
    if (marked.has(data.personId)) proxy(s.x, y + s.bubble + BUBBLE_BOX.h / 2, s.z, BUBBLE_BOX.w, BUBBLE_BOX.h, BUBBLE_BOX.w, { ...data, over: true });
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
    marked = markedIn(m);
    let rebuilt = false;

    for (const [id, built] of floors) {
      if (m.floors.some((f) => f.id === id)) continue;
      drop(built.group);
      floors.delete(id);
      rebuilt = true;
    }
    m.floors.forEach((f, i) => {
      const spot = t.floors[i];
      const k = floorKey(f, spot, marked);
      const built = floors.get(f.id);
      if (built?.key === k) return;
      const group = buildFloor(f, spot, kit, marked, built?.group);
      if (built) drop(built.group);
      scene.add(group);
      floors.set(f.id, { key: k, group });
      rebuilt = true;
    });
    const topKey = JSON.stringify([t.roofY, t.floors.map((f) => f.y), m.founder && lookKey(m.founder), m.founder && marked.has(m.founder.id)]);
    if (top?.key !== topKey) {
      const group = buildTop(m, t, kit, marked, top?.group);
      if (top) drop(top.group);
      top = { key: topKey, group };
      scene.add(group);
      rebuilt = true;
    }

    cast = [...floors.values(), ...(top ? [top] : [])].flatMap(({ group }) => group.children.filter(isPerson));
    liftCar = top?.group.getObjectByName("liftCar") ?? null;
    sizeMarks();
    noteFinishes(m);
    placeRising(m, t);
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

  /** Someone who was working and has stopped, with nothing to ask, has finished: a small jump. */
  function noteFinishes(m: OfficeModel): void {
    const now = new Map([...(m.founder ? [m.founder] : []), ...m.floors.flatMap((f) => f.people)].map((p) => [p.id, p.state]));
    for (const [id, state] of now) {
      if (shownStates.get(id) === "working" && (state === "review" || state === "idle") && !o.reducedMotion) hops.set(id, clock());
    }
    shownStates = now;
  }

  /** Steam and bytes, over the desks where someone works. One mesh, rebuilt only when that changes. */
  function placeRising(m: OfficeModel, t: Tower): void {
    const seats = [
      ...t.floors.flatMap((spot) => spot.desks.filter((d) => stateOf(d.personId) === "working").map((d) => ({ d: d as Seat, y: spot.y, id: d.personId }))),
      ...(m.founder?.state === "working" ? [{ d: FOUNDER_DESK, y: t.roofY, id: m.founder.id }] : []),
    ];
    const key = JSON.stringify(seats);
    if (key === risingKey) return;
    risingKey = key;
    if (rising) {
      scene.remove(rising);
      rising.geometry.dispose();
      rising = null;
    }
    if (!seats.length) return;
    rising = risingMesh(risingGeometry(seats.flatMap((s, i) => risingAt(s.d, s.y, (i + 1) * 0.173))), risingLook);
    scene.add(rising);
  }

  function setLevel(l: Level, instant: boolean): void {
    level = l;
    aim(instant);
  }

  function setQuality(q: OfficeQuality): void {
    setting = q;
    applyTier(startTier(q, battery));
  }

  function hover(id: string | null): void {
    if (id === hovered) return;
    hovered = id;
    if (!tween) { advance(lastDraw); invalidate(); }
  }

  function parallax(x: number, y: number): void {
    if (o.reducedMotion || !kit.far) return;
    const on = level.kind === "building" ? 1 : 0;
    const dx = -x * PARALLAX.x * on;
    const dy = y * PARALLAX.y * on;
    if (Math.abs(dx - kit.far.position.x) < 0.01 && Math.abs(dy - kit.far.position.y) < 0.01) return;
    kit.far.position.x = dx;
    kit.far.position.y = dy;
    invalidate();
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
    if (awayTimer !== null) clearTimeout(awayTimer);
    awayTimer = null;
    if (on) {
      if (asleep) {
        // The picture comes back with the context: see onContextRestored.
        renderer.forceContextRestore();
        return;
      }
      invalidate();
      return;
    }
    halt();
    awayTimer = setTimeout(sleep, AWAY_MS);
  }

  /** Nobody has looked for a while: give the GPU its memory back, and keep what it takes to rebuild. */
  function sleep(): void {
    awayTimer = null;
    if (disposed || lost || visible) return;
    asleep = true;
    renderer.forceContextLoss();
  }

  function pick(x: number, y: number): { personId?: string; floorId?: string } | null {
    ray.setFromCamera(new Vector2((x / width) * 2 - 1, 1 - (y / height) * 2), camera);
    return pickFrom(ray.intersectObjects(proxies.children, false).map((h) => ({ ...(h.object.userData as Omit<PickHit, "distance">), distance: h.distance })));
  }

  /** A name goes over the head, and over the bubble when there is one: that is what says it needs you. */
  const tagHeight = (s: { head: number; bubble: number }, id: string) => (marked.has(id) ? s.bubble + BUBBLE_BOX.h : s.head + HEAD_ROOM);

  function anchors(): Anchor[] {
    if (!tower || !model) return [];
    const at = (id: string, kind: Anchor["kind"], x: number, y: number, z: number): Anchor => ({ id, kind, ...cssPoint(new Vector3(x, y, z).project(camera), width, height) });
    const someone = (id: string, s: { x: number; z: number; head: number; bubble: number }, y: number): Anchor => {
      const a = at(id, "person", s.x, y + tagHeight(s, id), s.z);
      if (!marked.has(id)) return a;
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
    const { geometries, textures } = renderer.info.memory;
    return { calls, triangles, frames, lost: lostCount, restored, lastRestoreMs, drawW: size.x, drawH: size.y, tier, geometries, textures, asleep };
  }

  function onContextLost(e: Event): void {
    e.preventDefault();
    lost = true;
    halt();
    if (asleep) return;
    lostAt = performance.now();
    lostCount += 1;
    o.onLost();
  }

  function onContextRestored(): void {
    lost = false;
    renderer.shadowMap.needsUpdate = seen.shadows;
    renderer.setSize(width, height, false);
    // Hidden, the picture waits: setVisible asks for it.
    if (visible) draw();
    if (asleep) {
      asleep = false;
      schedule();
      return;
    }
    restored += 1;
    lastRestoreMs = performance.now() - lostAt;
    o.onRestored();
    schedule();
  }

  canvas.addEventListener("webglcontextlost", onContextLost);
  canvas.addEventListener("webglcontextrestored", onContextRestored);
  const offBattery = watchBattery((on) => {
    battery = on;
    applyTier(startTier(setting, battery));
  });
  if (kit.deskLight) kit.deskLight.visible = seen.glow;
  if (kit.hazeLight) kit.hazeLight.visible = seen.glow;

  return {
    setModel, setLevel, resize, setVisible, setQuality, hover, parallax, pick, anchors, stats,
    onMoved(fn) {
      moved.add(fn);
      return () => { moved.delete(fn); };
    },
    dispose() {
      if (disposed) return;
      disposed = true;
      halt();
      if (awayTimer !== null) clearTimeout(awayTimer);
      offBattery();
      moved.clear();
      canvas.removeEventListener("webglcontextlost", onContextLost);
      canvas.removeEventListener("webglcontextrestored", onContextRestored);
      for (const { group } of floors.values()) disposeBuilt(group);
      if (top) disposeBuilt(top.group);
      floors.clear();
      cast = [];
      planes.clear();
      rising?.geometry.dispose();
      risingLook.dispose();
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
