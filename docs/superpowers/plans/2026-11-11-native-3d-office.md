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
- **P2 pure ports with their tests:** `layout.ts`, `model.ts`, `nav.ts`,
  `palette.ts`, `quality.ts`, file by file, tests ported first.
- **P3 the building:** `build.ts`, `props.ts`, `setting.ts`, `light.ts`,
  `geometry.ts`.
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
