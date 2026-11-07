/* The dev page for looking at the office: fixtures in, no app around it, so nothing
 * here can reach a real company. It never ships: vite builds index.html only. */
import { demoSource, levelFrom, type DemoOptions } from "./fixtures";
import { FLIGHT_MS } from "./layout";
import { createScene, webglProbe, type OfficeScene, type SceneStats } from "./scene";

declare global {
  interface Window { __office?: { scene: OfficeScene; stats: () => SceneStats } }
}

const query = new URLSearchParams(location.search);
const flag = (name: string) => query.get(name) === "1";
const host = document.getElementById("demo")!;

const floors = Number(query.get("floors"));
const options: DemoOptions = { empty: flag("empty"), quiet: flag("quiet"), flight: flag("flight"), ...(floors > 0 ? { floors } : {}) };

const probe = webglProbe(flag("software"));
if (!probe.ok) {
  host.textContent = `No usable WebGL here (${probe.renderer || "no context"}). Add ?software=1 to allow a software renderer.`;
} else {
  const canvas = document.createElement("canvas");
  canvas.style.cssText = "display:block;width:100%;height:100%";
  host.append(canvas);

  const source = demoSource(options);
  // `still` is reduced motion, so a plane waits at its destination. `air=0.5` holds one halfway
  // instead, for a picture of it. Both stop the clock, or the plane would land before the shot.
  const air = Number(query.get("air"));
  const leftAt = source.snapshot(Date.now()).model?.flights[0]?.at ?? Date.now();
  const stopped = air > 0 || flag("still") ? leftAt + (air || 0.5) * FLIGHT_MS : null;
  const scene = createScene(canvas, {
    reducedMotion: (flag("still") && !air) || matchMedia("(prefers-reduced-motion: reduce)").matches,
    ...(stopped === null ? {} : { clock: () => stopped }),
    dpr: devicePixelRatio,
    onLost: () => console.warn("office: context lost"),
    onRestored: () => console.info("office: context restored"),
  });

  const fit = () => scene.resize(host.clientWidth, host.clientHeight);
  const draw = () => {
    const { model } = source.snapshot(Date.now());
    if (model) scene.setModel(model);
  };

  fit();
  draw();
  scene.setLevel(levelFrom(query.get("level"), source.snapshot(Date.now()).model!), true);
  source.onChange(draw);
  new ResizeObserver(fit).observe(host);
  window.__office = { scene, stats: () => scene.stats() };
}
