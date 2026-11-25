# Clash of Valor 3D Visual Pass Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the duel easier to read and the hits feel heavier, without slowing the frame.

**Architecture:** All changes stay in the CPU ray marcher and glyph layer, in this order:
- Grass and sky are new branches in the glyph choice in `ascii.rs`.
- Fighter outline tint and hit flash go in `sample_cell`, keyed by the owning body.
- The contact shadow goes in `Scene::shade`.
- Shake and hit-stop are small pure state machines read by `main.rs` and `anim.rs`.
- Panels get box-drawing frames and text fitting in `ui.rs`.

**Tech Stack:** Rust 2024, macroquad 0.4, rayon. Run tests with `cargo test` in `ascii3d/`.

**Spec:** `docs/superpowers/specs/2026-11-25-ascii3d-visuals-design.md`

## Global Constraints

- **No zoom.** Camera distance is fixed. Shake may offset the eye and target by the same small vector, but never changes their distance.
- **No slower frame.** Raymarch ms at the default view must not regress. Baseline: run `--shot` with `--duel warrior,sorcerer` before Task 1 and record the ms. After every task, re-run the same shot and compare.
- **Shots go in the session scratchpad**, not `/tmp`.
- **No changes to rules, flow, or controls.** The one exception is the hit-stop, which pauses an action's clock.
- **Box-drawing glyphs** `┌─┐│└┘` exist in `assets/mono.ttf` (checked). `☾☽` do not exist; draw the moon with `●`, `o` or `O`.

## Review Focus

1. **Hit-stop vs skip:** skipping during or after a hit-stop must still apply the impact exactly once and end the action. (Task 5 test.)
2. **Hit-stop vs knockout:** a knockout during a hit-stop must still collapse the loser and reach the result screen. (Task 5 test through `Flow`.)
3. **Shake vs the no-zoom rule:** `eye - target` keeps the same length during shake. (Task 5 test.)
4. **Sky stars vs camera orbit:** stars are hashed on world direction, so the same direction gives the same glyph at any yaw. (Task 3 test.)
5. **800×500 window:** panel text and captions fit inside the window. (Task 6 tests plus a shot.)

---

### Task 1: Grass recedes

**Files:** `ascii3d/src/ascii.rs` (`grass_glyph`, new pure helpers and tests)

**Interfaces:**
- `pub fn grass_keep(dist: f32) -> f32`: the probability that a grass cell draws a blade, given its distance in metres from the duel centre.
  - 1.0 within 3 m.
  - Falls smoothly to about 0.35 by 12 m.
  - Never below 0.25.
- `pub fn grass_light(dist: f32) -> f32`: a brightness multiplier.
  - About 0.85 within 3 m.
  - Falls to about 0.45 by 12 m.
- The duel centre is the midpoint of `scene.bodies[0].pos` and `scene.bodies[1].pos`, or the origin if there are no bodies.
- In `grass_glyph`, a cell whose hash exceeds `grass_keep` becomes `' '` or `'.'` (hash-chosen). The colour is multiplied by `grass_light`.

- [ ] **Step 1: Baseline shot.** Run `--shot <sp>/v0.png --duel warrior,sorcerer` and record the raymarch ms.
- [ ] **Step 2: Failing test.** `grass_falls_off_with_distance` checks:
  - `grass_keep(0.0) == 1.0` and `grass_keep(2.9) == 1.0`.
  - `grass_keep(12.0) < 0.45`.
  - `grass_keep` is monotonically non-increasing over 0..30 in steps of 0.5.
  - `grass_keep(100.0) >= 0.25`.
  - `grass_light(0.0) > grass_light(12.0) + 0.3`.
- [ ] **Step 3: Implement.** Run `cargo test grass`; expect PASS. Then run the full suite.
- [ ] **Step 4: Shot.** Run the same shot as Step 1.
  - Expect: far grass sparser and darker, near grass intact.
  - Expect: raymarch ms no worse than baseline.
- [ ] **Step 5: Commit** with message "ascii3d: grass recedes with distance".

### Task 2: Fighters pop

**Files:** `scene.rs` (`Body.flash`, contact shadow in `shade`), `ascii.rs` (outline tint, flash), `main.rs` (`World::tick` sets and decays flash)

**Interfaces:**
- `Body.flash: f32`.
  - Set to 1.0 for the target of every `Event::Hit` at impact.
  - Decays at `dt / 0.12` and never goes below 0.
- `pub fn flash_rgb(rgb: Vec3, flash: f32) -> Vec3` in `ascii.rs`.
  - Lerps toward white (`Vec3::ONE * 1.2`) by `flash`.
  - Applied to every glyph of that body.
- Outline tint: for edge glyphs of hero materials, `rgb = (sh.rgb * 1.2).lerp(body.accent, 0.35)`, then brightened to at least 0.45 luminance.
- `Scene::contact_shadow(&self, p: Vec3) -> f32`.
  - Returns a multiplier in [0.35, 1] for a ground point.
  - About 0.35 at a body's feet. The feet point is `to_world(Vec3::ZERO)`, so it moves with `pose.root`.
  - 1.0 beyond about 0.9 m.
  - Ignores bodies with `dim >= 0.99`.
  - `shade` multiplies ground (`Mat::Grass`) colour by it.

- [ ] **Step 1: Failing tests.**
  - `flash_goes_white`: `flash_rgb(v, 0.0) == v`; `flash_rgb(v, 1.0)` has every channel ≥ 1.0.
  - `shadow_under_feet`:
    - `contact_shadow(feet) < 0.5`.
    - `contact_shadow(feet + (2,0,0)) == 1.0`.
    - With `bodies[0].dim = 1.0`, `contact_shadow(feet0) == 1.0`.
- [ ] **Step 2: Implement.** Wire flash into `World::tick`: on impact, read the `Hit` targets from `flow.last`.
- [ ] **Step 3: Run tests.** `cargo test`; expect all pass.
- [ ] **Step 4: Shots.**
  - Idle shot: fighters' outlines visibly accent-tinted, with a dark patch under the feet.
  - `--moves 1 --at 0.47` (just past impact): the target is lit white.
  - Compare raymarch ms.
- [ ] **Step 5: Commit** with message "ascii3d: fighter outlines, contact shadows, hit flash".

### Task 3: Sky

**Files:** `ascii.rs` (sky path in `sample_cell`, new `sky_glyph`)

**Interfaces:**
- `pub fn sky_glyph(dir: Vec3, time: f32) -> Option<(char, Vec3)>`: the glyph for a sky ray. `None` means stay empty.
- **Hills:** a horizon ridge `ridge(az) = 0.04 + 0.05 * noise(az)`, where `elev = asin(dir.y)` and `az = atan2(dir.z, dir.x)`. Two summed sines are enough for `noise`.
  - Below the ridge: a dark hill glyph (`^`, `_`, `/`, `\` by local slope; otherwise `'.'`), colour about `(0.08, 0.1, 0.16)`.
  - A thin treeline band just under the ridge uses `^`/`A` in dark green.
- **Moon:** a fixed direction `MOON = normalize(0.45, 0.32, -1.0)` (behind the fighters, upper right).
  - Within 0.035 rad: a moon glyph (`●` centre, `O`/`o` toward the rim), pale `(0.95, 0.95, 0.85)`.
  - 0.035..0.09 rad: a faint halo of `.`.
- **Stars:** hash the direction quantised to about 0.012 rad cells.
  - About 2% of cells get `.`, `*` or `+`.
  - Brightness twinkles with `sin(time * k + hash)`.
  - No stars below `elev = 0.06`.
- `sample_cell`: when the owning id is `SKY` and every sub-sample missed, call `sky_glyph` with the ray through the cell centre. Return `CellOut::Glyph` with depth `MAX_DIST`, or `Empty` if `None`.
  - Keep `Empty(nearest)` for cells that hit something but fell under the 0.2 coverage threshold.

- [ ] **Step 1: Failing tests.**
  - `moon_where_expected`: `sky_glyph(MOON, 0.0)` is `Some` with a moon glyph.
  - `hills_below_ridge`: `dir = normalize(0.0, 0.01, -1.0)` gives a hill glyph with every channel < 0.25.
  - `stars_stable_and_sparse`:
    - `sky_glyph(d, t)` returns the same char for the same `d` at `t=0` and `t=5`. Colour may differ.
    - Over a 200×100 grid of upper-sky directions (elev 0.2..1.2), the fraction of `Some` results is between 0.5% and 5%.
- [ ] **Step 2: Implement.** Run `cargo test sky`; then the full suite.
- [ ] **Step 3: Shot.** Expect stars, the moon at upper right and a hill silhouette on the horizon. Compare raymarch ms; the sky costs no extra marching.
- [ ] **Step 4: Commit** with message "ascii3d: night sky with moon, stars, hills".

### Task 4: Rocks and trees

**Files:** `scene.rs` (rock list), `ascii.rs` (`fill_glyph` for Stone and Leaf)

**Interfaces and changes:**
- Rocks:
  - `(5.0, 0.1, -2.0), 1.2` becomes `(7.5, 0.0, -5.5), 0.8`.
  - Any rock whose centre is within 4.5 m of the duel centre moves further out.
  - Re-check the camera view in a shot.
- Stone glyphs: replace the `.:odQB` ramp with a surface pattern of shading blocks and cracks.
  - Base ramp `['.', ':', '░', '▒', '▓']` by luminance.
  - Every hash-chosen ~1 in 9 cell becomes a crack glyph (`/`, `\`, `_`).
  - Stone keeps its current colour.
- Leaves: replace the `.:;dUQ` ramp with `['.', ',', ';', '*', '%', '&']` by luminance.
  - Add a hash-chosen `'` and `"` scatter so canopies look leafy, not lettered.

- [ ] **Step 1:** Existing tests stay green: `cargo test`.
- [ ] **Step 2: Shot.** Expect no `dddQQQ` blobs, rocks clear of the duel spot, canopies readable. Compare raymarch ms.
- [ ] **Step 3: Commit** with message "ascii3d: stone and leaf glyphs, rock placement".

### Task 5: Hit feel (shake, hit-stop, bursts)

**Files:** `effects.rs` (`Shake`), `anim.rs` (hit-stop inside `Action`), `main.rs` (apply shake to the camera), `effects.rs` (burst counts)

**Interfaces:**
- `pub struct Shake { pub amp: f32, t: f32 }` in `effects.rs`.
  - `fn kick(&mut self, strength: f32)`: `amp = amp.max(strength)` and `t = 0`.
  - `fn update(&mut self, dt)`: amp decays to 0 within 0.3 s.
  - `fn offset(&self) -> Vec3`: a jitter of at most `amp` metres, using sines of `t` at a few frequencies, mostly x/y.
- Strengths:
  - Normal hit: 0.05.
  - Crit or fire orb: 0.11.
  - Fully absorbed hit: 0.03.
- `main.rs`: on impact, kick based on the `Hit` events in `flow.last`. Add `shake.offset()` to both `eye` and `target`; the offset is the same vector, so distance is unchanged.
- Hit-stop: `Action` gains `pub hold: f32` (seconds left).
  - When `advance` crosses the impact, set `hold = HIT_STOP` (0.08), but only if the move has a `Hit` event.
  - While `hold > 0`, `advance` consumes `dt` from `hold` before `t` moves.
  - `skip()` clears `hold`.
  - `done()` is unchanged (it means `t >= dur`).
- Bursts:
  - Normal hit sparks: 14 → 22.
  - Crit sparks: 30 → 44.
  - Orb embers: 40 → 60.
  - Add a white `Spark` flash ring of 10 at impact for any `Hit`.

- [ ] **Step 1: Failing tests.**
  - `shake_decays`: after `kick(0.1)`, `offset().length() <= 0.1 + 1e-4`. After `update(0.35)`, `amp == 0` and `offset() == ZERO`.
  - `shake_keeps_distance`: for `eye = (0,2,5)`, `target = (0,1,0)`, `(eye + o) - (target + o)` has the same length for any `o = offset()`. This pins the main.rs rule as a pure helper `fn shaken(eye, target, o) -> (Vec3, Vec3)`.
  - `hit_stop_freezes_then_resumes`: for an Attack action with a `Hit` event:
    - Advance to just past impact; it returns true once.
    - `t` stays at impact for 0.08 s of further `advance` calls.
    - Then `t` moves again.
    - Total frames to `done` equal the old frames plus about 0.08 s.
  - `no_hit_stop_without_hit`: a Rest action never sets `hold`.
  - `skip_clears_hit_stop`: advance into the hold, `skip()` returns false (the impact already fired), and `done()` is true.
  - Flow test `knockout_with_hit_stop_reaches_result`: KO attack, `update` in 1/60 steps, reaches `result` within 3 s.
- [ ] **Step 2: Implement.** Run `cargo test`; expect all pass.
- [ ] **Step 3: Shots.** Mid-orb impact (`--at 0.72`) shows bigger bursts. Shake is not visible in stills, which is expected; the tests cover it. Compare raymarch ms.
- [ ] **Step 4: Commit** with message "ascii3d: camera shake, hit-stop, bigger impact bursts".

### Task 6: Panels and menus

**Files:** `ui.rs`

**Interfaces:**
- `pub fn fit(s: &str, max_chars: usize) -> String`: returns `s` if it fits; otherwise truncates and ends with `…`. If `…` is missing from the font, use `...`. `max_chars < 2` returns an empty string.
- `pub fn wrap(s: &str, max_chars: usize) -> Vec<String>`: word-wraps; words longer than `max_chars` are hard-split.
- `pub fn draw_frame(font, x, y, w, h, color)`: draws `┌─┐│└┘` with glyphs along the rectangle edges, at a 13 px font; the size can be changed later.
- Panels:
  - Framed with `draw_frame` in a dim colour.
  - The active panel's frame is drawn in its accent colour.
  - An accent header strip (a filled rect at 25% alpha) holds the name, then `·`, then the class.
  - The metal line moves under the bars when present.
  - Every text line goes through `fit` using `chars = (panel_w - 20) / char_w`.
- Caption: each narration line goes through `wrap` with `chars = (screen_w * 0.9) / char_w`.
- Menu: drawn inside `draw_frame`. The selected row stays reverse video.
- The result banner gets a `draw_frame` in the winner's accent.

- [ ] **Step 1: Failing tests.**
  - `fit_truncates_with_ellipsis`:
    - `fit("abcdef", 10) == "abcdef"`.
    - `fit("abcdefghijk", 6).chars().count() == 6` and it ends with the ellipsis.
    - `fit("ab", 1) == ""`.
  - `wrap_breaks_on_words`:
    - `wrap("aa bb cc", 5) == ["aa bb", "cc"]`.
    - `wrap("abcdefgh", 3) == ["abc", "def", "gh"]`.
    - `wrap("", 5)` is empty or `[""]`; pick one and document it.
- [ ] **Step 2: Implement.** Run `cargo test`.
- [ ] **Step 3: Shots.**
  - 1280×800 mid-duel.
  - 800×500 with `--keys` entering two 16-character names and Warrior plus Sorcerer, followed by a move. Expect framed panels, nothing clipped and the caption wrapped.
- [ ] **Step 4:** Update `ascii3d/README.md` only if controls or flags changed. Expected: no change.
- [ ] **Step 5: Commit** with message "ascii3d: framed panels and menus, fitted text".
