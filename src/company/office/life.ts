/* The small things that rise while people work: steam from a mug and bytes off a screen.
 * One mesh for the whole building, moved on the graphics card by one number, so the
 * clock costs nothing here. Every mark is a quad that faces the camera. */
import {
  AdditiveBlending, BufferAttribute, BufferGeometry, Color, Mesh, ShaderMaterial,
} from "three";
import { DESK_TOP } from "./geometry";
import type { Seat } from "./layout";
import { OFFICE } from "./palette";

export interface Rising { x: number; y: number; z: number; kind: 0 | 1; seed: number }

const CORNERS = [[-1, -1], [1, -1], [1, 1], [-1, -1], [1, 1], [-1, 1]];
const STEAM = { x: -0.52, y: DESK_TOP + 0.1, z: 0.14, count: 3 };
const BYTES = { x: 0, y: DESK_TOP + 0.6, z: -0.2, count: 4 };

const inWorld = (d: Seat, floorY: number, at: { x: number; y: number; z: number }) => ({
  x: d.x + at.x * Math.cos(d.rot) + at.z * Math.sin(d.rot),
  y: floorY + at.y,
  z: d.z - at.x * Math.sin(d.rot) + at.z * Math.cos(d.rot),
});

/** What rises from one working desk. */
export function risingAt(d: Seat, floorY: number, seed: number): Rising[] {
  const out: Rising[] = [];
  for (let i = 0; i < STEAM.count; i += 1) out.push({ ...inWorld(d, floorY, STEAM), kind: 0, seed: (seed * 0.37 + i / STEAM.count) % 1 });
  for (let i = 0; i < BYTES.count; i += 1) out.push({ ...inWorld(d, floorY, { ...BYTES, x: (i - 1.5) * 0.14 }), kind: 1, seed: (seed * 0.61 + (i * 2) / (BYTES.count * 2 + 1)) % 1 });
  return out;
}

export function risingGeometry(marks: Rising[]): BufferGeometry {
  const n = marks.length * CORNERS.length;
  const anchor = new Float32Array(n * 3);
  const corner = new Float32Array(n * 2);
  const about = new Float32Array(n * 2);
  marks.forEach((m, i) => {
    CORNERS.forEach(([cx, cy], k) => {
      const v = i * CORNERS.length + k;
      anchor.set([m.x, m.y, m.z], v * 3);
      corner.set([cx, cy], v * 2);
      about.set([m.kind, m.seed], v * 2);
    });
  });
  const g = new BufferGeometry();
  g.setAttribute("position", new BufferAttribute(anchor, 3));
  g.setAttribute("corner", new BufferAttribute(corner, 2));
  g.setAttribute("about", new BufferAttribute(about, 2));
  return g;
}

const vertex = /* glsl */ `
  attribute vec2 corner;
  attribute vec2 about;
  uniform float time;
  varying vec2 vCorner;
  varying float vKind;
  varying float vFade;
  void main() {
    float steam = 1.0 - about.x;
    float period = mix(2.6, 3.4, steam);
    float k = fract(time / period + about.y);
    float rise = k * mix(0.62, 0.74, steam);
    float sway = sin(k * 6.0 + about.y * 40.0) * 0.05 * steam;
    vec4 at = modelViewMatrix * vec4(position + vec3(sway, rise, 0.0), 1.0);
    at.xy += corner * mix(0.03, 0.07 + k * 0.08, steam);
    vCorner = corner;
    vKind = about.x;
    vFade = sin(k * 3.14159);
    gl_Position = projectionMatrix * at;
  }
`;

const fragment = /* glsl */ `
  uniform vec3 steamColour;
  uniform vec3 byteColour;
  varying vec2 vCorner;
  varying float vKind;
  varying float vFade;
  void main() {
    float round = 1.0 - smoothstep(0.1, 1.0, length(vCorner));
    float alpha = mix(round * 0.42, 0.85, vKind) * vFade;
    gl_FragColor = vec4(mix(steamColour, byteColour, vKind) * alpha, alpha);
  }
`;

export function risingMaterial(): ShaderMaterial {
  return new ShaderMaterial({
    vertexShader: vertex,
    fragmentShader: fragment,
    uniforms: { time: { value: 0 }, steamColour: { value: new Color("#fff3e2") }, byteColour: { value: new Color(OFFICE.screenOn) } },
    transparent: true,
    depthWrite: false,
    blending: AdditiveBlending,
  });
}

export const risingMesh = (g: BufferGeometry, m: ShaderMaterial): Mesh => {
  const out = new Mesh(g, m);
  out.frustumCulled = false;
  out.renderOrder = 2;
  return out;
};
