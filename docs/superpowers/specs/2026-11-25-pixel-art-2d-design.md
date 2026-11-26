# Clash of Valor 2D: pixel-art duel with cut-out sprites (design)

Status: **approved by the user on 2026-11-26.** Supersedes the earlier "3D pixel art with
voxel characters" draft that lived at this path under the name
`2026-11-25-pixel-art-voxel-design.md` (see git history).

## Intent

A separate 2D pixel-art version of the Clash of Valor duel that `ascii3d/` plays today:
same classes, moves, rules, bot, menus, turn limit and effects, drawn as hand-made pixel
art instead of ray-marched glyphs. The user's reason is taste: they like the look of 2D
pixel art more than the 3D chunky-pixel spike. They want it **pixelated but smooth in both
senses**:

- **Smooth motion:** fluid swings, steps and recoils, not a few stiff frames.
- **Smooth look:** soft shading, gradients and hand anti-aliased edges inside sprites,
  not flat colour bands.

## Look (decided with mockups)

- **Three-quarter battle view.** The camera looks down at a clearing at a fixed angle;
  the two fighters stand on a diagonal facing each other (one upper-left, one
  lower-right), like a classic 16-bit RPG battle screen. Night setting: banded dithered
  sky, moon, pine treeline, grass, dirt clearing, campfire. The approved mockup is the
  "three-quarter" option in `.superpowers/brainstorm/*/content/view-angle.html` (local,
  not committed).
- **480x270 internal resolution, fighters about 110px tall.** Everything (backdrop,
  fighters, effects, HUD) renders into one 480x270 target, then scales to the window
  with nearest-neighbour sampling. One pixel size everywhere: no mixed-resolution
  sprites, no blur.
- Palette: start from the spike's night palette (`ascii3d/src/pixel.rs`), extended with
  the extra ramp steps that smooth shading needs. Not a hard fixed palette.

## Goals

1. Play the full duel (setup, moves, result, rematch, pause, bot) in the 2D look.
2. Fighters built from cut-out part sprites, animated every frame by the existing pose
   rig, so all current animations keep working and motion is smooth.
3. **Claude builds all the art to final quality**: fighters, backdrop, effects, HUD.
   The user reviews and steers but does not draw. Art lives in image files the user
   could still open in a pixel editor, one part at a time, with no code change.
4. Effects (slash arcs, sparks, fire orb, shield rim, crit vanish and afterimage, potion
   sparkles, floating numbers, hit flash, KO dim, screen shake) redrawn as pixel art.
5. A pixel-art HUD: pixel font, panels, bars and menus at 480x270.
6. The `--shot` screenshot path keeps working, so every visual change can be checked.

## Non-goals

- No camera orbit or 3D camera. Shake stays; the idle sway becomes a small parallax
  drift of the backdrop layers.
- No glyph renderer in this game, and no switch between looks.
- No voxels and no 3D models.
- No rule, move, bot or balance changes.
- No merging back into `ascii-3d`.

## Anti-goals (would count as failure even if it works)

- Puppet look: visible joint seams, limbs that slide apart, parts that pop in draw order.
- Blurry or mixed pixels: bilinear scaling, sprites at a different pixel size than the
  world, sub-pixel smeared rotation.
- Art locked in code: the user must be able to change a fighter by editing image files.

## Constraints

- Lives on branch `pixel-spike` (worktree `~/DEV/ClashOfValor-pixel-spike`). Never touch
  `~/DEV/ClashOfValor` or branch `ascii-3d`; another session is rewriting the glyph
  renderer for the GPU there.
- Keep `rules.rs`, `bot.rs` and `flow.rs` logic unchanged. Their tests keep passing.
- Keep `--shot`, `--duel`, `--moves`, `--at`, `--ff`, `--keys`, `--size`, `--metal`.
  Camera flags `--yaw` and `--dist` and the glyph flags go away.
- Runtime needs only files shipped in `assets/`. No editor or external tool at runtime.
- Rust + macroquad, as today. Develop on the Linux desktop. Before running the game or
  `--shot` in a shell without display variables:
  `export XDG_RUNTIME_DIR=/run/user/$(id -u) WAYLAND_DISPLAY=wayland-1`. Shots export
  upside down: `magick raw.png -flip -filter point -resize 400% out.png`.
- Every font or art asset that is not ours ships with its licence (as `assets/OFL.txt`
  does today).

## Approaches considered

Choices the user made along the way, in order:

1. **3D chunky pixels with voxel characters** (the earlier draft): the SDF scene
   ray-marched at 320x200 with dither and palette, characters as MagicaVoxel parts on the
   rig. Dropped: the user prefers the 2D look.
2. **2D view:** side view, three-quarter battle view (**chosen**), or isometric arena.
3. **Resolution:** 192x108 (fighters ~45px), 320x180 (~75px), or 480x270 (~110px,
   **chosen**).
4. **How fighters are built:**
   - **A. Cut-out part sprites on the existing pose rig (chosen).** Hand-drawn parts at
     the 3/4 angle, positioned every frame from the 3D rig projected through a fixed
     camera. Smooth motion at any frame rate, keeps every animation, art stays editable
     per part. Risk: puppet feel if joints are drawn badly.
   - B. 3D models rendered offline to sprite sheets, then palette-reduced and touched up.
     Smooth and consistent, but leans toward the pre-rendered 3D look the user moved away
     from.
   - C. Hand-drawn frame by frame. Best classic feel, but about 240+ frames at 110px; only
     realistic with months of the user's own pixel art.
5. **Who makes the art:** Claude, all the way to the final design. (Earlier plan:
   Claude makes starters and the user refines them. The user changed this: they review,
   Claude draws.) LibreSprite (`pacman -S libresprite`) or Aseprite can still open the
   PNGs if the user ever wants to touch something up.
6. **Replace or beside the glyph version:** separate game. This branch becomes the 2D
   game; the glyph game continues on `ascii-3d`. Cost accepted: later rules or flow
   fixes get copied between branches by hand.
7. **How the diagonal fighters face the viewer:** both cheated to a front three-quarter
   view, one mirrored (**chosen**: one sprite set per class, classic RPG convention), or
   one true camera with the near fighter seen from behind (about 2x the art, and PvP
   would need a rule for who stands near).

## How it works

### Shared code kept as-is

`rules.rs`, `bot.rs`, `flow.rs` (screens, menus, popups, input handling), `anim.rs`
(move timelines, `actor_pose`, `flinch_pose`, `idle_pose`, `collapse_pose`,
`actor_dim`, hit-stop) and the effect *simulation* in `effects.rs` (`Fx`, particles,
floating text, `Shake`, `on_action`). The world stays 3D in model terms: fighters have
world positions and yaw, effects have 3D positions. Only drawing becomes 2D.

### The rig and the projection

- The pose rig (`models.rs` `Pose`) keeps producing joint positions in each fighter's
  local space. Today the builders emit SDF primitives; in the 2D game a class instead
  emits a **skeleton**: named joints (hips, knees, feet, chest, neck, head, shoulders,
  elbows, hands, weapon tip, shield centre, hat tip and so on) from the same pose maths,
  plus the existing `Anchors` for effects.
- **2-bone IK for elbows and knees** (Claude's call; the user agreed to split limbs into
  upper and lower parts). The rig today gives only shoulder→hand and hip→foot. Elbow and
  knee come from fixed segment lengths and a fixed bend direction (elbows back and out,
  knees forward). No change to `anim.rs`.
- **Both fighters are cheated to a front three-quarter view** (user's choice, over one
  true camera that would show the near fighter's back). Each fighter's local-space
  joints (after root offset and fall, as in `Body::to_world` but without the world yaw)
  go through the **same fixed front-3/4 orthographic view of a fighter facing right**
  into sprite space. The fighter's world position only decides where on screen that
  posed figure stands (upper-left or lower-right of the clearing) and which fighter
  draws first (the lower one on screen draws last). The fighter facing left is the
  mirror image.
- One sprite set per class. Mirroring swaps sword and shield hands on screen; that is
  the accepted classic convention.
- Each part is placed by its two joints: position from the first, rotation from the
  direction to the second. Draw order within a fighter comes from depth in that
  front-3/4 view, with fixed tie-breaks per class (far arm behind torso, near arm in
  front).
- **Sprites never stretch** (Claude's call, from the "one pixel size" anti-goal). When a
  limb or weapon points toward or away from the viewer, the rig supplies only the
  direction and the part keeps its drawn length. Elbows, knees and weapon tips are
  solved in 2D from sprite lengths, and effect anchors (`weapon_tip`, `hand_r`,
  `chest`, `focus`) are read from that 2D solve, so effects line up with the drawn
  weapon. If a pose looks wrong because of this, the fix is an alternate sprite for that
  part (for example a sword seen end-on), not stretching.

### Part sprites (the art files)

- One PNG per part per class, for example `assets/fighters/warrior/forearm_near.png`.
  Near and far limbs can share art; the far copy is drawn a shade darker.
- Each class has a small text file next to its PNGs: for each part, the pivot pixel (the
  joint it hangs from), the point that should line up with its second joint, its draw
  layer and which palette slots it uses. Keep the format readable and hand-editable.
- Joints are hidden by overlap: each part extends past its joint, and round joint caps
  (shoulder, elbow, knee) are drawn into the parent part.
- **Palette slots instead of baked colours** for things that change at runtime: the
  Warrior's armour metal (M/N keys and `--metal`), the cloth accent that follows the
  player colour. Sprites paint those areas in reserved key colours, swapped at draw time.
- Hit flash brightens the sprite toward white; KO and crit vanish fade it (`Body.flash`,
  `Body.dim`, as today).
### How the art gets made

- **Art source is hand-placed pixel maps** checked into the repo: each part written
  pixel by pixel as a text grid (one character per palette entry), not built from
  geometric shapes. Shapes are fine for a first blocking pass; the final art is the
  pixel map. A build step (a small Rust tool in the repo) turns the maps into the
  shipped PNGs and pivot files. Generated PNGs are committed, so the game never needs
  the tool to run.
- Shading is part of the drawing: light from the moon (upper left), 3 to 5 tone ramps
  per material, hand anti-aliasing on curves, selective outlines (dark outline on the
  silhouette, softer coloured lines inside). This is what delivers the "smooth look".
- **Review loop:** every part and every class is checked in real `--shot` frames
  (idle and mid-move) and shown to the user, in the browser mockup page or as PNGs,
  before moving on. The user approves each class's look before its effects and polish.
  Expect several rounds per class.
- If the user ever edits a PNG directly, the build step must detect it and not
  overwrite it silently.

### Backdrop

Layered PNGs (sky, moon, treeline, ground and clearing), made the same way as the
fighters and reviewed the same way. The campfire is a small animated sprite
with glow. Idle sway becomes a slow parallax drift of the far layers; shake offsets the
whole frame in whole pixels.

### Effects

`Fx` keeps simulating particles in 3D, but because each fighter is cheated to its own
view, effects are placed in screen space: an effect attached to a fighter starts at that
fighter's 2D anchor; an effect that travels between fighters (fire orb, crit dash) moves
between the two fighters' screen anchors along the 3D timeline's progress; ground
effects use the backdrop's 3/4 ground mapping. Drawn as pixel sprites or single pixels: embers, sparks, smoke puffs, sparkles, the fire orb
with a dithered glow, slash arcs as short animated sprite strips, the shield rim as an
outline glow around the Warrior, the crit afterimage as a faded copy of the posed
fighter, floating numbers in the pixel font. Depth against fighters uses projected depth
per sprite, not a per-pixel depth buffer, unless the builder finds that is needed.

### HUD and menus

Same information and layout logic as today (`ui.rs` panels, bars, move menu with costs
and dimmed unavailable moves, log of the last four moves, popups, banners, result
screen, name entry, pause menu), redrawn at 480x270 with a pixel font and pixel panels.
Keyboard controls stay as the README lists them, minus camera and glyph keys.

### Screenshots and tests

- `--shot` renders the 480x270 frame and writes it scaled by an integer factor
  (nearest-neighbour), so screenshots show true pixels.
- Unit tests for the new pure parts: IK solving, projection, part placement from two
  joints, draw order, pivot file parsing, palette slot swapping.

## Left to the builder

- **Rotation quality.** Rotating pixel art with nearest sampling on the GPU is simplest
  but can look jagged; alternatives are pre-baked rotations every few degrees or a
  RotSprite-style upscale-rotate-downscale. Pick by looking at `--shot` output against
  the "no smeared pixels" anti-goal.
- Camera angle and exact framing for the 3/4 view (match the approved mockup).
- Exact skeleton joints and part list per class (about 12 parts each): Warrior (helm with
  plume, plate torso, tabard, pauldrons, chain arms, legs, sword, kite shield), Assassin
  (hood, torso, cloak, arms, legs, two daggers), Sorcerer (pointed hat, beard, robe,
  sleeves, staff with crystal).
- Pivot file format (keep it hand-editable).
- Window scaling: integer scale with letterbox where possible; behaviour for windows
  smaller than 480x270 at 1x (today's "please enlarge" message is the precedent).
- Pixel font choice (a bitmap font with a permissive licence, shipped with its licence).
- Crate layout and naming: whether to rename `ascii3d/` (for example to `pixel2d/`,
  binary `clash-of-valor-2d`) and how much glyph-only code (`ascii.rs`, `pixel.rs`, the
  SDF parts of `scene.rs` and `models.rs`) to delete versus keep while porting.
- Order of milestones, as long as an early milestone gives a playable duel with
  blocked-in placeholder art and a working `--shot`, and the final art for each class
  goes through the user's review before the next class starts.
