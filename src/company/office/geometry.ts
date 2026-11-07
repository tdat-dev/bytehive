/* What the building and the people are both made with: geometry that keeps its colour
 * on its vertices, so everything of one material can merge into one draw call. */
import { BoxGeometry, BufferAttribute, Color, ExtrudeGeometry, Matrix4, PlaneGeometry, Shape, type BufferGeometry } from "three";
import { RoundedBoxGeometry } from "three/examples/jsm/geometries/RoundedBoxGeometry.js";
import { mergeGeometries, toCreasedNormals } from "three/examples/jsm/utils/BufferGeometryUtils.js";
import { VIEW } from "./layout";

export const DESK_TOP = 0.78;

/** Keeps position and normal, and puts the colour on the vertices. `top` paints what faces up. */
export function tint(g: BufferGeometry, hex: string, top?: string): BufferGeometry {
  const flat = g.index ? g.toNonIndexed() : g;
  flat.deleteAttribute("uv");
  const normal = flat.getAttribute("normal");
  const body = new Color(hex);
  const up = new Color(top ?? hex);
  const colors = new Float32Array(normal.count * 3);
  for (let i = 0; i < normal.count; i += 1) (normal.getY(i) > 0.9 ? up : body).toArray(colors, i * 3);
  flat.setAttribute("color", new BufferAttribute(colors, 3));
  return flat;
}

export const moved = (g: BufferGeometry, x: number, y: number, z: number, turn = 0): BufferGeometry =>
  g.clone().applyMatrix4(new Matrix4().makeRotationY(turn).setPosition(x, y, z));
export const merged = (parts: BufferGeometry[]): BufferGeometry => mergeGeometries(parts, false);
export const box = (w: number, h: number, d: number, hex: string) => tint(new BoxGeometry(w, h, d), hex);
export const soft = (w: number, h: number, d: number, r: number, hex: string) => tint(new RoundedBoxGeometry(w, h, d, 2, r), hex);

/** Colours each vertex from where it is and which way it faces. */
export function paint(g: BufferGeometry, at: (x: number, y: number, z: number, nx: number, ny: number, nz: number) => Color): BufferGeometry {
  const pos = g.getAttribute("position");
  const normal = g.getAttribute("normal");
  const colour = g.getAttribute("color");
  for (let i = 0; i < pos.count; i += 1) {
    const c = at(pos.getX(i), pos.getY(i), pos.getZ(i), normal.getX(i), normal.getY(i), normal.getZ(i));
    colour.setXYZ(i, c.r, c.g, c.b);
  }
  return g;
}

/** From `low` at the foot of a shape to `high` at its top. */
export function ramp(g: BufferGeometry, low: string, high: string): BufferGeometry {
  g.computeBoundingBox();
  const { min, max } = g.boundingBox!;
  const [a, b] = [new Color(low), new Color(high)];
  return paint(g, (_x, y) => a.clone().lerp(b, (y - min.y) / (max.y - min.y || 1)));
}

const VIEW_LEN = Math.hypot(VIEW.x, VIEW.y, VIEW.z);
const VIEW_YAW = Math.atan2(VIEW.x, VIEW.z);
const VIEW_PITCH = Math.asin(VIEW.y / VIEW_LEN);

/** What a light throws: a soft picture on a plane, so the colour goes on the vertices and the uv stays. */
function lightPlane(g: BufferGeometry, hex: string, level: number): BufferGeometry {
  const c = new Color(hex).multiplyScalar(level);
  const count = g.getAttribute("position").count;
  const colours = new Float32Array(count * 3);
  for (let i = 0; i < count; i += 1) c.toArray(colours, i * 3);
  g.setAttribute("color", new BufferAttribute(colours, 3));
  return g;
}

/** The glow round a light. The view never turns, so it can stand facing the viewer for good. */
export const flare = (x: number, y: number, z: number, size: number, hex: string, level: number): BufferGeometry =>
  lightPlane(new PlaneGeometry(size, size).rotateX(-VIEW_PITCH).rotateY(VIEW_YAW).translate(x, y, z), hex, level);

/** A pool of light lying on something flat. */
export const pool = (x: number, y: number, z: number, w: number, d: number, hex: string, level: number): BufferGeometry =>
  lightPlane(new PlaneGeometry(w, d).rotateX(-Math.PI / 2).translate(x, y, z), hex, level);

/** The same, on a wall facing +z or, turned, +x. */
export const wash = (x: number, y: number, z: number, w: number, h: number, turn: number, hex: string, level: number): BufferGeometry =>
  lightPlane(new PlaneGeometry(w, h).rotateY(turn).translate(x, y, z), hex, level);

/** A small repeatable random number, so a seed always gives the same room. */
export function seeded(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

export const hashOf = (s: string): number => [...s].reduce((n, c) => (Math.imul(n, 31) + c.charCodeAt(0)) >>> 0, 7);

const BEVEL = 0.14;

/** Smooth round the edge, while the top stays one flat face. */
function flatTopped(g: BufferGeometry): BufferGeometry {
  const s = toCreasedNormals(g, 0.9);
  const pos = g.getAttribute("position");
  const normal = s.getAttribute("normal");
  for (let i = 0; i < pos.count; i += 3) {
    const flat = Math.abs(pos.getY(i) - pos.getY(i + 1)) < 1e-5 && Math.abs(pos.getY(i) - pos.getY(i + 2)) < 1e-5;
    if (flat) for (let k = 0; k < 3; k += 1) normal.setXYZ(i + k, 0, pos.getY(i) < -1e-5 ? -1 : 1, 0);
  }
  return s;
}

/** A plate with rounded corners and a soft edge, its top at y = 0. Most of the toy look. */
export function roundedSlab(w: number, d: number, h: number, r: number, bevel = BEVEL): BufferGeometry {
  const hw = w / 2 - bevel;
  const hd = d / 2 - bevel;
  const q = Math.max(0.01, Math.min(r - bevel, hw, hd));
  const s = new Shape();
  s.absarc(hw - q, hd - q, q, 0, Math.PI / 2, false);
  s.absarc(-hw + q, hd - q, q, Math.PI / 2, Math.PI, false);
  s.absarc(-hw + q, -hd + q, q, Math.PI, Math.PI * 1.5, false);
  s.absarc(hw - q, -hd + q, q, Math.PI * 1.5, Math.PI * 2, false);
  const depth = Math.max(0.01, h - bevel * 2);
  const g = new ExtrudeGeometry(s, { depth, bevelEnabled: true, bevelSize: bevel, bevelThickness: bevel, bevelSegments: 3, curveSegments: 8 });
  g.rotateX(-Math.PI / 2);
  g.translate(0, -(depth + bevel), 0);
  return flatTopped(g);
}
