# Clash of Valor 3D: pixel-art renderer with voxel characters (design draft)

Status: **draft, not yet approved by the user.** Brainstorming moved from the Mac to the
Linux desktop mid-conversation. Settle the open question below with the user, get
approval of this doc, then write the implementation plan.

## Intent

Same game as `ascii3d/` (the turn-based duel: classes, moves, menus, effects, bot,
camera), drawn as pixel art instead of glyphs. The user wants the "chunky 3D pixels"
look, with much more detailed character design than the current primitive-built fighters.

## What was tried (spike, branch `pixel-spike`)

`ascii3d/src/pixel.rs` plus a `--pixel <chunky|mono|sprites>` flag on the `--shot` path in
`main.rs`. It is throwaway spike code with no tests. Reuse its ideas, not necessarily its code.

```sh
cd ascii3d && cargo build --release
./target/release/clash-of-valor-3d --shot raw.png --pixel chunky \
    --duel sorcerer,warrior --moves 1 --at 0.5 --size 1280x800
magick raw.png -flip -filter point -resize 400% chunky.png   # export comes out upside down
```

The four styles compared:

1. **Chunky 3D pixels (chosen).** The same SDF scene, ray-marched at 320x200 with one ray
   per pixel, upscaled 4x with nearest-neighbour. Colours snap to a fixed 34-colour night
   palette after a 4x4 Bayer dither. Other parts of the look:
   - 1px near-black silhouette outlines around fighters.
   - Inner lines where a part sits behind a nearer part of the same fighter.
   - Additive glow around emissive pixels, which the dither turns into rings.
   - Particles drawn as single depth-tested pixels.
   - The sky is drawn per pixel: banded gradient, moon with craters, 1px stars, pine
     treeline.
2. 16-bit hand-drawn side-view sprites. Not mocked up.
3. Sprites rendered from the 3D models over a painted 2D backdrop (rejected: the camera
   can no longer orbit or sway).
4. A 4-shade palette variant of style 1 (rejected as the main style; could return as a
   palette toggle).

What the spike showed:

- The scene, moon, fire, orb and embers already look good at 320x200.
- The fighters look lumpy. At 320x200 a fighter is only about 75px tall, and the models
  are about 16 SDF primitives each (`ascii3d/src/models.rs`).

## Decisions so far

- **Style 1, chunky 3D pixels.** Keep the 3D scene, the orbit/sway/shake camera and all of
  the duel flow.
- **Characters become voxel models made in MagicaVoxel** (the user picked this over
  Blender meshes and over richer SDF models in code). Each body part is its own `.vox`
  model: head, torso, upper and lower arms, legs, weapon, shield, hat or hood and so on.
  The game loads the parts and attaches them to the existing pose rig (`models.rs`
  `Pose`/`Anchors`, `anim.rs`), so every current animation keeps working.
- **Fighters get more pixels than the world**, about 2x (the glyph version's fine pass is
  the precedent). This goes with the voxel work, because detail is capped by fighter pixel
  height.
- **Development moves to the Linux desktop** (better GPU). The Mac could not handle it.
  This makes the GPU path from the ascii3d README ("Where to go next") realistic for this
  renderer too.

## Open question for the user (ask first)

Who sculpts the characters? Either the user sculpts them in MagicaVoxel, or Claude
generates starter `.vox` parts by script for the user to refine. The answer changes the
first milestone. MagicaVoxel is a GUI app: on Linux it runs under Wine, or Goxel is a
native open-source alternative that also exports `.vox`. Check which one the user wants.

## Constraints

- Keep the existing duel logic untouched: `rules.rs`, `bot.rs`, `flow.rs`, `ui.rs` layout
  logic.
- The `--shot`/`--duel`/`--moves`/`--at` screenshot path must keep working. It is how
  every visual change gets checked.
- External tools are optional at runtime. Ship the exported `.vox` files in `assets/`, and
  the game must not need MagicaVoxel installed to run.
- Another Claude session is still working on the glyph version on branch `ascii-3d`, and
  has an uncommitted design for a GPU glyph renderer:
  `docs/superpowers/specs/2026-11-26-ascii3d-gpu-renderer-design.md` on the Mac. Don't
  break `ascii-3d`. Do the pixel work on its own branch, starting from `pixel-spike`. Decide
  with the user whether pixel art replaces glyphs or lives beside them as a mode.

## Approaches considered for detailed characters

- **A. Voxel parts from MagicaVoxel on the existing rig (chosen).** Fits the chunky look,
  gives the user a real design tool and keeps the animation system. Costs: a voxel tracer
  in the renderer (DDA inside each part's box, composited with the SDF world by depth),
  joint gaps hidden by overlapping parts, and detail capped by voxel size.
- **B. Blender meshes (glTF) with a skeleton.** Highest detail. It is the biggest change:
  a triangle renderer composited with the ray-marched world plus a skinning pipeline, and
  much of the fine detail is lost at about 150px tall.
- **C. Richer SDF models in code.** No tool to install, but the user can't sculpt, and the
  detail is limited by what primitives can express.

## Left to the builder

- CPU voxel DDA versus moving the whole renderer to the GPU now. The desktop GPU makes a
  GPU pass attractive; measure before deciding.
- How voxel colours map onto the palette and materials (metal glints, the accent cloth
  that follows the player colour, hit flash, dim on KO).
- Voxel scale (voxels per metre) and the fighter render resolution.
- Pixel-art HUD: a pixel font and panels to replace the monospace glyph UI.
