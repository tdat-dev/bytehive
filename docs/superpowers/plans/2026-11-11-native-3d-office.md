# Native 3D office

**Goal:** the Company page in the native app shows the 3D office the Tauri app
has (feat/office-3d, `src/company/office/`), drawn in Rust, with the 2D floors
kept as the fallback. Owner's choice 2026-11-11: real 3D written in Rust, not an
embedded WebView and not pre-rendered 2.5D.

## What stands in the way

- GPUI has no GPU surface on Windows. `Window::paint_surface` and
  `PaintSurface` exist only on macOS; the DirectX renderer's `draw_surfaces` is
  a stub.
- A child HWND with its own swap chain cannot sit in the page: GPUI presents
  through DirectComposition with `CreateTargetForHwnd(hwnd, true)`, a topmost
  target, so its own layer covers any child window.
- So the office draws offscreen with wgpu (DX12) into a BGRA texture, the frame
  is read back, and GPUI shows it as an image (`img(ImageSource::Render)`),
  dropping the previous frame's atlas entry each time.

## What the office is made of (three.js, about 5,000 lines without tests)

Procedural geometry only, no model files: boxes, cylinders, spheres, capsules,
cones, tori, planes, circles, extrusions, merged per material with colours on
the vertices. MeshStandardMaterial with the "clay" changes (own light, rim
light, darkening toward each storey's floor), hemisphere and directional light,
PCF soft shadows, ACES tone mapping in sRGB, additive camera-facing marks,
points for dust and stars, canvas textures for words and logos, raycast picking,
camera flights between floors, people that hop and animate, quality tiers.

## Phases (board card "Native 3D office in Rust")

- **P0 spike, a go or stop gate.** wgpu offscreen into GPUI at the panel's size
  (about 1150 x 710 logical), a few hundred lit, vertex-coloured boxes turning.
  Measure on this machine, one metric each: frames per second shown, CPU ms per
  frame on the GPUI thread, process private working set before and after, GPU
  memory. Go when it holds 30 fps or better with under 8 ms of GPUI-thread time
  per frame and under 60 MB more private memory; stop and report otherwise.
- **P0 result (2026-11-12): go.** Release build, Company page, 1150 x 500 panel,
  BYTEHIVE_OFFICE3D=spike. On the Intel UHD 630 (the card GPUI draws on):
  30 fps, 8.9 ms of GPUI-thread time a frame, +66 MB private, flat over time,
  39% of a core while turning. Three fixes got it there, each measured:
  - Draw on the low-power adapter, GPUI's own (`EnumAdapters(0)`). The RTX 2070
    woke a second driver stack for nothing.
  - `MemoryHints::MemoryUsage`: wgpu's default 256 MB blocks cost +296 MB for
    one frame; with small blocks the device costs +55 MB.
  - Drop each frame's image one frame late. Dropped first, GPUI's atlas
    texture emptied, was destroyed and made again every frame (+270 MB of
    driver memory); kept one frame, two frames share one texture.
  Carried into P1: read frames back asynchronously (double buffer, no wait on
  the GPU in paint) to get under 8 ms; draw only when something moves, so an
  idle office costs nothing; a lower resolution tier for weak cards.
- **P1 engine.** glam math; a mesh builder for every primitive above with
  vertex colours and merging; the clay lighting in WGSL; a directional shadow
  map with PCF; ACES and sRGB; picking by ray against bounds. Tested on the CPU
  side (geometry counts, normals, bounds, picks).
- **P1 progress (2026-11-16).** Done, each matched corner for corner against
  three.js 0.170 goldens (`native/src/office3d/fixtures/prims.json`, made by
  `fixtures/prims.mjs` with node and the real `three.module.js`, fetched by
  `npm pack three@0.170.0` into a scratch folder): `geom.rs` (tint, moved,
  merged, paint, ramp, sRGB hex to linear), `prims.rs` (Box with segments,
  Plane, Circle, Cylinder, Cone, Sphere and its parts, Torus, RoundedBox),
  `lathe.rs` (Lathe, Capsule), async readback (`readback.rs`, 3.3 ms a frame on
  the UI thread). Next: `roundedSlab` (geometry.ts): a rounded rectangle Shape
  of four `absarc`s, `ExtrudeGeometry` with bevel (curveSegments 8,
  bevelSegments 3), `rotateX(-PI/2)`, then `flatTopped` (`toCreasedNormals(g,
  0.9)` and flat normals on level faces). Port addShape's bevel rings from
  ExtrudeGeometry.js (getBevelVec, the ring loops, buildLidFaces,
  buildSideFaces); the lids use earcut, so test the side faces corner for
  corner and the lids by area and bounds (a convex fan covers the same
  surface). Then the clay WGSL, shadow map, picking.
- **roundedSlab done (2026-11-16):** `slab.rs` and `earcut.rs`, floor, wall
  (0.34 thick) and lawn matched corner for corner, normals included. The
  earcut port covers convex outlines only; a slab thinner than two bevels
  (2 x 0.14) folds its outline over itself and would need the full earcut
  (filterPoints, cureLocalIntersections, splitEarcut). The office has none.
  Next in P1: the clay WGSL, the shadow map, picking.
- **Clay light done (2026-11-16):** `office.wgsl` is three r170's
  MeshStandardMaterial path (Lambert / PI, GGX with F Schlick, Smith
  correlated V, GGX D; hemisphere mixed on n.y) with clay's foot darkening,
  room glow and edge light, then three's fitted ACES at exposure 1.05;
  `clay.rs` holds the lights and the three looks. naga validates the WGSL in
  `cargo test`. Next: the sun's shadow (PCFSoftShadowMap: 3 x 3 with lerped
  edges, bias -0.0006, normalBias 0.03, intensity 0.62, the frustum scene.ts
  sets near line 523), then picking.
- **Sun shadow done (2026-11-16):** `shadow.rs` (1024 map, fitSun box, back
  faces in, LessEqual compare sampler) and `shadow_at` in `office.wgsl` (the
  PCFSoft 3 x 3 with blended outer taps, bias, normalBias, intensity). Spike
  on the Intel UHD 630: 116 fps, 0.9 ms a frame on the UI thread. Next in P1:
  picking (ray against each pickable's box, as scene.ts's proxies).
- **P1 done (2026-11-16):** picking in `pick.rs` (a ray from the camera, as
  Raycaster.setFromCamera, against each proxy's box; nearest wins). Next is
  P2: port layout.ts, model.ts, nav.ts, palette.ts and quality.ts with their
  tests, tests first.
- **P2 pure ports with their tests:** `layout.ts`, `model.ts`, `nav.ts`,
  `palette.ts`, `quality.ts`, file by file, tests ported first.
- **P2 done (2026-11-16):** `layout.rs` (+ `layout_tests.rs`, layout.test.ts
  case for case), `nav.rs` (nav_up, clamp_level, frame_delay), `palette.rs`
  (the meshes' colours, accents, beads, WCAG contrast), `quality.rs` (the
  steps). model.ts was already ported by Ana in `inbox/company/model.rs`; the
  layout takes (floor id, person ids in seat order) so it does not depend on
  it. Kept for P6: storing the quality setting, reading the battery, the
  crumbs and rail targets (the Company page has them). Next: P3, the building.
- **P3 the building:** `build.ts`, `props.ts`, `setting.ts`, `light.ts`,
  `geometry.ts`.
- **P3 progress (2026-11-16):** goldens now come from the office's own
  TypeScript, bundled with three by esbuild and run in node
  (`native/src/office3d/fixtures/gold/README.md`). Done and matched:
  `seed.rs` (seeded, hashOf), `light.rs` (lightAt, shadeAt, bake,
  deskBlockers), `glow.rs` (flare, pool, wash), `colour.rs` (dim, shift,
  three's rounding), `props.rs` + `props_wall.rs` (every prop), `furnish.rs`
  (whole rooms: six floors, every geometry as a digest in `furnish.json`,
  plus lamps and blockers; a mutation check fails it). Next: build.ts's
  geometry (shell, desk, chair, wallGrid, hexFloor, screen, deskFor,
  deskThing, noteParts, stringLights, roof, liftCar); its canvas words wait
  for P5's overlays. Then setting.ts.
- **P3 building parts done (2026-11-16):** `build_parts.rs` (shell, desk,
  chair and its back, board, deskThing, stickies, noteParts, stringLights
  with three's Matrix4.lookAt, liftCar) and `surfaces.rs` (wallGrid,
  hexFloor), matched as digests in `build.json`; the gold bundler exports
  build.ts's private parts on the way in and stubs Vite's import.meta.glob.
  Lesson: pass sizes to `rounded_slab` in f64. The wall's 0.34 held as an f32
  left its rounded ends 2e-9 apart and the outline grew two points.
  Left in P3: the roof (roofSteps) and setting.ts. deskFor and roomSteps
  need people.ts, so they move to P4.
- **P4 people and life marks:** `people.ts`, `life.ts`, animation clock.
- **P5 interaction:** camera flights (`mixShots`, `easeLevel`), hover and click
  through picking, anchors projected to the page so GPUI draws the words over
  them instead of canvas textures.
- **P6 into the Company page:** the floors panel shows the office when the GPU
  draws it, the 2D list otherwise; the office sleeps when hidden; perf budget
  checked again; CHANGELOG and a native release.

## Rules

- Files under 400 lines, one feature per file: `native/src/office3d/`.
- wgpu with only `dx12` and `wgsl` (no Vulkan, Metal, GL), to keep the binary
  small.
- The look follows the Tauri office (palette.ts values) inside the Inbox's
  panels; owner judges by eye against the Tauri office.
