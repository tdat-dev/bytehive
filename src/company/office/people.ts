/* The people of the office: chunky toys with bead heads, big enough to read from the
 * street. `pose` is everything a state looks like, from the state and the clock alone,
 * so nothing is kept between frames. Each person is a few meshes: what moves on its own
 * is a mesh, and the rest of them is merged, colours on the vertices. */
import {
  BufferAttribute, BufferGeometry, CapsuleGeometry, Color, ConeGeometry, CylinderGeometry, Group, Matrix4, Mesh, PlaneGeometry, SphereGeometry, TorusGeometry,
  type Material, type Object3D,
} from "three";
import type { Kit } from "./build";
import { DESK_TOP, box, merged, moved, tint } from "./geometry";
import { FLIGHT_MS, chairSpot, flightPoint, type Seat, type Tower } from "./layout";
import type { Flight, OfficeFloor, OfficePerson, OfficeState } from "./model";
import { OFFICE } from "./palette";

export interface Pose {
  sit: number;        // 0 standing, 1 seated
  lean: number;       // chair back, radians
  armL: number; armR: number; // arm swing, radians
  screen: "work" | "off" | "dim";
  lamp: boolean;
  bubble: boolean;
  present: boolean;   // false: an empty chair
}

const STILL: Pose = { sit: 1, lean: 0, armL: 0, armR: 0, screen: "dim", lamp: false, bubble: false, present: true };

/** A character's whole picture from its state and the clock. Nothing is kept between frames. */
export function pose(state: OfficeState, t: number, seat: number): Pose {
  switch (state) {
    case "needs": return { ...STILL, sit: 0, lamp: true, bubble: true };
    case "working": {
      const phase = t / 125 + seat * 1.7;
      return { ...STILL, screen: "work", armL: Math.sin(phase) * 0.18, armR: Math.sin(phase + 2) * 0.18 };
    }
    case "review": return { ...STILL, armR: 0.5 };
    case "idle": return { ...STILL, lean: 0.32 };
    default: return { ...STILL, present: false, screen: "off" };
  }
}

/* ---- sizes: everything from the hip, with -z as forward ---- */

const SIT_HIP = 0.6;
const STAND_HIP = 0.7;
const HEAD_UP = 0.92;
const HEAD_R = 0.34;
const SHOULDER = { x: 0.31, y: 0.62 };
const HAND = -0.54;
/** Arms reach for the desk sitting down, and hang on their feet. */
const ARM_SIT = 1.02;
const ARM_STAND = 0.12;
/** Leaning back, the hands come off the desk, this much for each radian. */
const ARM_REST = 2.2;
/** Out of the chair toward the viewer. Looking down on the low floors, the slab above cuts a
 *  back row head off, and the next row hides anyone who steps the other way. At a desk turned
 *  to face the viewer they step round its end: straight ahead is the desk. */
const STEP = { back: { x: 0.72, z: 0.85 }, front: { x: 1.15, z: 0.58 } };
const stepOf = (rot: number) => (Math.cos(rot) > 0 ? STEP.back : STEP.front);
/** The fixed view's bearing, for what turns to face it. */
const VIEW_YAW = Math.atan2(1, 0.84);
/** Facing the viewer, with -z as forward. */
const FACE_VIEWER = VIEW_YAW + Math.PI;
const BUBBLE_GAP = 0.52;
const BUBBLE_SCALE = 1.3;
const INK = "#14161c";
const CARD = "#f6f3ec";
const TROUSERS = "#2b2f3a";
const BADGE = { y: 0.46, z: 0.25 };

const hipAt = (sit: number) => STAND_HIP + (SIT_HIP - STAND_HIP) * sit;
const bubbleAt = (sit: number) => hipAt(sit) + HEAD_UP + HEAD_R + BUBBLE_GAP;

/** Where someone in this state is, for a click to land on them and a label to point at them. */
export function spotOf(d: Seat, state: OfficeState): { x: number; z: number; head: number; bubble: number } {
  const { sit } = pose(state, 0, 0);
  const c = chairSpot(d);
  const step = stepOf(d.rot);
  return { x: c.x + step.x * (1 - sit), z: c.z + step.z * (1 - sit), head: hipAt(sit) + HEAD_UP, bubble: bubbleAt(sit) };
}

/* ---- the body ---- */

function torsoGeometry(p: OfficePerson): BufferGeometry {
  const hair = tint(new SphereGeometry(HEAD_R + 0.015, 24, 12, 0, Math.PI * 2, 0, Math.PI / 2), p.hair).rotateX(0.32);
  const smile = tint(new TorusGeometry(0.07, 0.013, 6, 10, Math.PI), INK).rotateZ(Math.PI);
  return merged([
    moved(tint(new CapsuleGeometry(0.24, 0.34, 6, 14), p.shirt), 0, 0.38, 0),
    moved(tint(new SphereGeometry(HEAD_R, 24, 18), OFFICE.bead), 0, HEAD_UP, 0),
    moved(hair, 0, HEAD_UP, 0),
    moved(tint(new SphereGeometry(0.04, 8, 6), INK), -0.12, HEAD_UP + 0.05, -0.31),
    moved(tint(new SphereGeometry(0.04, 8, 6), INK), 0.12, HEAD_UP + 0.05, -0.31),
    moved(smile, 0, HEAD_UP - 0.05, -0.325),
    moved(box(0.22, 0.24, 0.015, CARD), 0, BADGE.y, -BADGE.z + 0.012),
  ]);
}

const legsGeometry = (): BufferGeometry => {
  const leg = tint(new CapsuleGeometry(0.125, 0.47, 4, 10), TROUSERS);
  return merged([moved(leg, -0.135, -0.345, 0), moved(leg, 0.135, -0.345, 0)]);
};

/** What a prop looks like held out at the desk, set into the arm that swings it. */
const inHand = (g: BufferGeometry) => g.applyMatrix4(new Matrix4().makeRotationX(-ARM_SIT).setPosition(0, HAND, 0));

function clipboard(): BufferGeometry {
  return inHand(merged([
    box(0.2, 0.28, 0.02, OFFICE.deskTop),
    box(0.17, 0.23, 0.03, OFFICE.paper),
    moved(box(0.08, 0.035, 0.04, OFFICE.steel), 0, 0.135, 0),
  ]).rotateX(0.5));
}

function megaphone(): BufferGeometry {
  const cone = tint(new ConeGeometry(0.15, 0.34, 14), OFFICE.paper).rotateX(Math.PI / 2);
  const ring = tint(new TorusGeometry(0.15, 0.025, 6, 16), OFFICE.honey);
  return inHand(merged([moved(cone, 0, 0.04, -0.14), moved(ring, 0, 0.04, -0.31)]));
}

function magnifier(): BufferGeometry {
  return inHand(merged([
    moved(tint(new TorusGeometry(0.11, 0.024, 8, 18), OFFICE.steel), 0, 0.22, 0),
    moved(tint(new CylinderGeometry(0.024, 0.03, 0.16, 8), OFFICE.deskTop), 0, 0.05, 0),
  ]));
}

function writer(body: string, tip: string): BufferGeometry {
  return inHand(merged([
    moved(tint(new CylinderGeometry(0.024, 0.024, 0.28, 8), body), 0, 0.03, 0),
    moved(tint(new ConeGeometry(0.024, 0.07, 8), tip).rotateX(Math.PI), 0, -0.145, 0),
  ]).rotateX(0.35));
}

/** A Map: a prop's name comes from a file on disk. */
const IN_HAND = new Map<string, { hand: "L" | "R"; make: () => BufferGeometry }>([
  ["clipboard", { hand: "L", make: clipboard }],
  ["megaphone", { hand: "R", make: megaphone }],
  ["magnifier", { hand: "R", make: magnifier }],
  ["pen", { hand: "R", make: () => writer(OFFICE.steel, OFFICE.noteDoing) }],
  ["pencil", { hand: "R", make: () => writer(OFFICE.honey, INK) }],
]);

function armGeometry(p: OfficePerson, hand: "L" | "R"): BufferGeometry {
  const parts = [
    moved(tint(new CapsuleGeometry(0.09, 0.34, 4, 10), p.shirt), 0, -0.26, 0),
    moved(tint(new SphereGeometry(0.095, 10, 8), OFFICE.bead), 0, HAND + 0.02, 0),
  ];
  if (hand === "L" && p.rank === "lead") parts.push(moved(tint(new CylinderGeometry(0.1, 0.1, 0.07, 14), OFFICE.honey), 0, -0.17, 0));
  const held = IN_HAND.get(p.prop);
  if (held?.hand === hand) parts.push(held.make());
  return merged(parts);
}

/** A honey pill with a dark mark and a small tail: the one thing over anyone's head. */
function bubbleGeometry(): BufferGeometry {
  const pill = tint(new CapsuleGeometry(0.27, 0.34, 6, 20), OFFICE.honey).rotateZ(Math.PI / 2).scale(1, 1, 0.4);
  const tail = tint(new ConeGeometry(0.13, 0.24, 4), OFFICE.honey).rotateX(Math.PI);
  return merged([
    moved(tail, 0, -0.32, 0),
    pill,
    moved(box(0.085, 0.24, 0.02, INK), 0, 0.06, 0.12),
    moved(tint(new SphereGeometry(0.056, 12, 8), INK), 0, -0.15, 0.12),
  ]);
}

function part(name: string, g: BufferGeometry, m: Material, cast = true): Mesh {
  const out = new Mesh(g, m);
  out.name = name;
  out.castShadow = cast;
  out.userData.owned = true;
  return out;
}

interface Parts { body: Object3D; trunk: Object3D; legs: Object3D; armL: Object3D; armR: Object3D; bubble: Object3D | null }

export type PeopleKit = Pick<Kit, "matte" | "bubble" | "badge">;

/** A person at the origin, looking down -z. `seatPerson` puts them at a desk. */
export function buildPerson(p: OfficePerson, kit: PeopleKit): Object3D {
  const root = new Group();
  const body = new Group();
  const trunk = new Group();
  body.name = "body";
  trunk.name = "trunk";

  const badge = new PlaneGeometry(0.17, 0.17).rotateY(Math.PI).translate(0, BADGE.y, -BADGE.z);
  const armL = part("armL", armGeometry(p, "L"), kit.matte);
  const armR = part("armR", armGeometry(p, "R"), kit.matte);
  armL.position.set(-SHOULDER.x, SHOULDER.y, 0);
  armR.position.set(SHOULDER.x, SHOULDER.y, 0);
  trunk.add(part("torso", torsoGeometry(p), kit.matte), part("badge", badge, kit.badge(p.cli), false), armL, armR);

  const legs = part("legs", legsGeometry(), kit.matte);
  body.add(trunk, legs);
  root.add(body);

  const look = pose(p.state, 0, p.seat);
  let bubble: Mesh | null = null;
  if (look.bubble) {
    bubble = part("bubble", bubbleGeometry(), kit.bubble, false);
    bubble.rotation.y = VIEW_YAW;
    bubble.scale.setScalar(BUBBLE_SCALE);
    bubble.renderOrder = 3;
    root.add(bubble);
  }

  const parts: Parts = { body, trunk, legs, armL, armR, bubble };
  root.userData = { person: { state: p.state, seat: p.seat }, parts, rot: 0 };
  applyPose(root, look);
  return root;
}

/** Puts a person at a desk, on a floor at height `y`. */
export function seatPerson(obj: Object3D, d: Seat, y: number): void {
  const c = chairSpot(d);
  obj.position.set(c.x, y, c.z);
  obj.userData.rot = d.rot;
  const { state, seat } = obj.userData.person as { state: OfficeState; seat: number };
  applyPose(obj, pose(state, 0, seat));
}

export function applyPose(obj: Object3D, p: Pose): void {
  const { parts, rot } = obj.userData as { parts: Parts; rot: number };
  const up = 1 - p.sit;
  const hip = hipAt(p.sit);
  const reach = ARM_STAND + (ARM_SIT - ARM_STAND) * p.sit - p.lean * ARM_REST;
  obj.visible = p.present;
  const step = stepOf(rot);
  parts.body.position.set(step.x * up, hip, step.z * up);
  parts.body.rotation.y = p.sit < 0.5 ? FACE_VIEWER : rot;
  parts.trunk.rotation.x = p.lean;
  parts.legs.rotation.x = (Math.PI / 2) * p.sit;
  parts.armL.rotation.x = reach + p.armL;
  parts.armR.rotation.x = reach + p.armR;
  if (parts.bubble) {
    parts.bubble.visible = p.bubble;
    parts.bubble.position.set(step.x * up, bubbleAt(p.sit), step.z * up);
  }
}

/** The pose this person has now. */
export function animate(obj: Object3D, t: number): void {
  const { state, seat } = obj.userData.person as { state: OfficeState; seat: number };
  applyPose(obj, pose(state, t, seat));
}

export const isPerson = (o: Object3D): boolean => !!o.userData.person;

/* ---- what stands on a desk, in the desk's own place ---- */

/** A prop that is set down rather than held. */
export function deskProp(prop: string): BufferGeometry | null {
  if (prop === "laptop") {
    const lid = box(0.44, 0.3, 0.02, OFFICE.steel).translate(0, 0.15, 0).applyMatrix4(new Matrix4().makeRotationX(-0.26));
    return merged([moved(box(0.44, 0.022, 0.3, OFFICE.steel), 0.4, DESK_TOP + 0.011, 0.12), moved(lid, 0.4, DESK_TOP + 0.02, -0.03)]);
  }
  if (prop === "book") {
    const leaf = (s: number) => moved(box(0.2, 0.04, 0.28, OFFICE.paper).applyMatrix4(new Matrix4().makeRotationZ(s * 0.09)), 0.4 + s * 0.1, DESK_TOP + 0.03, 0.12);
    return merged([leaf(-1), leaf(1)]);
  }
  return null;
}

/** Where the review papers go: the Lead's desk, or the person's own on a floor with no Lead. */
export function paperDesk(f: OfficeFloor): string | null {
  if (!f.papers) return null;
  return f.lead ?? f.people.find((p) => p.state === "review")?.id ?? null;
}

/** In the corner the viewer can see past someone standing. `aside` moves it off a prop that has that corner. */
export function paperStack(n: number, aside = false): BufferGeometry {
  const x = aside ? -0.46 : 0.54;
  return merged(Array.from({ length: n }, (_, i) => moved(box(0.3, 0.024, 0.32, OFFICE.paper), x, DESK_TOP + 0.014 + i * 0.03, -0.02, (i - 1) * 0.09)));
}

/** A small desk lamp: its base and stem are matte, and its shade is alight. */
export function lampGeometry(): { solid: BufferGeometry; lit: BufferGeometry } {
  const at = { x: 0.62, z: -0.29 };
  return {
    solid: merged([
      moved(tint(new CylinderGeometry(0.09, 0.1, 0.03, 12), OFFICE.deskBody), at.x, DESK_TOP + 0.015, at.z),
      moved(tint(new CylinderGeometry(0.02, 0.02, 0.44, 8), OFFICE.deskBody), at.x, DESK_TOP + 0.24, at.z),
    ]),
    lit: moved(tint(new ConeGeometry(0.18, 0.18, 14), OFFICE.lampNeeds), at.x, DESK_TOP + 0.5, at.z),
  };
}

/** Light lying on a desk top: from far off, and from behind a screen, this is what shows
 *  someone is at work. Its picture is a soft round pool, so it keeps its uv. */
export function deskGlow(kind: "work" | "lamp"): BufferGeometry {
  const { w, x, hex, level } = kind === "work" ? { w: 1.44, x: 0, hex: OFFICE.screenOn, level: 2.2 } : { w: 1.0, x: 0.24, hex: OFFICE.lampNeeds, level: 1 };
  const g = new PlaneGeometry(w, 0.76).rotateX(-Math.PI / 2).translate(x, DESK_TOP + 0.006, 0);
  const c = new Color(hex).multiplyScalar(level);
  const count = g.getAttribute("position").count;
  g.setAttribute("color", new BufferAttribute(new Float32Array(Array.from({ length: count }, () => c.toArray()).flat()), 3));
  return g;
}

const SCREEN_LEVEL = { work: 1.1, dim: 0.2 };

/** A screen's colour: lit parts hold their own light, so its level is in the colour. */
export function screenLight(s: Pose["screen"]): Color {
  return s === "off" ? new Color(OFFICE.screenOff) : new Color(OFFICE.screenOn).multiplyScalar(SCREEN_LEVEL[s]);
}

/* ---- the paper plane ---- */

/** Three triangles, nose on +z: two wings and the fold under them. */
export function planeGeometry(): BufferGeometry {
  const nose = [0, 0, 0.6];
  const tailL = [-0.36, 0.06, -0.42];
  const tailR = [0.36, 0.06, -0.42];
  const mid = [0, 0.06, -0.28];
  const keel = [0, -0.14, -0.36];
  const g = new BufferGeometry();
  g.setAttribute("position", new BufferAttribute(new Float32Array([...nose, ...tailL, ...mid, ...nose, ...mid, ...tailR, ...nose, ...mid, ...keel]), 3));
  const paper = new Color(OFFICE.paper);
  const fold = paper.clone().multiplyScalar(0.62);
  const colours = [paper, paper, paper, paper, paper, paper, fold, fold, fold].flatMap((c) => c.toArray());
  g.setAttribute("color", new BufferAttribute(new Float32Array(colours), 3));
  return g;
}

const NOSE = 0.01;

export interface PlaneSpot { x: number; y: number; z: number; dx: number; dy: number; dz: number }

/** Where a flight's plane is `now`, and which way its nose points. Reduced motion holds it
 *  at its destination for the time the flight takes. Nothing flies inside one floor, or to
 *  someone who has gone: `floorOf` gives a floor, null for the roof, undefined for nobody. */
export function planeAt(t: Tower, f: Flight, floorOf: (id: string) => string | null | undefined, now: number, reduced: boolean): PlaneSpot | null {
  const from = floorOf(f.from);
  const to = floorOf(f.to);
  if (from === undefined || to === undefined || from === to) return null;
  const k = (now - f.at) / FLIGHT_MS;
  if (k < 0 || k >= 1) return null;
  const at = reduced ? 1 : k;
  const a = flightPoint(t, from, to, Math.max(0, at - NOSE));
  const b = flightPoint(t, from, to, Math.max(NOSE, at));
  const here = flightPoint(t, from, to, at);
  return { ...here, dx: b.x - a.x, dy: b.y - a.y, dz: b.z - a.z };
}
