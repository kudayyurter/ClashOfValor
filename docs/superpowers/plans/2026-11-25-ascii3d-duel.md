# Clash of Valor 3D Duel Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the `ascii3d/` walk-around prototype into a local two-player, turn-based duel between three visually distinct classes, with a glyph effect for every move.

**Architecture:** A pure rules module (no macroquad) ports the C++ combat. The scene holds two fighter bodies, each rebuilt every frame from a class model and a pose. A duel state machine in `main.rs` moves through setup, choosing, animating and result. Each move plays an `Action` timeline that drives poses and spawns effects, and applies the new numbers on screen at the impact moment. Effects are either SDF parts added to the scene (fire orb) or free glyphs projected from 3D (sparks, trails, numbers, sparkles), drawn the way the campfire embers are drawn today.

**Tech Stack:** Rust 2024, macroquad 0.4, rayon. Tests use `cargo test` with in-file `#[cfg(test)] mod tests`.

**Spec:** `docs/superpowers/specs/2026-11-25-ascii3d-duel-design.md`. Read it before any task. It holds the full rules table, the class descriptions and the per-move effect table.

## Global Constraints

- Every change is inside `ascii3d/`. Do not touch the C++ game in `src/`.
- Rules match C++ exactly, except for the one approved change: unaffordable moves are dimmed and cannot be picked.
- Starting stats: health 100/100, stamina 100 (no upper cap), damage 10, health potions 2. Warrior shield 25/25. Assassin crit 1.5. Sorcerer mana 50/50, mana potions 3.
- Costs: Attack 10 stamina. Crit 20 stamina, deals `int(10*1.5)` = 15. Raise shield 15 stamina, +25 shield capped. Fire orb 10 mana and 10 stamina, deals 14. Health potion +50 capped. Mana potion +50 capped. Rest +10 health capped and +25 stamina; the Sorcerer also gets +20 mana capped.
- Menus follow the project `CLAUDE.md` rules:
  - Option lists are centred horizontally on the live window width, measured every frame.
  - The selected row is highlighted in reverse video. There is no `>` prompt.
  - Arrow keys move the selection. Enter confirms. Number keys jump to an option and confirm it.
  - A menu disappears as soon as an option is chosen.
- Colours: damage red, health green, mana cyan, shield blue, stamina gold. Player 1's accent is blue, player 2's is red.
- The binary is duel-only. Remove WASD, the walk cycle, `--walk` and the single `scene.player`.
- `cargo test` must never open a window. Tests cover pure code only: rules, menu, name field, action timeline, id ownership.
- Performance target: about 30 fps or better at the default glyph size (16 px) on the user's Mac, measured with `cargo run --release`.

## Review Focus

1. **Name entry edge cases.** Empty or whitespace-only names cannot be accepted. Names are capped at 16 characters. Only printable ASCII is accepted, because the font may lack other glyphs. Backspace on an empty field is a no-op. (Task 4 tests.)
2. **Key mashing during an animation.** Keys pressed while a move plays must not queue a move for the next player. Skipping a move with Space or Enter applies the impact exactly once, even when the skip comes after the impact. (Task 5 tests.)
3. **Mirror matches.** In Warrior vs Warrior, part ids never collide, each fighter keeps its own metal and accent, and M/N only changes the active fighter. (Task 2 `owner_of_ids` test, Task 3 visual check.)
4. **Long fights with repeated Rest.** Stamina goes above 100. The stamina bar must not overflow its panel: the bar is clamped to full and the number shows the real value. A fighter with zero stamina, mana and potions still has Rest and Flee. (Task 1 `rest_and_flee_always_available` test, Task 4 visual check.)
5. **Small or resized window.** At 800×500 the two status panels, the centred menu and the caption must not overlap. Layout is computed from `screen_width()` and `screen_height()` every frame. (Task 4 visual check with a screenshot at that size.)

---

## File Structure

| File | Responsibility |
|---|---|
| `ascii3d/src/rules.rs` (new) | Fighter stats, classes, moves, availability, resolving a move, the duel turn loop, narration copy. Pure std, no macroquad. |
| `ascii3d/src/models.rs` (new) | `Pose`, `Anchors`, class model builders that return SDF parts in local space. |
| `ascii3d/src/scene.rs` (modify) | World and props as today. `Body` (one per fighter) replaces the single hero. Owner lookup by id. Per-fighter metal, accent and dim. New materials. |
| `ascii3d/src/ascii.rs` (modify) | Glyph choice for the new materials. Per-body surface UV and dim. Fine-pass mask per body. |
| `ascii3d/src/effects.rs` (new) | Overlay particles (embers generalised), floating text, the fire orb, ward shimmer. Update logic, plus a draw function. |
| `ascii3d/src/anim.rs` (new) | `Action` timeline (duration, impact) and the pose for each move phase, idle, flinch and collapse. |
| `ascii3d/src/ui.rs` (new) | `Menu`, `NameField`, drawing of panels, menus, caption, banner and setup prompts. |
| `ascii3d/src/main.rs` (modify) | Window, args, render passes (as today), duel state machine, input routing, `--shot` flags. |
| `ascii3d/README.md` (modify) | Duel controls and the new `--shot` flags. |

Part id ranges, which every task relies on:
- Body 0 uses 10–49.
- Body 1 uses 50–89.
- Effects use 90–99.
- Props start at 101, unchanged.
- `GROUND` (1) and `SKY` (0) are unchanged.

---

### Task 1: Rules module

**Files:**
- Create: `ascii3d/src/rules.rs`
- Modify: `ascii3d/src/main.rs` (add `mod rules;` only)
- Test: in-file `#[cfg(test)] mod tests` in `rules.rs`

**Interfaces:**
- Produces:
  - `pub enum Class { Warrior, Assassin, Sorcerer }`, deriving `Clone, Copy, PartialEq, Eq, Debug`. Methods:
    - `fn name(self) -> &'static str`: "Warrior", "Assassin", "Sorcerer".
    - `fn blurb(self) -> &'static str`: the C++ setup descriptions, exactly:
      - "carries a shield that absorbs damage before health"
      - "trades survivability for a high-damage critical strike"
      - "spends mana on fire orbs that hit harder than steel"
    - `const ALL: [Class; 3]`.
  - `pub struct Fighter`, deriving `Clone, Debug`. Public fields: `name: String`, `class: Class`, `health: i32`, `max_health: i32`, `stamina: i32`, `damage: i32`, `health_potions: i32`, `shield: i32`, `max_shield: i32`, `crit: f32`, `mana: i32`, `max_mana: i32`, `mana_potions: i32`. Fields a class doesn't use are 0. Methods:
    - `fn new(name: &str, class: Class) -> Fighter`
    - `fn alive(&self) -> bool`
    - `fn take_damage(&mut self, amount: i32) -> (i32 /*absorbed*/, i32 /*health lost*/)`
  - `pub enum Move { Attack, RaiseShield, CritAttack, FireOrb, HealthPotion, ManaPotion, Rest, Flee }`, deriving `Clone, Copy, PartialEq, Eq, Debug`. `fn label(self) -> &'static str` returns the C++ menu text: "Attack", "Raise shield", "Crit Attack", "Cast fire orb", "Drink health potion", "Drink mana potion", "Rest", "Flee".
  - `pub fn moves(class: Class) -> &'static [Move]`, in C++ menu order:
    - Warrior: `[Attack, RaiseShield, HealthPotion, Rest, Flee]`
    - Assassin: `[Attack, CritAttack, HealthPotion, Rest, Flee]`
    - Sorcerer: `[FireOrb, HealthPotion, ManaPotion, Rest, Flee]`
  - `pub fn availability(f: &Fighter, m: Move) -> Result<(), String>`. The `Err` is the dim reason, exactly one of:
    - "needs 10 stamina"
    - "needs 20 stamina"
    - "needs 15 stamina"
    - "needs 10 mana"
    - "no potions left"
    - "no mana potions left"
    - "already at full health"
    - "shield already full"
    - "mana already full"

    For FireOrb, mana is checked before stamina, as in C++.
  - `pub enum Event`, deriving `Clone, Debug, PartialEq`:
    - `Hit { target: usize, absorbed: i32, damage: i32, crit: bool }`
    - `Healed { who: usize, hp: i32 }`
    - `Mana { who: usize, mp: i32 }`
    - `Shield { who: usize, amount: i32 }`
    - `Rested { who: usize, hp: i32, stamina: i32, mana: i32 }`
    - `Fled { who: usize }`
    - `Defeated { who: usize }`
  - `pub struct Resolution { pub actor: usize, pub mv: Move, pub events: Vec<Event>, pub narration: Vec<String> }`, deriving `Clone, Debug`.
  - `pub enum End { Won { winner: usize }, Fled { who: usize } }`, deriving `Clone, Copy, PartialEq, Debug`.
  - `pub struct Duel { pub f: [Fighter; 2], pub turn: usize, pub end: Option<End> }`, deriving `Clone`. Methods:
    - `fn new(a: Fighter, b: Fighter) -> Duel`: `turn` is 0.
    - `fn take_turn(&mut self, m: Move) -> Option<Resolution>`: `None` if the duel is over, the move isn't in `moves(class)`, or it's unavailable. In those cases nothing changes.
    - `fn rematch(&mut self)`: same names and classes, fresh stats, turn 0, end None.

Narration copy (`{n}` is the actor's name, `{t}` the target's). Use these exactly:
- Warrior Attack: "{n} unleashes a devastating slash with their blade, cleaving through with raw power!"
- Assassin Attack: "{n} slips through the guard and lands a swift, clean strike!"
- Crit: "{n} vanishes into the shadows and strikes a vital spot for {d} damage!"
- Fire orb: "{n} conjures a fiery orb of energy and hurls it, exploding on impact!"
- Raise shield: "{n} braces their shield against the next blow. +{x} shield ({s}/{m})."
- Absorbed (added after any hit on a Warrior with shield): "{t}'s shield absorbs {a} damage ({s}/{m} remaining)."
- Health potion: "{n} drinks a healing potion, mending their wounds. +{g} health ({left} left)."
- Mana potion: "{n} drains a shimmering mana potion, arcane energy coursing through them. +{g} mana ({left} left)."
- Rest: "{n} takes a moment to breathe, stepping back and regaining strength as the battle rages on." followed by "+{h} health, +{s} stamina." For a Sorcerer whose mana changed, a third line "+{m} mana."
- Flee:
  - Warrior: "{n} breaks off and flees the fight!"
  - Assassin: "{n} melts into the shadows and flees the fight!"
  - Sorcerer: "{n} melts into a puff of smoke and flees the fight!"
- Defeat (last line): "{t} has been defeated! {n} wins!"

- [ ] **Step 1: Write failing tests** in `rules.rs`:
  - `new_fighter_stats`: each class has health 100/100, stamina 100, damage 10, potions 2. Warrior shield 25/25. Assassin crit 1.5. Sorcerer mana 50/50 and mana potions 3.
  - `warrior_attack_deals_10`: Warrior (0) vs Sorcerer (1). `take_turn(Attack)` gives sorcerer health 90, warrior stamina 90, `turn == 1`. Events are `[Hit{target:1, absorbed:0, damage:10, crit:false}]`. `narration[0] == "A unleashes a devastating slash with their blade, cleaving through with raw power!"`, with the actor named "A".
  - `shield_absorbs_first`: Assassin (0) vs Warrior (1). The Warrior plays Rest on each of its turns; that heals nothing at full health.
    - Crit: `Hit{absorbed:15, damage:0}`, shield 10, health 100.
    - Attack: `Hit{absorbed:10, damage:0}`, shield 0.
    - Crit: `Hit{absorbed:0, damage:15}`, health 85.
  - `crit_costs_20_deals_15`: Assassin stamina 80 after the crit, target health 85, event `crit: true`.
  - `fire_orb_costs_mana_and_stamina`: mana 40, stamina 90, target health 86. With mana set to 5: `availability == Err("needs 10 mana")` and `take_turn` returns `None` with no state change.
  - `raise_shield_rules`: at 25/25, `Err("shield already full")`. After `take_damage(30)`, shield 0 and health 95. Raise shield gives shield 25 and stamina 85. With stamina 10, `Err("needs 15 stamina")`.
  - `health_potion_rules`: at full health, `Err("already at full health")`. After health 30 → potion → 80, potions 1. Health 90 → potion → 100 (gain 10, narration "+10 health (0 left)."). Potions 0 → `Err("no potions left")`.
  - `mana_potion_rules`: at full mana, `Err("mana already full")`. Mana 20 → 50, potions 2. Mana potions 0 → `Err("no mana potions left")`.
  - `rest_rules`: health 95 and stamina 100 → health 100 (+5), stamina 125. Sorcerer mana 45 → 50 (+5), narration line 3 is "+5 mana.". At full mana there is no mana line.
  - `rest_and_flee_always_available`: stamina 0, mana 0, potions 0: every non-Rest/Flee move is `Err` and Rest/Flee are `Ok`.
  - `flee_ends_duel`: `end == Some(End::Fled{who:0})`, a further `take_turn` returns `None`.
  - `knockout`: target health 10, Attack → health 0. `end == Some(End::Won{winner:0})`. The last event is `Defeated{who:1}`. The last narration line is "B has been defeated! A wins!". `turn` does not advance.
  - `move_not_in_class_is_rejected`: a Warrior's `take_turn(FireOrb)` returns `None`.
  - `rematch_resets`: after damage and a turn change, `rematch()` restores full stats, turn 0, end None, same names and classes.
- [ ] **Step 2: Run** `cargo test rules` in `ascii3d/`. Expected: compile errors or FAIL.
- [ ] **Step 3: Implement** `rules.rs` to the interfaces above. `take_damage` uses the shield first if `class == Warrior`, and health clamps at 0.
- [ ] **Step 4: Run** `cargo test rules`. Expected: all pass.
- [ ] **Step 5: Commit** with `git commit -m "ascii3d: add duel rules ported from the C++ game"`.

---

### Task 2: Two fighter bodies in the scene, duel-only binary

The current knight builder moves into `models.rs` as the Warrior. Two Warriors stand facing each other. Walking is removed. Rendering looks as it does today, just with two knights.

**Files:**
- Create: `ascii3d/src/models.rs`
- Modify: `ascii3d/src/scene.rs` (remove `player`, `yaw`, `walk`, `stride`, `hero`, `metal`, `build_hero`, `hero_part`, `to_local`; add `Body`; `sdf` loops bodies; `base_color` and `shade` use the owner body's metal and accent)
- Modify: `ascii3d/src/ascii.rs` (`hero_uv` uses `Scene::body_part`; grass push uses every body's feet; colour is multiplied by `1 - body.dim`)
- Modify: `ascii3d/src/main.rs` (remove WASD and `--walk`; camera framing; one name tag per body)
- Test: in-file tests in `scene.rs`

**Interfaces:**
- Consumes: `rules::Class`.
- Produces:
  - `models.rs`:
    - `#[derive(Clone, Copy, Default)] pub struct Pose { pub root: Vec3, pub lean: f32, pub crouch: f32, pub arm_r: f32, pub arm_l: f32, pub weapon: f32, pub guard: f32, pub fall: f32, pub breath: f32 }`. Every field is a 0-centred amount; `Default` is the neutral stance.
      - `root` is a local offset; +z steps toward the opponent.
      - `arm_r` and `arm_l` swing the arms forward (+) and back.
      - `weapon` raises the weapon or staff.
      - `guard` raises the shield or off hand.
      - `fall` runs from 0 (standing) to 1 (lying on the ground).
      - `breath` is a small vertical bob.
    - `#[derive(Clone, Copy, Default)] pub struct Anchors { pub weapon_tip: Vec3, pub hand_r: Vec3, pub chest: Vec3, pub head: Vec3, pub focus: Vec3 }`, all in local space. `focus` is the staff crystal for the Sorcerer and the chest otherwise.
    - `pub fn build(class: Class, pose: &Pose, id_base: u16, chain_legs: bool) -> (Vec<Part>, Anchors)`. Part ids are `id_base + k` with `k < 40`. In this task Assassin and Sorcerer may return the Warrior model; Task 3 fills them in.
  - `scene.rs`:
    - `pub struct Body { pub class: Class, pub pos: Vec3, pub yaw: f32, pub pose: Pose, pub metal: usize, pub accent: Vec3, pub dim: f32, pub parts: Vec<Part>, pub anchors: Anchors }`. Methods:
      - `fn to_local(&self, world: Vec3) -> Vec3`
      - `fn to_world(&self, local: Vec3) -> Vec3`
    - `pub const ACCENTS: [Vec3; 2]`: blue `(0.25, 0.45, 1.0)` and red `(0.95, 0.25, 0.25)`.
    - `pub const FX_ID: u16 = 90`.
    - `Scene` gains:
      - `pub bodies: Vec<Body>` and `pub fx_parts: Vec<Part>` (world space, ids 90–99, used from Task 6).
      - `pub fn set_fighters(&mut self, classes: [Class; 2])`: places body 0 at `(-1.9, ground, 0)` facing +x and body 1 at `(1.9, ground, 0)` facing -x, with accents from `ACCENTS`.
      - `pub fn update(&mut self, dt: f32)`: time, flicker, rebuilds every body from its pose.
      - `pub fn owner(id: u16) -> Option<usize>`: 10–49 gives 0, 50–89 gives 1, anything else gives None.
      - `pub fn body_part(&self, id: u16) -> Option<(&Body, &Part)>`.
    - `sdf` skips a body whose `dim >= 0.99`. Keep the per-body bounding sphere: radius 1.7 around `pos + (0, 1, 0)`, widened to cover `pose.root` and `fall`.
    - Move the campfire to `(0.3, ground, -2.0)`, behind the fighters from the camera's view, so it lights both and stays in frame. Keep `scene.fire` as the field. Rocks or trees that end up overlapping the duel spot may be moved.
- [ ] **Step 1: Write failing tests** in `scene.rs`:
  - `owner_of_ids`: `owner(10) == Some(0)`, `owner(49) == Some(0)`, `owner(50) == Some(1)`, `owner(89) == Some(1)`, `owner(90) == None`, `owner(101) == None`, `owner(1) == None`.
  - `bodies_have_disjoint_ids`: after `set_fighters([Warrior, Warrior])` and `update(0.016)`, every part id of body 0 has `owner == Some(0)` and every part id of body 1 has `owner == Some(1)`.
  - `local_world_roundtrip`: for body 1, `to_world(to_local(p)) ≈ p` within 1e-4.
  - `bodies_face_each_other`: `body0.to_world(vec3(0,0,1)) - body0.pos` has x > 0.9, and body 1's has x < -0.9.
- [ ] **Step 2: Run** `cargo test scene`. Expected: FAIL.
- [ ] **Step 3: Implement.** Moving `build_hero` into `models::build` must keep the current knight's look. `s` (the walk swing) becomes driven by `pose.arm_r` and `pose.arm_l`, with legs still. Remove the WASD block, `--walk`, `scene.update(dt, mv)`, `fwd` and `right` in `main.rs`. M/N changes `bodies[active].metal`, with `active = 0` until Task 4. The camera targets the midpoint of the two bodies at y≈1.0 with yaw 0, so the eye is on the +z side looking toward -z. Default distance is about 7 and pitch about 0.3. Right-drag orbit and wheel zoom stay. Draw one name tag per body, tinted with its accent. Use the strings "Player 1" and "Player 2" until Task 4 provides names.
- [ ] **Step 4: Run** `cargo test`. Expected: all pass, including rules.
- [ ] **Step 5: Visual check.** Run `cargo run --release -- --shot /tmp/t2.png`, then Read the PNG.
  - Expected: two knights facing each other in the clearing, both outlined, with the fire visible and lighting them.
  - Report the raymarch ms printed by the shot.
- [ ] **Step 6: Commit** with `git commit -m "ascii3d: two fighter bodies, duel-only scene"`.

---

### Task 3: Distinct class models

**Files:**
- Modify: `ascii3d/src/models.rs` (Sorcerer and Assassin builders; shield on the Warrior)
- Modify: `ascii3d/src/scene.rs`:
  - New `Mat` variants `Robe`, `Leather`, `Crystal`, `Orb`.
  - `is_hero` covers `Robe`, `Leather` and `Crystal`. `Orb` is not a hero material.
  - `base_color` and `shade` for the new materials. `Crystal` and `Orb` are emissive, like `Fire`.
- Modify: `ascii3d/src/ascii.rs`:
  - New glyph functions `robe_glyph` and `leather_glyph`, using a surface-space pattern like `cloth_glyph`.
  - `Crystal` and `Orb` use a fire-like emissive ramp and set `glow`.
- Modify: `ascii3d/src/main.rs`:
  - `--duel <class>,<class>` arg. Classes are lowercase names; the default is `warrior,sorcerer`.
  - `--noedges` shot flag for the silhouette check.

**Interfaces:**
- Consumes: `models::build`, `Pose`, `Anchors`, `Scene::set_fighters`.
- Produces: `models::build` returns real models for all three classes, filling every `Anchors` field. `Mat::Orb` (used by Task 6).

Model requirements, from the spec:
- **Sorcerer:**
  - A robe flared to the ground, with no visible legs. An ellipsoid or a stack of capsules is fine.
  - A tall pointed hood or hat that reads as a point in silhouette, and long sleeves.
  - A staff taller than the body, held in the right hand, with a `Crystal` orb on top.
  - Robe colour is deep purple, trimmed with the fighter accent. The pattern is runes or stars in rows.
- **Assassin:**
  - About 10% shorter and slimmer than the Warrior, in a crouched stance (`crouch` baseline > 0).
  - A hood with the face area dark, and a short cloak.
  - Two daggers held low, one in each hand.
  - The `Leather` material is dark brown-grey with a stitched or wrapped pattern. The cloak uses the accent colour.
- **Warrior:**
  - The existing knight plus a shield (a flattened `RBox` or ellipsoid disc) on the left forearm, in the body's metal.
  - The tabard uses the fighter accent instead of fixed blue.

- [ ] **Step 1: Visual check, all three classes.** Run each of these, then Read every PNG:
  - `cargo run --release -- --shot /tmp/t3a.png --duel sorcerer,assassin`
  - `cargo run --release -- --shot /tmp/t3b.png --duel warrior,assassin`
  - `cargo run --release -- --shot /tmp/t3c.png --duel warrior,warrior`

  Expected:
  - Each class is identifiable by shape alone.
  - In the mirror match, the two Warriors differ by tabard accent (blue vs red).
  - Neither fighter is cut off by the frame.
- [ ] **Step 2: Silhouette check.** Run `cargo run --release -- --shot /tmp/t3d.png --duel sorcerer,assassin --noedges`, then Read it. Expected: hat point, staff and robe flare are distinct from the hood, cloak and daggers.
- [ ] **Step 3: Run** `cargo test`. Expected: all pass. `bodies_have_disjoint_ids` still holds with up to 40 parts per body.
- [ ] **Step 4: Commit** with `git commit -m "ascii3d: distinct Warrior, Assassin and Sorcerer models"`.

---

### Task 4: Menus, setup, panels, and a playable duel (no animation yet)

Moves resolve instantly in this task. Task 5 adds the timeline.

**Files:**
- Create: `ascii3d/src/ui.rs`
- Modify: `ascii3d/src/main.rs` (the `Screen` state machine and input routing; replace `draw_ui`)
- Test: in-file tests in `ui.rs`

**Interfaces:**
- Consumes:
  - `rules::{Class, Fighter, Move, Duel, End, Resolution, moves, availability}`
  - `Scene::set_fighters`
  - `Body.metal`
  - `ACCENTS`
- Produces:
  - `pub enum Key { Up, Down, Enter, Digit(u8) }`.
  - `pub struct Menu { pub items: Vec<String>, pub notes: Vec<String>, pub enabled: Vec<bool>, pub sel: usize }`. Methods:
    - `fn new(items: Vec<String>, notes: Vec<String>, enabled: Vec<bool>) -> Menu`: `sel` is the first enabled item. `notes` holds the dim reason or blurb, `""` for none.
    - `fn handle(&mut self, k: Key) -> Option<usize>`: returns the confirmed 0-based index.
  - `pub struct NameField { pub text: String }`. Methods:
    - `fn push(&mut self, c: char)`: accepts `' '..='~'` only, at most 16 chars, no leading space.
    - `fn backspace(&mut self)`.
    - `fn accept(&self) -> Option<String>`: trimmed, `None` if empty.
  - Drawing functions, all centred on the live `screen_width()`:
    - `pub fn draw_menu(font: &Font, m: &Menu, center_x: f32, top: f32)`: highlights the selected row in reverse video. Disabled rows are dimmed and show their note. Rows are prefixed "1 ", "2 ", and so on.
    - `pub fn draw_panels(font: &Font, shown: &[Fighter; 2], active: usize, accents: [Vec3; 2])`: each fighter's panel is on that fighter's screen side. It shows name, class, and bars for health, stamina (bar clamped full above 100, number shows the real value), shield or mana, and potions. The active panel is outlined in its accent.
    - `pub fn draw_caption(font: &Font, lines: &[String])`: centred above the menu area.
    - `pub fn draw_banner(font: &Font, text: &str, rgb: Vec3)`.
- `main.rs` keeps the screen state:

  ```rust
  enum Screen {
      Setup { who: usize, naming: bool, field: NameField, menu: Menu, names: [String; 2], classes: [Class; 2] },
      Choose { menu: Menu },
      Result { menu: Menu },
  }
  ```

  Task 5 adds `Animate`. Alongside it, `duel: Option<Duel>`, `shown: [Fighter; 2]` (the numbers on screen), and `caption: Vec<String>`.
  - Setup flow: P1 name, then P1 class, then P2 name, then P2 class. Then `scene.set_fighters` runs and the screen goes to `Choose`.
    - While a player is browsing the class list, their body previews the highlighted class at their spot.
    - Before P2 picks, P2's spot shows a Warrior dimmed to 0.6.
  - In `Choose`:
    - The menu is `moves(active class)`. Items are `Move::label`, enabled by `availability`, with the `Err` reason as the note.
    - Picking an item calls `duel.take_turn`, sets `shown = duel.f.clone()` and the caption to the narration. It moves to `Result` if `end` is set, otherwise rebuilds the menu for the new active fighter.
  - The `Result` banner reads "{name} wins!" in the winner's accent, or "{name} fled the fight!".
    - Its menu is "Rematch", "New duel", "Quit".
    - Rematch calls `duel.rematch()`. New duel goes back to Setup with empty fields. Quit exits.
  - Esc quits on every screen.
  - Text input uses `get_char_pressed()` while naming. Digits typed into a name go into the name, not the menu.
  - On the `knockout` path, the loser's `dim` stays 0 in this task. Collapse comes in Task 5.
- [ ] **Step 1: Write failing tests** in `ui.rs`:
  - `menu_starts_on_first_enabled`: with enabled `[false, true, true]`, `sel == 1`.
  - `menu_arrows_skip_disabled_and_wrap`: with enabled `[true, false, true]` and sel 0:
    - Down: sel 2.
    - Down again: sel 0 (wrap).
    - Up: sel 2.
  - `menu_digit_confirms_enabled`: `Digit(3)` returns `Some(2)` and `sel == 2`.
  - `menu_digit_ignores_disabled_and_out_of_range`: `Digit(2)` on a disabled item returns `None` with sel unchanged. `Digit(9)` and `Digit(0)` return `None`.
  - `menu_enter_confirms_sel`: Enter returns `Some(sel)`.
  - `name_field_rules`:
    - A leading `' '` is ignored, and `'é'` is ignored.
    - 20 pushes of `'a'` leave 16 chars.
    - Backspace on empty doesn't panic.
    - `accept` on `"   "` (built via `text` directly) returns `None`, and on `" Bo "` returns `Some("Bo")`.
- [ ] **Step 2: Run** `cargo test ui`. Expected: FAIL.
- [ ] **Step 3: Implement** `ui.rs` and the `Screen` machine in `main.rs`. Add `--duel` to the shot so it skips setup with names "Player 1" and "Player 2". Add `--moves 1,1,2`, which plays those 1-based menu choices in turn order before the capture.
- [ ] **Step 4: Run** `cargo test`. Expected: all pass.
- [ ] **Step 5: Visual check.** Run each of these, then Read every PNG:
  - `cargo run --release -- --shot /tmp/t4a.png --duel warrior,sorcerer --moves 1,1`
    - Expected: both panels show the post-move numbers. Warrior health 100, shield 11 (the orb's 14 was all absorbed), stamina 90. Sorcerer health 90, mana 40, stamina 90.
    - Also expected: the caption shows the orb narration, and the menu for Player 1's turn is centred with "Raise shield" enabled.
  - Window size 800×500: add `--size 800x500` (sets `window_width` and `window_height` in `conf`; read the args before `conf` via `std::env::args`), then `cargo run --release -- --shot /tmp/t4b.png --size 800x500 --duel assassin,sorcerer`.
    - Expected: no overlap between panels, menu and caption.
  - Without `--shot`, play a full duel by hand with `cargo run --release`: setup, a few turns, a flee, rematch, new duel. Report anything odd.
- [ ] **Step 6: Commit** with `git commit -m "ascii3d: setup screen, move menus, status panels, playable duel"`.

---

### Task 5: Action timeline and poses

**Files:**
- Create: `ascii3d/src/anim.rs`
- Modify: `ascii3d/src/main.rs` (`Screen::Animate`, displayed-number lag, skip, idle and collapse poses, M/N on the active fighter)
- Test: in-file tests in `anim.rs`

**Interfaces:**
- Consumes: `rules::{Resolution, Move, Event, Class}`, `models::Pose`.
- Produces:
  - `pub struct Action { pub res: Resolution, pub t: f32, pub dur: f32, pub impact: f32, impact_done: bool }`. Methods:
    - `fn new(res: Resolution) -> Action`
    - `fn advance(&mut self, dt: f32) -> bool`: true exactly on the call where `t` first reaches `impact`.
    - `fn skip(&mut self) -> bool`: sets `t = dur`. Returns true if the impact had not fired yet.
    - `fn done(&self) -> bool`
    - `fn phase(&self) -> f32`: `t / dur` clamped to 0–1.
  - Timings (`dur`, `impact`) in seconds:

    | Move | dur | impact |
    |---|---|---|
    | Attack | 1.2 | 0.55 |
    | CritAttack | 1.4 | 0.75 |
    | FireOrb | 1.5 | 1.05 |
    | RaiseShield | 1.0 | 0.45 |
    | HealthPotion and ManaPotion | 1.1 | 0.5 |
    | Rest | 1.1 | 0.5 |
    | Flee | 1.2 | 1.0 |

  - Pose functions:
    - `pub fn actor_pose(class: Class, mv: Move, phase: f32, gap: f32) -> Pose`. `gap` is the distance to the opponent. Melee moves move `root.z` toward the target, stopping about 0.9 m short, then return.
    - `pub fn flinch_pose(since_impact: f32) -> Pose`: a recoil of about 0.25 s.
    - `pub fn idle_pose(time: f32, who: usize) -> Pose`: a breath bob, out of phase per fighter.
    - `pub fn collapse_pose(f: f32) -> Pose`: `fall` eases from 0 to 1.
  - `main.rs` gains `Screen::Animate { action: Action, collapse: f32 }`.
    - Entering it: `shown` keeps the pre-move numbers.
    - On the impact frame (from `advance` or `skip`): `shown = duel.f.clone()` and the caption shows. Task 6 hooks its effects here.
    - When `done()`: if the duel ended in `Won`, the loser collapses over 1.0 s (`collapse` 0 to 1, skippable), then the screen goes to `Result`. If it ended in `Fled`, the fleeing body's `dim` ramps to 1 over the action, then `Result`. Otherwise the screen goes to `Choose` for the next fighter.
    - While animating, only Space, Enter and Esc are read. Number and arrow keys are ignored and not buffered.
  - The Choose screen can no longer be entered mid-action.
- [ ] **Step 1: Write failing tests** in `anim.rs`:
  - `impact_fires_once`: an Attack action advanced in 0.1 s steps to 2.0 s returns true exactly once, and `done()` is then true.
  - `skip_before_impact_reports_impact`: `skip()` at t=0.1 returns true, then `done()`. A later `advance` returns false.
  - `skip_after_impact_does_not_repeat`: after advancing past impact, `skip()` returns false.
  - `timings_per_move`: for each `Move`, `dur` and `impact` match the table, and `impact < dur`.
  - `melee_lunge_returns_home`: `actor_pose(Warrior, Attack, 0.0, 3.8).root` and `actor_pose(..., 1.0, ...)` are within 0.01 of zero. At the impact phase, `root.z` is between 2.5 and 3.0.
  - `collapse_bounds`: `collapse_pose(0.0).fall == 0.0` and `collapse_pose(1.0).fall == 1.0`.
- [ ] **Step 2: Run** `cargo test anim`. Expected: FAIL.
- [ ] **Step 3: Implement** `anim.rs` and wire it into `main.rs`. `models::build` must honour every `Pose` field for all three classes; the knight now swings its sword from `weapon` and `arm_r`. Add `--at <fraction>` to the shot: the last `--moves` entry is captured at that phase instead of being completed.
- [ ] **Step 4: Run** `cargo test`. Expected: all pass.
- [ ] **Step 5: Visual check.** Run each of these, then Read every PNG:
  - `cargo run --release -- --shot /tmp/t5a.png --duel warrior,assassin --moves 1 --at 0.45`
    - Expected: the Warrior lunged, mid-swing, near the Assassin.
  - `--duel sorcerer,warrior --moves 1 --at 0.4`
    - Expected: the staff is raised.
  - `--duel assassin,warrior --moves 5 --at 0.8`
    - Expected: the Assassin is fading, after a flee.
  - By hand: mash number keys during an animation, and check the next player's menu doesn't fire.
- [ ] **Step 6: Commit** with `git commit -m "ascii3d: move timelines, poses, lunge, flinch, collapse"`.

---

### Task 6: Effects

**Files:**
- Create: `ascii3d/src/effects.rs`
- Modify: `ascii3d/src/main.rs` (replace `Ember`, `update_embers` and the ember draw loop with `Fx`; spawn effects from the action; push the fire-orb part into `scene.fx_parts`)
- Test: in-file tests in `effects.rs` for update logic only

**Interfaces:**
- Consumes: `Action`, `Resolution`, `Event`, `Body.anchors`, `Body::to_world`, `Mat::Orb`, `FX_ID`, `Cam::project`, coarse `CellOut` depths.
- Produces:
  - Colour constants `pub const RED, GREEN, CYAN, BLUE, GOLD: Vec3`.
  - `pub enum Kind { Ember, Spark, Trail, Sparkle, Smoke, Mote }`.
  - `pub struct Particle { pub p: Vec3, pub v: Vec3, pub life: f32, pub max: f32, pub kind: Kind, pub glyph: char, pub rgb: Vec3, pub drag: f32, pub lift: f32 }`.
  - `pub struct FloatText { pub p: Vec3, pub text: String, pub rgb: Vec3, pub life: f32, pub max: f32, pub scale: f32 }`.
  - `pub struct Fx { pub particles: Vec<Particle>, pub texts: Vec<FloatText>, pub orb: Option<Vec3>, pub ward: [f32; 2], pub ward_flash: [f32; 2] }`. Methods:
    - `fn update(&mut self, dt: f32)`: ages, moves and removes particles and texts. `ward_flash` decays.
    - `fn campfire(&mut self, fire: Vec3, time: f32, dt: f32)`: the existing ember spawn.
    - `fn burst(&mut self, at: Vec3, n: usize, kind: Kind, rgb: Vec3)`.
    - `fn trail(&mut self, at: Vec3, glyph: char, rgb: Vec3)`.
    - `fn text(&mut self, at: Vec3, s: String, rgb: Vec3, scale: f32)`: the text rises about 0.8 m over 1.2 s and fades.
    - `fn on_action(&mut self, a: &Action, bodies: &[Body], dt: f32, impact_now: bool)`: per-frame effect choreography for the move playing. Spawns trails while the weapon swings, places the orb, and spawns the impact effects on `impact_now`.
  - `pub fn draw(fx: &Fx, cam: &Cam, depth_at: &dyn Fn(f32, f32) -> f32, font: &Font, fs: u16)`:
    - Draws particles and texts depth-tested against the coarse grid, like today's embers.
    - Draws the ward rim: for each fighter with `ward > 0`, about 24 glyphs (`( ) | -` chosen by angle) on a ring of radius 0.75 around the fighter's chest, facing the camera. The glyphs are blue, brightness `0.25 + ward_flash`.
    - The ward is an overlay rim, not an SDF shell, so it never hides the Warrior. This is a builder's call within the spec's "dome or rim".

Per-move choreography (impact effects fire at `impact_now`; full table in the spec):
- **Hit events:**
  - The target always gets a spark burst at `target.anchors.chest`.
  - Damage text "-{damage}" in RED, or "CRIT {damage}" in RED at scale 1.6 when `crit`.
  - When `absorbed > 0`: text "-{absorbed} shield" in BLUE, and `ward_flash[target] = 1`.
- **Warrior Attack:** while phase is 0.3–0.6, a `Trail` glyph each frame at `weapon_tip` world position. The glyph is `/ - \ |`, chosen from the tip's screen-space motion direction, in pale metal colour, fading over 0.3 s.
- **Assassin Attack:** a short trail over phase 0.4–0.5. The impact burst is small.
- **Crit:**
  - Phase 0.1–0.35: the actor `dim` ramps to 0.85 with smoke puffs.
  - Phase 0.35–0.45: the actor's pose `root` jumps to the target's side.
  - Phase 0.45–0.6: `dim` returns to 0.
  - At impact: a large RED spark burst. After the action the actor is home again.
- **Fire orb:**
  - Phase 0.2–0.4: a glow burst at `focus`.
  - Phase 0.4–0.7: `orb` travels from the actor's `focus` to the target's chest on a slight arc. `scene.fx_parts` holds a `Mat::Orb` sphere (r 0.16, id `FX_ID`) at the orb's position, and `Ember` particles trail behind it.
  - At impact: `orb = None`, plus 40 embers and a flash burst.
- **Raise shield:** at impact, `ward[actor] = 1` and `ward_flash[actor] = 1`, plus BLUE sparkles. `ward[i]` is kept equal to `shown[i].shield > 0`, so it disappears when the shield breaks.
- **Potions:** GREEN (health) or CYAN (mana) sparkles rise from the feet, using `✧ ✦ * .`. Text "+{n} HP" or "+{n} MP".
- **Rest:** GOLD motes. Text "+{h} HP +{s} ST", plus " +{m} MP" for the Sorcerer when mana changed.
- **Flee:** smoke puffs while `dim` ramps up.
- **Knockout:** a small dust burst at the feet when `collapse` reaches 1.
- [ ] **Step 1: Write failing tests** in `effects.rs`:
  - `particles_expire`: after a `burst` of 10 with `max <= 2.0`, `update(0.1)` called 25 times leaves it empty.
  - `text_rises_and_expires`: after `text(...)` and `update(0.6)`, `p.y` has increased. After 1.3 s total it is removed.
  - `ward_flash_decays`: with `ward_flash[0] = 1`, after `update(1.0)` it is below 0.1 and not negative.
- [ ] **Step 2: Run** `cargo test effects`. Expected: FAIL.
- [ ] **Step 3: Implement** `effects.rs` and wire it in. Remove `Ember` and `update_embers` from `main.rs`; the campfire now runs through `Fx::campfire`. Hero glow for `Crystal` uses the existing fine-pass halo code.
- [ ] **Step 4: Run** `cargo test`. Expected: all pass.
- [ ] **Step 5: Visual check.** Run each of these, then Read every PNG:
  - `--duel warrior,assassin --moves 1 --at 0.5`: slash trail visible.
  - `--duel sorcerer,warrior --moves 1 --at 0.6`: orb mid-flight with an ember trail.
  - `--duel sorcerer,warrior --moves 1 --at 0.72`: explosion, plus "-14 shield" in blue and a ward flash. At 25 shield, all 14 are absorbed.
  - `--duel assassin,warrior --moves 2 --at 0.3`: the Assassin is dim, with smoke.
  - `--duel assassin,warrior --moves 2,4,2,3 --at 0.6`: green sparkles and "+5 HP". The sequence is crit, Warrior rests, crit (10 absorbed, 5 to health), Warrior drinks a health potion.
  - `--duel warrior,assassin --moves 2`: Raise shield is dimmed at full, so the shot must print "move 2 unavailable" and exit non-zero. Make the shot do that for any unavailable `--moves` entry.
- [ ] **Step 6: Commit** with `git commit -m "ascii3d: glyph effects for every move"`.

---

### Task 7: Fine-grid fallback, README, final check

**Files:**
- Modify: `ascii3d/src/ascii.rs`:
  - `Opts` gains `pub fine_mask: [bool; 2]`.
  - Coarse cells defer only if a hero sub-sample's `Scene::owner` is in the mask.
  - The fine pass keeps only hero materials of masked bodies.
- Modify: `ascii3d/src/main.rs`:
  - Track a moving average of raymarch ms over 30 frames.
  - If it is above 25 ms, the mask becomes `[active == 0, active == 1]`. If it is below 18 ms for 60 frames, it returns to `[true, true]`.
  - F still toggles fine entirely.
  - The top info line shows `fine both|active|off`.
- Modify: `ascii3d/README.md`:
  - Replace the walk controls with the duel flow and controls: arrows, Enter, digits, Space or Enter to skip, M/N metal on your Warrior's turn, E, F, G, RMB orbit, wheel zoom, Esc.
  - Document `--shot`, `--duel`, `--moves`, `--at`, `--size` and `--noedges`.
- [ ] **Step 1: Run** `cargo test`. Expected: all pass.
- [ ] **Step 2: Run** `cargo clippy --release 2>&1 | tail -20`. Expected: no errors. Fix warnings in new code.
- [ ] **Step 3: Visual check.** Run `cargo run --release -- --shot /tmp/t7.png --duel sorcerer,assassin`, then Read the PNG.
  - Expected: both fighters on the fine grid (half-size glyphs) and the info line shows `fine both`.
  - Report the raymarch ms.
- [ ] **Step 4: By hand.** Run `cargo run --release` and play one full duel per class pairing you haven't seen yet (at least warrior/sorcerer, assassin/assassin). Note the fps shown at the default glyph size and confirm it is about 30 or better.
- [ ] **Step 5: Commit** with `git commit -m "ascii3d: adaptive fine grid, README for the duel"`.
