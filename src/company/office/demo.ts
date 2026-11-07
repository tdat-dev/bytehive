/* The dev page for looking at the office: fixtures in, no app around it, so nothing
 * here can reach a real company. It never ships: vite builds index.html only. */
import { demoSource, levelFrom, type DemoOptions } from "./fixtures";
import { FLIGHT_MS } from "./layout";
import type { DeskTab } from "./nav";
import type { OfficeScene, SceneOptions, SceneStats } from "./scene";
import { createJobBox, createRail } from "./rail";
import { createShell, type ShellHost } from "./shell";

declare global {
  interface Window {
    __office?: { scene: OfficeScene; stats: () => SceneStats; shell: ReturnType<typeof createShell>; boardOpened: string | null };
  }
}

const query = new URLSearchParams(location.search);
const flag = (name: string) => query.get(name) === "1";
const floors = Number(query.get("floors"));
const crowd = Number(query.get("crowd"));
const options: DemoOptions = { empty: flag("empty"), quiet: flag("quiet"), flight: flag("flight"), ...(floors > 0 ? { floors } : {}), ...(crowd > 0 ? { crowd } : {}) };

// The icons and the confirm dialog live in the app's page, so the demo borrows them.
void fetch("/index.html").then((r) => r.text()).then((page) => {
  const app = new DOMParser().parseFromString(page, "text/html");
  const sprite = [...app.querySelectorAll("svg")].find((x) => x.querySelector("symbol"));
  for (const part of [sprite, app.getElementById("confirmModal")]) if (part) document.body.append(document.importNode(part, true));
});

const source = demoSource(options);
const stage = document.getElementById("demoStage")!;
const drawer = document.getElementById("demoDrawer")!;
const message = document.getElementById("demoMessage")!;
let scene: OfficeScene | null = null;
let boardOpened: string | null = null;

// `still` is reduced motion, so a plane waits at its destination. `air=0.5` holds one halfway
// instead, for a picture of it. Both stop the clock, or the plane would land before the shot.
const air = Number(query.get("air"));
const leftAt = source.snapshot(Date.now()).model?.flights[0]?.at ?? Date.now();
const stopped = air > 0 || flag("still") ? leftAt + (air || 0.5) * FLIGHT_MS : null;

/** The options object stays the shell's: it writes reduced motion there when the setting changes. */
const demoOptions = (o: SceneOptions): SceneOptions => {
  if (flag("still") && !air) o.reducedMotion = true;
  o.allowSoftware = flag("software");
  const quality = query.get("quality");
  if (quality === "low" || quality === "high" || quality === "auto") o.quality = quality;
  if (stopped !== null) o.clock = () => stopped;
  return o;
};

const person = (id: string) => {
  const m = source.snapshot(Date.now()).model;
  return m?.founder?.id === id ? m.founder : m?.floors.flatMap((f) => f.people).find((p) => p.id === id);
};

/** A stand-in for the real pane, with the real stage's size. It takes focus as focusPane does. */
function fakeTerminal(id: string): void {
  const p = person(id);
  stage.innerHTML = `<pre class="demo-term" tabindex="0"></pre>`;
  const term = stage.firstElementChild as HTMLElement;
  term.textContent = [`${p?.name} (${p?.cli})`, p?.doing ? `Card: ${p.doing}` : "No card right now.", "> npm test", "142 passed", "> card_done checkout-errors", ""].join("\n");
  requestAnimationFrame(() => term.focus());
}

const host: ShellHost = {
  showDesk(id: string, tab: DeskTab) {
    if (!person(id)?.paneId) {
      stage.textContent = "";
      drawer.hidden = true;
      return false;
    }
    fakeTerminal(id);
    drawer.hidden = tab !== "changes";
    return true;
  },
  levelChanged(l) {
    if (l.kind !== "desk") drawer.hidden = true;
  },
  leave() {
    shell.hide();
    message.hidden = false;
  },
  failed(why) {
    message.hidden = false;
    message.textContent = `${why} Add ?software=1 to allow a software renderer.`;
    shell.hide();
  },
  changesOpenFor: () => (drawer.hidden ? null : "demo"),
  openBoard(id) {
    boardOpened = id;
    if (window.__office) window.__office.boardOpened = id;
  },
  contextLost: () => console.warn("office: context lost"),
  contextRestored: () => console.info("office: context restored"),
  async loadScene() {
    const real = await import("./scene");
    return {
      ...real,
      createScene(canvas: HTMLCanvasElement, o: SceneOptions) {
        scene = real.createScene(canvas, demoOptions(o));
        window.__office = { scene, stats: () => scene!.stats(), shell, boardOpened };
        return scene;
      },
    };
  },
};

const shell = createShell(source, host);
document.body.append(shell.el);
if (flag("side")) {
  // As office.ts does it with no 3D: the rail and the job box as a column beside the app.
  const side = document.createElement("aside");
  side.className = "of-side";
  side.innerHTML = `<div data-job-box></div><div data-rail></div>`;
  document.getElementById("app")!.append(side);
  document.body.classList.add("office-fallback");
  createJobBox(source).mount(side.querySelector<HTMLElement>("[data-job-box]")!);
  createRail(source, () => {}).mount(side.querySelector<HTMLElement>("[data-rail]")!);
} else shell.show(levelFrom(query.get("level"), source.snapshot(Date.now()).model!));

drawer.querySelector("button")!.addEventListener("click", () => { drawer.hidden = true; });
message.addEventListener("click", () => {
  message.hidden = true;
  shell.show();
});

// As the app will do it: on document, in the capture phase, so a terminal never sees it first.
document.addEventListener("keydown", (e) => {
  if (e.key !== "Escape") return;
  const at = document.activeElement as HTMLElement | null;
  const went = shell.escape({ terminal: !!at?.closest(".demo-term"), dialog: false, dock: false, typing: !!at?.closest("input, textarea, select") });
  if (went) {
    e.preventDefault();
    e.stopPropagation();
  }
}, true);
