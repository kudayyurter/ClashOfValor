# Clash of Valor 3D: visual pass

Date: 2026-11-25
Branch: `ascii-3d`
Code: `ascii3d/` (follows the duel in `2026-11-25-ascii3d-duel-design.md`)

## Intent

The duel works, but it is hard to read and the hits lack punch. Grass covers most of
the screen at the same brightness as the fighters. The fighters are small and dim.
The sky is an empty black band. The rocks render as `dddQQQ` blobs. The panels are
plain boxes. The user agreed to fix all six of the problems below in one pass.

## Goals

1. **Grass recedes.** Beyond about 3 m from the duel centre, grass thins out (more
   blanks and `.`) and darkens with distance. Near the fighters it is dimmed slightly
   but keeps its sway and parting.
2. **Fighters pop.** Outline glyphs on fighters draw brighter, tinted with the
   player's accent. Each fighter has a dark contact shadow on the ground under its
   feet. A hit makes the target's glyphs flash white for about 0.12 s at the impact.
3. **Sky.** Sky cells show twinkling stars (`.` `*` `+`, hashed on view direction so
   they stay put as the camera orbits) and a decorative moon disc with a soft halo. A
   dark hill and treeline silhouette runs along the horizon. Only sky cells pay for
   this.
4. **Rocks and trees.** Stone gets a new glyph set that reads as stone, not letters.
   The big rock on the right is made smaller and pushed further back. Leaves get a
   cleaner pattern.
5. **Hits feel heavy.**
   - A short camera shake on impact. Crits and fire orbs shake harder.
   - A hit-stop of about 0.08 s, where the animation freezes at the impact moment.
   - Bigger, brighter impact bursts.
   - The camera distance never changes: no zoom (the user's standing rule).
6. **Panels and menus.**
   - Box-drawing frames (`┌─┐│└┘`, available in the bundled font).
   - An accent-coloured header strip with the name and class.
   - The move menu framed to match.
   - The panel class line and narration shorten or wrap so nothing clips at
     800×500. This fixes two minor issues from the duel review.

## Non-goals

- Moving the renderer to the GPU.
- Any change to rules, flow, or controls. One exception: the hit-stop delays the end
  of an action by its own length.
- Zoom, or any change to camera distance.

## Anti-goals

- A slower frame. Raymarch time at the default view must not get worse. Measure with
  shots before and after; thinning the grass should make it faster.
- Effects or shake so strong that it becomes hard to follow who did what.
- A sky or ground so busy that it competes with the fighters again.

## Approach

Everything stays in the CPU ray marcher and glyph layer:

- Grass changes go in `grass_glyph`.
- Outline tint and hit flash go in `sample_cell`, using the owner body. The hit flash
  is a new `Body.flash`.
- The contact shadow is a darkening term in `Scene::shade` for ground hits near each
  body's feet.
- The sky is a new path for `SKY` hits in `sample_cell`, which today return `Empty`.
- Shake and hit-stop are pure state that main.rs reads: a `Shake` (or similar) value
  that decays, and a hit-stop timer in the flow or the action.

The GPU alternative was rejected for this pass as too big.

## Verification

- Same-moment screenshots before and after: idle, mid-slash, orb impact, knockout.
  Also check 800×500.
- Raymarch ms from those shots must be compared, and must not regress.
- Pure logic gets unit tests: shake decay and strength per move, hit-stop timing (the
  impact still fires exactly once, and skip still works), text shortening for panels
  and captions, and grass density falling off with distance.

## Left to the builder

Exact colours, glyph sets, thresholds, shake amplitude, and the moon's position, all
within the goals above.
