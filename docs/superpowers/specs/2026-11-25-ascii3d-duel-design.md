# Clash of Valor 3D: turn-based duel with move effects

Date: 2026-11-25
Branch: `ascii-3d`
Code: `ascii3d/` (Rust, macroquad + rayon). The C++ terminal game in `src/` is not touched.

## Intent

The 3D glyph renderer in `ascii3d/` is a sandbox today: one knight walks around a
campfire clearing. The user wants it to become the first playable piece of the next
Clash of Valor: the original game's **Player vs Player duel**, played in the 3D world.

Two people share one keyboard and take turns. Their characters stand facing each other
and act out each move with visible glyph effects, the way the original game narrates
and flourishes each move in text. There is no walking around in this version; it is a
duel where you take turns.

The user's own words: "If two people were taking turns fighting, I want their
characters to have effects like we have in the original version of the game. The game
will take turns. There will not be moving around for the initial version, it will be
like a duel where you take turns." And: "I want all classes to look different."

## Goals

1. A complete local two-player duel loop inside the macroquad window: setup, turns,
   knockout or flee, then rematch or new duel.
2. The three classes from the C++ game (Warrior, Assassin, Sorcerer) with the same
   moves, costs and numbers as the C++ rules (below).
3. Each class has its own distinct 3D model, readable at a glance.
4. Every move plays a short 3D animation with glyph effects. Every change to health,
   shield or mana shows on screen as it happens.
5. The narration text from the C++ game appears as a caption for each move.

## Non-goals (this version)

- Networking, online play, or two machines.
- Walking around, positioning, or any movement input during the duel.
- A bot opponent. (The rules module should not make one hard to add later.)
- XP, levels, coins, saving or loading characters (the C++ game's SQLite saves).
- Sound.
- Changing the C++ game or sharing code with it.

## Anti-goals (would make this a failure even if it works)

- Effects so busy that you cannot tell who did what to whom.
- Classes that look alike in silhouette, so the user has to read a label to know who
  is who.
- A frame rate that makes the duel feel broken. Target: smooth (about 30 fps or
  better) at the default glyph size on the user's Mac. The G key still trades detail
  for speed.
- Rules that quietly diverge from the C++ game without being written down here.

## Approaches considered

- **A. Everything in the Rust prototype (chosen).** The single hero becomes a list of
  fighters, each with a class model, a placement and a pose. The C++ combat rules are
  copied into a small Rust module. Effects are a mix of real SDF objects (lit,
  occluded) and free-floating projected glyphs (like the existing campfire embers).
  Trade-off: the rules live in two places, C++ and Rust.
- **B. C++ runs the fight, Rust only draws it over IPC.** One copy of the rules, but a
  protocol, two processes, and sync bugs. Too much plumbing for a first version, and
  the 3D game is meant to replace the terminal one.
- **C. Draw the 3D duel in the terminal from C++.** Rejected: the prototype already
  chose its own window, and a terminal cannot do the half-size hero glyph grid.

The user picked A.

## Rules (port of the C++ game, exactly)

Source of truth: `src/Character.{h,cpp}`, `src/Warrior.cpp`, `src/Assassin.cpp`,
`src/Sorcerer.cpp`, `src/main.cpp` (`newCharacter`, `runBattle`, `playerVsPlayer`).

Starting stats for a new fighter (level 1), all classes:
health 100 / max 100, stamina 100, damage 10, health potions 2.

- Warrior: shield 25 / max 25.
- Assassin: crit multiplier 1.5.
- Sorcerer: mana 50 / max 50, mana potions 3.

Stamina has no upper cap (C++ `setStamina` only clamps at 0). Health clamps to
`[0, maxHealth]`; shield to `[0, maxShield]`; mana to `[0, maxMana]`.

Moves:

| Class | Move | Cost | Effect |
|---|---|---|---|
| Warrior | Attack | 10 stamina | opponent takes 10 |
| Warrior | Raise shield | 15 stamina | shield +25, capped at max. In C++, at full shield the 15 stamina is still spent and nothing else happens (`spendStamina` runs before the full check). With dimming (below) this case cannot be picked. Warriors start at 25/25, so Raise shield is dimmed until they take a hit |
| Assassin | Attack | 10 stamina | opponent takes 10 |
| Assassin | Crit Attack | 20 stamina | opponent takes `int(10 * 1.5)` = 15 |
| Sorcerer | Cast fire orb | 10 mana and 10 stamina (both checked before either is spent) | opponent takes 10 + 4 = 14 |
| Sorcerer | Drink mana potion | 1 mana potion | mana +50, capped. Refused if no potions or mana full |
| All | Drink health potion | 1 health potion | health +50, capped. Refused if no potions or health full |
| All | Rest | none | health +10 (capped), stamina +25. Sorcerer also mana +20 (capped) |
| All | Flee | none | the duel ends with no winner |

Taking damage: a Warrior's shield absorbs damage first and only the rest reaches
health. Other classes take it all on health.

Turn order: player 1 always acts first, then player 2, alternating. After each move,
if the opponent's health is 0 the actor wins.

**One deliberate change from C++ (approved by the user):** in C++, a
move the fighter cannot pay for (not enough stamina or mana, no potions, already full)
prints a refusal and still ends the turn. In the 3D duel, such moves are shown dimmed
in the menu with a short reason ("needs 20 stamina", "no potions left", "already at
full health") and cannot be picked, so a turn is never wasted by a misclick. Rest and
Flee are always available, so a player always has a legal move.

Narration: each move's caption reuses the C++ text, for example "Name unleashes a
devastating slash with their blade, cleaving through with raw power!", plus the result
line ("Name's shield absorbs 10 damage (15/25 remaining).", "+50 health (1 left).").

## Class models (each must look different)

The current knight is built from SDF primitives in `Scene::build_hero` and rebuilt each
frame so limbs can move. Each class gets its own builder in the same style, in hero
local space (feet at origin, facing +z), driven by a pose (see Animation).

- **Warrior:** the current knight. Plate torso and pauldrons, chainmail coif and arms,
  blue tabard with gold `$` ornaments, a sword. Add a shield on the off arm (it is the
  class's mechanic). The M/N metal cycling keeps working and applies to the Warrior.
- **Sorcerer:** a long robe (a cone or flared shape to the ground, no visible legs), a
  tall pointed hood or hat, long sleeves, and a staff taller than the body with a
  glowing crystal or orb on top (emissive, with a soft halo like the campfire). Robe
  fill uses its own surface-space glyph pattern (for example runes or stars stitched
  in rows, in the same way cloth does `$`), in a deep purple or crimson.
- **Assassin:** slimmer and shorter than the Warrior, slightly crouched stance. Dark
  leather (its own material with a stitched or wrapped glyph pattern), a hood with a
  shadowed face, a short cloak, and two daggers held low, one per hand.

New materials are fine (for example Robe, Leather, Crystal). Hero materials must count
as `is_hero()` so they draw on the fine grid.

Silhouette test: the three must be told apart by shape alone, with outlines on and
colour ignored.

**Mirror matches (designer's call):** both players may pick the same class, as in C++.
Each player gets an accent colour so two fighters of the same class are still easy
to tell apart. Player 1 is blue and player 2 is red. The accent shows on the tabard,
robe trim or cloak, on that player's status panel, and on their name tag. Appearance
settings, including the Warrior's armour metal, are stored per fighter, not as one
shared `scene.metal`. M/N cycles the metal of whichever Warrior is the active fighter.

## Stage and camera

- Same clearing, trees, rocks and campfire. The two fighters stand about 3.5 to 4 m
  apart, facing each other, with the campfire off to one side so it lights them both.
- The camera frames both fighters side-on, slightly elevated, so both are in view the
  whole duel. During an action it may ease toward the action (for example following a
  fire orb in flight), then ease back.
- Right-mouse orbit and wheel zoom keep working.
- **The binary becomes duel-only (user's decision).** The walk-around sandbox is
  removed: WASD movement, the walk cycle, `--walk`, and the single `scene.player`.
  It stays in git history. `--yaw`, `--dist` and `--metal` may be kept for `--shot`
  if useful.
- The grass push-aside effect follows the fighters' feet instead of a single player.

## Screens and input

All on-screen menus follow the codebase's menu rules (from the project `CLAUDE.md`):
arrow keys move a highlighted selection, Enter confirms, number keys jump to and
confirm an option. There is no `>` prompt. Name entry is a plain text field.

1. **Setup:** "Player 1" types a name (Enter to accept, non-empty, sensible length
   limit), then picks a class from the three-item list with the same one-line
   descriptions as the C++ game. Then the same for "Player 2". The selected class
   previews in 3D at that player's spot as you move through the list.
2. **Duel:**
   - Top or sides: one status panel per fighter, on that fighter's side of the
     screen. Name, class, health bar, then stamina, and the class bar (shield for the
     Warrior, mana for the Sorcerer), potions left. Bars animate toward new values
     rather than jumping.
   - Whose turn it is is obvious: the active fighter's panel is highlighted and a
     marker floats over their head (the current "you" name tag becomes a name tag
     over each fighter).
   - The bottom panel shows the active player's move menu, which depends on their
     class.
   - The caption line shows the narration for the move playing now.
3. **Result:** the loser collapses; a banner shows "Name wins!" (or "Name fled the
   fight!"). Menu: Rematch (same names and classes, stats reset), New duel (back to
   setup), Quit.

Esc quits from any screen. Input is ignored while a move animation plays, except
Space or Enter, which skips to the end of the animation (all state changes still
apply).

## Animation and effects

Each move plays one action of roughly 1 to 1.5 s: wind-up, strike or cast, impact,
recover. The rules resolve at the start of the move, but the displayed numbers (bars,
damage text) change at the impact moment, so what you see matches what happens.

A pose is a small set of parameters (arm angles, torso lean, root offset, weapon
angle) that each class builder reads. Idle fighters breathe or sway slightly so the
scene never looks frozen. Melee attackers may step or lunge toward the target and
return to their spot; that is animation, not movement.

Two kinds of effect:

- **SDF effects** (real objects in the scene, lit and occluded, drawn as glyphs):
  the fire orb, the Warrior's shield dome or rim, the Sorcerer's staff glow, potion
  bottle glow.
- **Overlay effects** (free glyphs projected from 3D points and depth-tested against
  the coarse grid, like the existing embers): sparks, slash trails, damage and heal
  numbers, sparkles, smoke.

Per move:

| Move | Effect |
|---|---|
| Warrior Attack | Lunge and sword swing. A bright arc trail of `/ - \ |` glyphs follows the blade tip and fades. Sparks burst at the target on impact |
| Warrior Raise shield | Shield comes up; a glowing glyph dome or rim forms around the Warrior and settles to a faint shimmer while shield > 0. When later hits are absorbed it flashes and shows "-N shield" in blue |
| Assassin Attack | Quick dash and stab. Short sharp streak and small spark burst |
| Assassin Crit Attack | Assassin fades to dim glyphs or smoke, reappears at the target's side, strikes. A larger red burst and a bigger, highlighted damage number ("CRIT 15") |
| Sorcerer Cast fire orb | Staff glows and raises. A fire orb (emissive, `@ * o` glyphs, ember trail reusing the ember system) flies to the target and explodes into embers and a flash |
| Sorcerer Drink mana potion | Blue sparkles rise around the Sorcerer, "+N MP" floats up in cyan |
| Drink health potion | Green sparkles rise around the fighter, "+N HP" floats up in green. This echoes the C++ `✧ ✦ ✧` flourish (`src/Animate.cpp`) |
| Rest | Fighter lowers weapon and breathes; soft gold motes; "+N HP +N ST" floats up |
| Flee | Fighter turns and fades out in smoke |
| Any hit | Target flinches (short recoil), a red damage number floats up and fades |
| Knockout | Target collapses to the ground. Its glyphs dim |

Colours stay consistent: red for damage, green for health, cyan for mana, blue for
shield, gold for stamina.

## Rendering and performance

- The scene holds a list of fighters instead of one hero. Part ids must be unique
  across all fighters and effects (outlines depend on ids). The SDF bounding-sphere
  test per fighter stays.
- Both fighters draw on the fine (half-size) grid when the frame rate allows. If two
  fighters on the fine grid are too slow at the default glyph size, the first
  fallback is fine grid for the acting fighter only; the builder picks the exact
  rule.
- Effects must not slow the frame much. SDF effects need a bounding sphere like
  props; overlay effects are cheap.

## Code shape

- A rules module with no macroquad dependency: fighter stats, classes, moves,
  availability with reasons, and resolving a move into a list of outcomes (damage
  dealt, shield absorbed, healed, and so on) plus narration strings. This is the part
  that gets unit tests (`cargo test`), checked against the table above.
- The scene and models: fighters, class builders, poses, new materials and glyph
  functions.
- A duel state machine: setup, choosing, animating, result.
- Effects: SDF effect parts and the overlay particle system (the ember code
  generalised).
- UI drawing for panels, menus, captions, banners.

The `--shot` screenshot mode stays working. Extend it so a frame mid-move can be
captured (for example `--shot out.png --duel warrior,sorcerer --move 1 --at 0.6`) to
check effects without playing by hand. Exact flags are the builder's call.

## Left to the builder

- File layout inside `ascii3d/src/`.
- Exact primitives, proportions, colours and glyph sets for each class model, within
  the descriptions above.
- Exact timings, easing and particle counts.
- Camera framing details.
- How the performance fallback decides.
- Updating `ascii3d/README.md` with the duel controls.
