# Clash of Valor 3D Round 3 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the audit findings: endless duels, first-mover edge, unclear menu, one-key quit, see-through fighters, invisible weapons. Also add a bot opponent, a move log with score, an animation-speed option, and an idle camera sway.

**Architecture:**
- Rules changes stay in `rules.rs`.
- A new `bot.rs` holds the move-choice heuristics and a seeded RNG.
- `flow.rs` gains a mode, a pop-up layer, score and log, bot turns, and speed. It stays pure and is tested.
- `ui.rs` gains the menu header and cost column, the block font, and the pop-up, log and result drawing.
- Visual changes go in `ascii.rs`, `scene.rs`, `models.rs`, `effects.rs` and `main.rs`, as in the previous passes.

**Tech Stack:** Rust 2024, macroquad 0.4, rayon. Run `cargo test` in `ascii3d/`.

**Spec:** `docs/superpowers/specs/2026-11-26-ascii3d-round3-design.md`

## Global Constraints

- Camera distance never changes. Sway and shake move yaw or the shared offset only.
- Menus follow `CLAUDE.md`: centred, reverse-video selection, arrows plus number keys, no `>` prompt. Pop-up menus follow the same rules.
- No single key may quit the game.
- Raymarch median (`bench.sh --duel warrior,sorcerer`, 5 shots) must stay within noise of about 20 ms. Glyph-draw additions get measured by fps in a shot.
- Shots go to the session scratchpad.
- `rules.rs` constants:
  - `TURN_LIMIT = 40` (per fighter).
  - `SPELL_BONUS = 2`.

## Review Focus

1. **Pop-up isolation:** while a pop-up is open, no input reaches the screen beneath, and closing it restores that screen unchanged. This includes a pop-up opened mid-animation: the animation pauses too.
2. **Bot turn edge cases:**
   - The bot never acts during a human's turn.
   - The bot's move goes through the same availability check.
   - The bot can't play twice when skips, the menu grace and the think delay interact.
   - A knockout on the bot's move still reaches the result screen.
3. **Time-up:**
   - The 80th move ends the duel even if it is a Flee or a knockout, in which case that end takes precedence.
   - Draw handling reaches the result screen with no winner.
   - The score is unchanged on a draw.
4. **Rematch alternation:** after a rematch, the first mover's menu appears and `turn == first`. With a bot, if the bot moves first it acts on its own.
5. **Small window:** below 640×400, nothing panics and no negative sizes reach the draw calls.

---

### Task 1: Rules (turn limit, first mover, orb 12, damage line, stats, log line)

**Files:** `rules.rs` and its tests.

**Interfaces:**
- `pub const TURN_LIMIT: i32 = 40;`
- `End` gains `TimeUp { winner: Option<usize> }`.
- `Duel` gains:
  - `pub first: usize`
  - `pub moves: [i32; 2]`: turns taken per fighter.
  - `pub dealt: [i32; 2]`: health lost plus shield absorbed, dealt by each fighter.
- `Duel::turn_number(&self) -> i32`: the active fighter's `moves + 1`.
- `rematch()` flips `first` and sets `turn = first`. It resets `moves` and `dealt`.
- `take_turn` behaviour:
  - Increments `moves[actor]` and adds to `dealt[actor]`.
  - After a hit with health lost, adds the narration line `"{t} takes {n} damage ({hp}/{max})."`.
  - After the move, if `end` is still `None` and both fighters have `moves >= TURN_LIMIT`, it sets `TimeUp`. The winner is compared on `health * 100 / max_health`. It adds "The duel reaches its time limit." and then either "{name} wins on health!" or "It's a draw!".
- `pub fn log_line(res: &Resolution, duel: &Duel) -> String`. Formats (`{a}` actor, `{t}` target):
  - Hit: `"{a} · {Move label} → {t} -{damage}"`, plus `" ({absorbed} shield)"` when `absorbed > 0`, plus `" CRIT"` when `crit`. When `damage == 0`, the text is `"→ {t} blocked {absorbed}"`.
  - Healed: `"{a} · {label} +{hp} HP"`.
  - Mana: `"+{mp} MP"`.
  - Shield: `"+{amount} shield"`.
  - Rested: `"{a} · Rest"`, then `+{hp} HP`, `+{st} ST`, `+{mp} MP`, omitting zero terms.
  - Fled: `"{a} fled"`.

- [ ] **Step 1: Failing tests** (update existing ones that use 14 to use 12):
  - `fire_orb_deals_12`.
  - `damage_line_after_health_loss`: an attack on a Sorcerer shows "B takes 10 damage (90/100)."; a fully absorbed hit has no such line.
  - `time_up_winner_on_health_percent`: a loop of Rest/Rest for 80 moves after setting health A = 50, B = 60. End is `TimeUp { winner: Some(1) }` and the narration has "B wins on health!".
  - `time_up_draw`: equal percentages give `TimeUp { winner: None }` and "It's a draw!".
  - `knockout_on_last_move_wins`: if move 80 is a knockout, the end is `Won`.
  - `rematch_alternates_first`: after `rematch()`, `first == 1` and `turn == 1`. After a second rematch, both are 0.
  - `stats_track_moves_and_dealt`.
  - `log_lines`: one case per event kind.
- [ ] **Step 2:** Run `cargo test rules`. Expect FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run `cargo test`. Expect PASS.
- [ ] **Step 5:** Commit "ascii3d: turn limit, alternating first move, orb 12, duel stats".

### Task 2: Bot

**Files:** create `bot.rs`, add `mod bot;` in main.rs.

**Interfaces:**
- `pub struct Rng(u64)`:
  - `fn new(seed: u64) -> Rng`
  - `fn next_f32(&mut self) -> f32` (xorshift64*)
  - `fn below(&mut self, n: u32) -> u32`
- `pub fn choose(f: &Fighter, rng: &mut Rng) -> Move`: the heuristics from spec A.11. The result must pass `availability`. Falls back to Rest.
- `pub const THINK: f32 = 0.7;`

- [ ] **Step 1: Failing tests:**
  - `bot_heals_when_low`: health 30 with potions gives HealthPotion for each class.
  - `bot_never_picks_unavailable`: over 2000 random fighter states and seeds, `availability(choose(..))` is Ok.
  - `bot_rests_when_broke`: stamina 0 gives Rest.
  - `bot_deterministic`: the same seed gives the same sequence.
  - `sorcerer_drinks_mana_when_dry`: mana 5 with mana potions gives ManaPotion.
- [ ] **Step 2:** Implement, run `cargo test`, commit "ascii3d: bot opponent heuristics".

### Task 3: Flow (mode, pop-ups, Esc back, bot turns, speed, score, log)

**Files:** `flow.rs` and its tests.

**Interfaces:**
- `pub enum Mode { Pvp, Bot }`.
- `Screen::Setup` gains a mode step: `stage: SetupStage`, where `enum SetupStage { Mode, Name, Class }`.
  - In Bot mode, after Player 1's class, the class menu for player 2 is titled "Choose the bot's class" with 4 items (Warrior, Assassin, Sorcerer, Random). The name is "Bot".
  - Random uses `Rng`.
- `pub enum Popup { Pause { menu: Menu }, HowTo, ConfirmFlee { menu: Menu } }` and `Flow.popup: Option<Popup>`.
  - `step()` routes all input to the pop-up while it is set.
  - `update()` does nothing to the animation while a pop-up is open.
  - Pause items: Resume, How to play, "Animations: Normal"/"Animations: Fast" (toggles in place, keeps the selection), New duel, Quit.
  - In setup, pause has only Resume and Quit.
- Esc rules:
  - During a duel or result: Esc opens Pause; with Pause open, Esc closes it.
  - In setup, Name and Class: Esc goes back one step.
  - In setup, Mode: Esc opens setup Pause.
  - With HowTo or ConfirmFlee open: Esc closes the pop-up.
  - Nothing sets `quit` except the Quit items.
- Choosing Flee opens ConfirmFlee, defaulting to Stay. Flee confirmed plays the flee move.
- `Flow.fast: bool`. `update(dt)` scales `dt` by 2 for Animate when `fast`.
- Bot turns: while the screen is `Choose` and `mode == Bot` and `duel.turn == 1`:
  - `update` accumulates `bot_wait`.
  - When `bot_wait >= THINK`, it plays `bot::choose`.
  - Human input to the menu is ignored on the bot's turn.
- `Flow.score: [u32; 2]`: a win adds 1 to the winner on the transition to Result. New duel resets it. Rematch keeps it.
- `Flow.log: Vec<String>` holds the last 4 `log_line`s, cleared on a new duel or rematch.
- New duel pre-fills the name fields with the old names. Bot mode keeps the mode.
- `pub fn banner(&self)` covers TimeUp: "Time! {name} wins on health" or "Draw!".
- `pub fn header(&self) -> Option<(String, usize)>`: "{name}'s move · Turn N/40", or "Bot is thinking…" on the bot's turn.
- M/N target: `active()` during setup returns the player currently choosing.

- [ ] **Step 1: Failing tests:**
  - `mode_then_names_pvp`, `bot_mode_setup` (P1 name, class, bot class menu has 4 items, bot named "Bot").
  - `esc_backs_out_of_setup`.
  - `esc_opens_pause_and_closes`.
  - `pause_blocks_input_and_animation`: open Pause mid-animation, update 2 s; the action `t` is unchanged and the menu digits don't pick moves.
  - `pause_toggles_speed`.
  - `fast_halves_animation`.
  - `howto_any_key_closes`.
  - `flee_needs_confirm`: Digit(5) opens the confirm; Enter (Stay) closes it with no change; Digit(5) then Digit(2) flees.
  - `quit_only_from_menu_items`.
  - `bot_plays_after_think`: on the bot's turn, nothing happens before 0.7 s; after it, an Animate starts.
  - `human_keys_ignored_on_bot_turn`.
  - `bot_first_after_rematch_acts`.
  - `score_counts_wins_and_resets_on_new_duel`.
  - `log_keeps_last_four`.
  - `new_duel_prefills_names`.
  - `time_up_banner_and_draw`.
  - `header_text`.
- [ ] **Step 2:** Implement. Update the existing tests to the new setup start: the mode step comes first, so `typed()` needs a `Digit(1)` before it.
- [ ] **Step 3:** Run `cargo test`.
- [ ] **Step 4:** Commit "ascii3d: bot mode, pause/help/confirm pop-ups, Esc back, speed, score, log".

### Task 4: UI (menu header and costs, help bar, panels, tags, pop-ups, log, small window, F3, backspace repeat)

**Files:** `ui.rs`, `main.rs`, `flow.rs` (move menu builds reasons and costs).

**Interfaces:**
- `Menu.reasons: Vec<String>`: a dimmed row shows its reason in `(0.85, 0.45, 0.4)`. `notes` holds the description or cost.
- `pub fn move_cost(f: &Fighter, m: Move) -> String`: the exact strings from spec B.1.
- `Menu.flash: f32`, plus `fn flash_row(&mut self, i)`, which sets `flash` for 0.4 s on dimmed-row attempts. The flow calls it, and `update` decays it.
- `pub fn hp_color(frac: f32, time: f32) -> Vec3`: green, gold or red at the spec thresholds, pulsing below 0.15.
- `draw_menu_header(font, text, accent, cx, y)`.
- `draw_popup(font, popup, flow)`: a centred framed plate, with a backdrop dimming the scene to 55%. HowTo text is static.
- `draw_log(font, lines, x, y, w)`.
- The help bar text is chosen per screen, as in spec B.5.
- F3 toggles the info line. It defaults to hidden.
- Name tags clamp to the screen and sit below the panels. The tag anchor is `pos + root + (0, 2.5, 0)`, upright.
- Floating text spawns at tag height + 0.35 m (`effects.rs`).
- Below 640×400: draw only the "Make the window larger" plate. Skip the panels, menu and caption.
- Backspace repeat in `read_inputs`: hold for 0.4 s, then repeat every 0.05 s.

- [ ] **Step 1: Failing tests:**
  - `move_cost_strings`: every row from spec B.1 for all classes, including potion counts and the Sorcerer's Rest.
  - `hp_color_thresholds`.
  - `menu_flash_decays`.
  - Flow test `dimmed_press_flashes`.
- [ ] **Step 2:** Implement.
- [ ] **Step 3:** Run `cargo test`.
- [ ] **Step 4: Shots** (expect header, costs, help bar, log, pause pop-up, flee confirm, no clipping):
  - Duel menu at 1280 and 800.
  - Pause pop-up: `--keys` with Esc.

  Scripted keys need an Esc token: add `~` → Esc to the `--keys` map and document it.
- [ ] **Step 5:** Commit "ascii3d: menu header and costs, pop-ups, log, panels, help bar, small-window guard".

### Task 5: Setup and result screens (block title, framed prompts, class menu at bottom, block banner, score/stats)

**Files:** `ui.rs` (block font), `main.rs`.

**Interfaces:**
- `pub fn block_text(s: &str) -> [String; 3]`: a 3-row block font using `█▀▄ ` for A–Z, space and `!`. Each letter is 3 columns wide plus a 1-column gap.
- `draw_title(font, cx, y)` draws "CLASH OF VALOR" via `block_text` in a warm gold.
- The setup prompt plate is framed and backed. The class menu sits at the bottom, like the move menu.
- The result screen:
  - Dims the scene to 55%.
  - Banner: `▄` band, then text (fitted), then `▀` band, in the winner's accent, or grey for a draw.
  - Below: the score line, the stats line ("12 turns · Ann dealt 96 · Bo dealt 70"), and "{name} moves first next".

- [ ] **Step 1: Failing tests:**
  - `block_text_rows_equal_width`: for "CLASH OF VALOR", the 3 rows are equal width, `4 * chars - 1` columns.
  - `block_text_known_letter`: the rows for "A" match.
- [ ] **Step 2:** Implement.
- [ ] **Step 3: Shots:** setup with the mode menu, setup with the class menu showing the preview, KO result at 1280 and 800, draw result. A draw result needs a shot path: add an `--ff N` flag that sets both fighters' `moves` to N before `--moves`. This is a test aid; document it in the README.
- [ ] **Step 4:** Commit "ascii3d: block-glyph title and result banner, framed setup, score on result".

### Task 6: Visual: solid fighters, camera angle, lighting, fire placement

**Files:** `main.rs` (fine-cell backing, pitch and target), `ascii.rs` (MOON), `scene.rs` (ACCENTS, warm rim, fire position), `effects.rs` / `ascii.rs` (orb core colour).

- [ ] **Step 1: Baseline shots and bench.**
- [ ] **Step 2: Implement:**
  - `█` backing at 0.3× under fine cells that are not emissive.
  - Pitch 0.16, target 1.25.
  - Move the moon, checking in a shot that it clears the trees and panels.
  - Accent P1 `(0.35, 0.65, 1.0)`.
  - Warm rim term for hero materials: `+ base * vec3(0.5, 0.32, 0.18) * rim^2 * fire_dir_factor`.
  - Fire at about `(-0.6, z=-3.0)`.
  - Orb core shade goes white-yellow.

  There's no new pure logic. The existing tests must stay green. Add one test, `fire_off_action_line`: `|fire.x - centre.x| > 0.4` or `fire.z < -2.5`.
- [ ] **Step 3: Shots:** idle for each pairing, orb at 0.6, 800×500. Bench.
- [ ] **Step 4:** Commit "ascii3d: solid fighter backing, lower camera, balanced lighting, fire off the action line".

### Task 7: Visual: weapons, slash trail, hit-stop freeze for target and trails

**Files:** `ascii.rs` (Blade glyphs, glow), `models.rs` (blade radii ×1.5), `effects.rs` (trail samples, glyph, colour fade, no spawn during hold), `flow.rs` (`since_impact` doesn't advance while `action.hold > 0`).

- [ ] **Step 1: Failing tests:**
  - `no_trail_during_hold`: Attack with Hit; advance into the hold; on_action calls add no Trail particles.
  - `flinch_frozen_during_hold`: Flow `since_impact` unchanged across `update` while held.
  - `trail_samples_along_blade`: an Attack frame in the swing window adds ≥3 Trail particles.
- [ ] **Step 2:** Implement. Run tests.
- [ ] **Step 3: Shots:** slash at 0.42 and 0.45, crit at 0.56. Bench.
- [ ] **Step 4:** Commit "ascii3d: visible weapons and slash arcs, hit-stop freezes target and trails".

### Task 8: Visual: grass, sky, stone/leaf/chain glyphs

**Files:** `ascii.rs`.

- [ ] **Step 1: Failing tests:**
  - `stars_one_cell_per_patch`: for a star patch, sampling a grid of directions 4× finer than the patch, `Some` results all fall within one representative direction ± half a cell. Implement as: a patch yields a star only if the sampled direction is within `CELL/2` of the patch's representative point. The test checks that the lit fraction is under 1/9 of the patch area.
  - `flowers_rare_and_warm`: helper `flower_glyph(h) -> Option<(char, Vec3)>` returns `None` for `h <= 0.994`, and its colour has red > blue.
- [ ] **Step 2: Implement:**
  - Stars via representative points, plus halo, treeline and hill patches of at least one cell.
  - Moon `●` core with a `○`/`°` rim.
  - Grass ×0.6 and near-row darkening by `hit.t` (the camera distance to the grass point).
  - Stone `░▒▓` with cracks.
  - Leaf `░▒` plus `'`/`"`.
  - Chainmail `▒░` alternation.
- [ ] **Step 3:** Run tests, then shots: idle, an orbit sequence `--yaw 0/0.5/1` to check the shimmer.
- [ ] **Step 4:** Commit "ascii3d: calmer grass, stable stars, glyph-art moon, stone, leaves, chainmail".

### Task 9: Visual: small effects and idle sway

**Files:** `effects.rs`, `main.rs`, `flow.rs`.

**Interfaces:**
- Shield rim: 40 strokes, `▌▐` at the sides. Alpha is multiplied by `(1 - dim)` (pass dims into `draw`).
- Rest text omits zero terms.
- Crit: at phase 0.36, spawn 12 `░` afterimage particles of kind `Smoke` in the accent colour at the origin.
- `PANEL_BG` alpha 0.9.
- Absorbed crit: text "CRIT" at scale 1.0 next to "-N shield".
- KO dim: `body_state` sets the loser's dim to `0.4 * collapse`, held at 0.4 on Result. The shadow still shows because `dim < 0.99`.
- `pub struct Sway { pub t: f32 }`:
  - `fn yaw_offset(&self) -> f32` returns `(4°).to_radians() * sin(2π t / 20)`.
  - `fn update(&mut self, dt, active: bool)`: `t` advances only while active.
  - `main.rs` sets active when the screen is Choose or Result with no pop-up and the right mouse button isn't held.
  - It adds `yaw_offset` to the yaw used for the eye; `cam_yaw` itself isn't changed.

- [ ] **Step 1: Failing tests:**
  - `sway_bounded_and_keeps_distance`: `|yaw_offset| ≤ 4°+ε`; the eye computed with the offset has the same distance.
  - `sway_pauses_when_inactive`.
  - `rest_text_omits_zeros` (update the existing test that expects "+0 HP +25 ST").
  - `ko_dims_loser`.
  - `absorbed_crit_shows_tag`.
- [ ] **Step 2:** Implement. Run tests.
- [ ] **Step 3: Shots:** shield rim after a raise, crit at 0.4, KO result. Bench.
- [ ] **Step 4:** Update `ascii3d/README.md`: modes, bot, pause/help, Esc behaviour, F3, `~` key token, `--ff`, turn limit.
- [ ] **Step 5:** Commit "ascii3d: shield rim, crit afterimage, KO dim, idle camera sway, README".
