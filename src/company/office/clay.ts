/* The one matte surface everything in the office is made of. A standard material with
 * three small additions: it holds some light of its own, so a room reads with shadows
 * off; a soft edge light, so a character parts from the wall behind them; and it darkens
 * toward the floor of each storey, which is free ambient occlusion for every leg. */
import { Color, MeshStandardMaterial } from "three";
import { GROUND, SLAB, STOREY } from "./layout";

export interface ClayLook {
  /** What the surface gives off on its own, in the colour of the room. */
  self: string;
  selfLevel: number;
  /** The edge light. */
  rim: string;
  rimLevel: number;
  /** How dark the foot of a thing gets: 0 is none. */
  foot: number;
}

const f = (n: number) => n.toFixed(4);

export function clay(look: ClayLook): MeshStandardMaterial {
  const m = new MeshStandardMaterial({ vertexColors: true, roughness: 0.82, metalness: 0, dithering: true });
  const uniforms = {
    clayRoom: { value: new Color(look.self).multiplyScalar(look.selfLevel) },
    clayRim: { value: new Color(look.rim).multiplyScalar(look.rimLevel) },
    clayFoot: { value: look.foot },
  };
  m.onBeforeCompile = (s) => {
    Object.assign(s.uniforms, uniforms);
    s.vertexShader = s.vertexShader
      .replace("#include <common>", "#include <common>\nvarying float vClayY;")
      .replace("#include <begin_vertex>", "#include <begin_vertex>\nvClayY = (modelMatrix * vec4(position, 1.0)).y;");
    s.fragmentShader = s.fragmentShader
      .replace("#include <common>", "#include <common>\nvarying float vClayY;\nuniform vec3 clayRoom;\nuniform vec3 clayRim;\nuniform float clayFoot;")
      .replace("#include <emissivemap_fragment>", `#include <emissivemap_fragment>
        float clayUp = mod(vClayY - ${f(GROUND)}, ${f(STOREY)});
        float clayLow = 1.0 - clayFoot * (1.0 - smoothstep(0.0, 0.85, clayUp));
        diffuseColor.rgb *= clayLow;
        float clayEdge = pow(1.0 - saturate(dot(normal, normalize(vViewPosition))), 2.6);
        totalEmissiveRadiance += diffuseColor.rgb * clayRoom * (0.72 + 0.28 * smoothstep(0.0, ${f(STOREY - SLAB)}, clayUp)) + clayRim * clayEdge;`);
  };
  m.customProgramCacheKey = () => "clay";
  return m;
}
