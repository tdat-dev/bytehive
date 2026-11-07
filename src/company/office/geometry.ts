/* What the building and the people are both made with: geometry that keeps its colour
 * on its vertices, so everything of one material can merge into one draw call. */
import { BoxGeometry, BufferAttribute, Color, Matrix4, type BufferGeometry } from "three";
import { RoundedBoxGeometry } from "three/examples/jsm/geometries/RoundedBoxGeometry.js";
import { mergeGeometries } from "three/examples/jsm/utils/BufferGeometryUtils.js";

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
