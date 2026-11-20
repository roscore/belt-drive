# Task: timing-belt reduction designer for robot joints (Rust → WebAssembly, static web app)

Original open-source tool. Write all code yourself; do not copy anyone's code, tables or assets.
Use textbook methods only (pitch-circle geometry, open-belt length and centre-distance equations,
wrap angle, teeth in mesh, effective tension from torque, centrifugal tension, tight/slack side
tensions with pretension, belt span transverse natural frequency f = (1/2L)·sqrt(T/m'), simple
span stiffness for backlash/compliance estimate). Belt profile values (pitch, mass per metre, allowable
tension per mm width, min pulley teeth) are generic and labelled "typical" — no vendor names, no copied
catalogue tables.

## Product (SMALL scope — core + UI about 800-1200 lines of new Rust; renderer/math are reused)
A single-page web app (GitHub Pages, sub-path `/belt-drive/`): "I want a 1- or 2-stage timing-belt
reduction for my robot joint (e.g. a quasi-direct-drive leg or a wrist) — which pulley teeth, which
belt length and centre distance, how wide must the belt be, how much pretension, how stiff is it,
and how much backlash-free stiffness do I lose versus a gearbox?"

1. **Inputs**: belt profile preset: "GT-style 2 mm (typical)", "GT-style 3 mm (typical)",
   "HTD-style 5 mm (typical)", "T-style 5 mm steel cord (typical)" — each with pitch, mass per metre per
   mm width, allowable working tension per mm width, tensile stiffness per mm width (N per unit strain),
   min recommended teeth; stages 1 or 2; per stage: driver teeth z1, driven teeth z2, target centre
   distance C (mm); belt width b (mm); motor torque (N·m) and max speed (rpm); joint output peak torque
   requirement (N·m); pretension factor (fraction of working tension, default 0.5); service factor
   (default 1.4).
2. **Model (core)**: pitch diameters d = z·p/π; ratio per stage and total; nearest standard belt tooth
   count to the target C (integer teeth), real C from belt length (solve open-belt equation, closed form
   or Newton, unit tested); wrap angle on small pulley; teeth in mesh (warn < 6); effective tension
   Te = 2T/d1; centrifugal tension Tc = m'·v²; tight/slack tensions T1 = Ti + Te/2, T2 = Ti − Te/2
   (warn if T2 ≤ 0 → tooth jump risk); required width from allowable tension × service factor;
   belt speed and max motor rpm limit; span vibration frequency (for setting pretension with a phone
   tuner app) in Hz; torsional stiffness at the output from belt span stiffness k = EA/L_span for both
   spans (N·m/rad), and output angular deflection at peak torque (arcmin); shaft/bearing radial load
   ≈ vector sum of T1, T2. Status per check (OK / warning / fail) and an overall status.
3. **Extras (small)**: (a) "find reductions" search: for a target total ratio and tolerance, search
   z1 ∈ [min..40], z2 ∈ [z1..120] (1 or 2 stages) and widths {6,9,10,12,15,20,25} that pass all checks,
   list top 12 sorted by (passes, |ratio error|, total package length), click a row to load;
   (b) 2D plot: output stiffness and output deflection vs belt width with the current point marked.
4. **3D view (WebGL2, game-grade — see below)**: real 3D drive: pulleys as toothed discs (visible teeth
   via `prism()`), flanges, shafts, motor can on the input shaft, belt drawn as a ribbon of small tooth
   blocks following the tangent lines and arcs (both stages, stage 2 on a parallel plane), simple plate
   frame. Animated: pulleys rotate at correct relative speed and belt teeth travel along the path;
   play/pause, slow motion (default ~0.25×), time slider. Toggle: tension colouring (tight span red-ish,
   slack span blue-ish, intensity by tension), exaggerated span sag/deflection under load, force arrows
   (shaft loads), dimension labels in the HUD.
5. **2D plots** (canvas 2D, own small module): the stiffness/width plot above, plus a tension-vs-output-
   torque plot (T1, T2, with the T2=0 jump limit marked) with the current operating point.
6. Korean + English toggle (browser default). Mobile friendly. CSS prefers-color-scheme only.
Out of scope (do NOT build): belt life/fatigue curves, idlers, serpentine drives, thermal effects, URL
share, JSON/CSV import/export, STL export.

## 3D view requirements (game-like, reuse the existing renderer)
- `crates/bd_web/src/gl.rs` and `crates/bd_web/src/math.rs` already exist: a tool-independent WebGL2
  renderer (lit meshes, checker/grid ground with fog, sky gradient, stencil planar shadows, translucent
  volumes, lines, `prism()` extrusion) and pure math (mat4, damped orbit camera with inertia,
  ray-plane/ray-sphere picking) with unit tests. Reuse them; extend only if needed and keep them
  tool-independent and tested.
- Scene code in `view3d.rs`. Orbit (left drag) / pan (right drag, two fingers) / zoom (wheel, pinch)
  with inertia, double-click reset, `touch-action:none`. View presets: Perspective / Front / Side /
  Pulley close-up (small driver pulley mesh).
- Direct manipulation: dragging the driven pulley along the centre line changes centre distance C
  (snaps to the nearest valid belt length, updates inputs live); dragging the output pulley rim
  rotates the output to show loaded deflection; hover highlight + cursor change.
- Toggle buttons over the view: tension colouring, deflection exaggeration, force arrows, play/pause.
- HUD over the canvas: total ratio, belt length (teeth / mm), real C, teeth in mesh, T1/T2 N, width
  needed vs chosen, span frequency Hz, output stiffness N·m/rad, deflection arcmin, shaft load N,
  overall status.
- If WebGL2 is unavailable, show a message and keep the numeric readouts and 2D plots working.
- Release wasm ≲ 350 KB. No crates besides wasm-bindgen, web-sys, js-sys.

## Core crate (pure Rust, no web deps, unit tested) — `crates/bd_core`
- Tests (hand-checkable, 6-10): pitch diameter of 20T 2 mm ≈ 12.732 mm; open-belt length for equal
  pulleys equals 2C + π·d; centre-distance solver inverts the length equation (round-trip < 1e-6 mm);
  wrap angle 180° for equal pulleys and < 180° on the small pulley otherwise; Te = 2T/d; T2 ≤ 0 flagged
  when pretension is too low; span frequency scales with sqrt(T) and 1/L; stiffness doubles when width
  doubles; search results all pass every check and are sorted.
- `crates/bd_web`: UI, plain web-sys. The existing math.rs/gl.rs tests must keep passing.

## Repo hygiene
- README (Korean first, then English): what it is, model and assumptions (inextensible geometry, linear
  elastic spans, no tooth deformation/backlash model, generic typical values), controls, local build
  (`cargo test`, `trunk build --release`), MIT license, "educational estimates — validate with the
  belt maker's data and real tests before trusting a robot with it".
- LICENSE: MIT, copyright "belt-drive contributors" (exists). `.gitignore` for target/ dist/ (exists).
- `index.html` with a CSP meta that blocks external requests:
  `default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; connect-src 'self'; object-src 'none'; base-uri 'self'`.
  `Trunk.toml` exists (`filehash = false`, `public_url = "/belt-drive/"`, external `web/boot.js`
  copied via `<link data-trunk rel="copy-file" href="web/boot.js">`, which imports `./bd_web.js`).
- Root `Cargo.toml` workspace with members `crates/bd_core`, `crates/bd_web` (exists).
- `.github/workflows/pages.yml` exists (trunk 0.22.1, privacy check, cargo test, deploy-pages). Keep it.
- `tools/privacy_check.sh` already exists; keep it and keep it clean.
- Polished app UI: dark design tokens (CSS variables, system fonts), full-viewport 3D canvas as hero,
  slim floating toolbar with tooltips, collapsible right-side settings cards (bottom sheet on mobile),
  example loaded on first run with a dismissible hint.

## Done when
- `cargo test` green (workspace); `trunk build --release` succeeds with no warnings;
  `sh tools/privacy_check.sh` clean.
- Commit author `belt-drive contributors <59640857+roscore@users.noreply.github.com>`.
- Commit everything with clear commits; open a PR. Do NOT create new GitHub repos or change repo settings.
