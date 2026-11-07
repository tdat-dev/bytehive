/* At a Floor the rest of the tower falls back: everything outside one band of heights
 * goes dim and cool. It is one number on the graphics card, so it costs no draw call, it
 * moves with the camera, and the sky and the marks over people stay as they are. */
import { Vector3, type Material } from "three";

/** Lowest height, highest height, and how far the rest falls back: 0 is not at all. */
export type FocusBand = { value: Vector3 };

export const focusBand = (): FocusBand => ({ value: new Vector3(-1000, 1000, 0) });

const SOFT = 0.22;

/** Makes a material fall back with the band. Chain it after any other onBeforeCompile. */
export function fallsBack<M extends Material>(m: M, band: FocusBand): M {
  const before = m.onBeforeCompile.bind(m);
  m.onBeforeCompile = (s, r) => {
    before(s, r);
    s.uniforms.focusBand = band;
    s.vertexShader = s.vertexShader
      .replace("#include <common>", "#include <common>\nvarying float vFocusY;")
      .replace("#include <begin_vertex>", "#include <begin_vertex>\nvFocusY = (modelMatrix * vec4(position, 1.0)).y;");
    s.fragmentShader = s.fragmentShader
      .replace("#include <common>", "#include <common>\nvarying float vFocusY;\nuniform vec3 focusBand;")
      .replace("#include <opaque_fragment>", `
        float focusIn = smoothstep(focusBand.x - ${SOFT}, focusBand.x, vFocusY) * (1.0 - smoothstep(focusBand.y, focusBand.y + ${SOFT}, vFocusY));
        outgoingLight *= mix(vec3(1.0), vec3(0.2, 0.23, 0.32), focusBand.z * (1.0 - focusIn));
        #include <opaque_fragment>`);
  };
  if (!m.customProgramCacheKey().startsWith("clay")) m.customProgramCacheKey = () => "focus";
  return m;
}
