# Clash of Valor 3D: round 3 (rules, bot, UI, visuals)

Date: 2026-11-26
Branch: `ascii-3d`
Follows: `2026-11-25-ascii3d-duel-design.md`, `2026-11-25-ascii3d-visuals-design.md`

## Intent

Two audits of the duel found these problems:

- **Functionality and UI (Opus):**
  - A duel can run forever, and the first mover has an advantage.
  - The Sorcerer is the strongest class.
  - The menu never says whose turn it is or what a move costs.
  - Esc quits instantly and Flee has no confirm.
  - There is no help, no history, and the setup screen is awkward.
- **Visuals (Fable 5.2):**
  - Fighters look see-through and the camera looks down too much.
  - Weapons and the slash are invisible.
  - The left fighter is darker than the right.
  - The fire sits on the action line.
  - Sky, rocks and trees read as letters, and the banner is thin text.

The user asked for all of these to be fixed, plus four extras: a bot opponent, a move log with a score, an animation-speed option, and an idle camera sway.

## Decisions (the user's, and defaults the user accepted)

- **Turn limit:** 40 turns per fighter, 80 moves in all. When the limit is reached, the fighter with the higher health percentage wins. If both have the same percentage, the duel is a draw.
- **Rematch:** the first move alternates between the players each rematch.
- **Balance:** the C++ numbers stay, with one change: fire orb damage goes from 14 to 12 (`SPELL_BONUS` 4 → 2).
- **Extras:** bot opponent, move log and score, animation-speed option, idle camera sway.
- **Standing rules still hold:**
  - The camera distance never changes.
  - Menus are centred, use reverse video, arrows plus number keys, and have no `>` prompt.
  - Block glyphs are preferred for banner art.

## Part A: rules and flow

1. **Turn limit.**
   - `Duel` counts moves. After each move, if 40 turns per fighter have been played and nobody has won, the duel ends with `End::TimeUp { winner: Option<usize> }`.
   - The winner is the fighter with the higher `health * 100 / max_health`, using integer maths. Equal values give `None`, a draw.
   - Narration adds "The duel reaches its time limit." and then either "{name} wins on health!" or "It's a draw!".
   - The turn number is shown as "Turn N/40", where N is the active fighter's turn count.
2. **First mover alternates.**
   - `Duel` gains `first: usize`. `new` sets it to 0, and `rematch` flips it and sets `turn = first`.
   - The result screen shows who moves first next time.
3. **Fire orb deals 12.**
4. **Damage line.** After any hit that costs health, the narration adds "{target} takes {n} damage ({hp}/{max})."
5. **Score and stats.**
   - Each player has a win tally, kept across rematches and reset by New duel. Draws and flees add nothing.
   - Each duel tracks turns played and total damage dealt by each fighter, counting health lost plus shield absorbed.
6. **Move log.**
   - The last four moves are kept as one-line summaries, for example "Ann · Attack → Bo -10", "Bo · Rest +10 HP +25 ST", "Ann · Fire orb → Bo -6 (6 shield)".
   - The log is shown at the side below the status panels, at a 12 px font. It is hidden when the window is shorter than 600 px.
7. **Pop-up layer.** `Flow` gains an optional pop-up drawn over any screen. While a pop-up is open it receives all input, and the screen beneath does not change. Pop-ups:
   - **Pause:**
     - Items: Resume, How to play, Animations: Normal/Fast, New duel, Quit.
     - The animations item toggles in place.
     - Opened with Esc during a duel or on the result screen. Esc closes it again.
   - **How to play:** a static page covering the goal, the three classes, every move with its cost and effect, the turn limit, and the keys. Any key closes it.
   - **Confirm Flee:** "Flee the duel? This ends it." with two items, Stay and Flee. The default is Stay, and 1/2 jump-confirm as usual.
8. **Esc in setup goes back one step.**
   - Class goes back to name. Player 2's name goes back to Player 1's class.
   - On the mode screen (the first step), Esc opens a small pause with Resume and Quit.
   - Nothing quits the game on a single key.
9. **New duel keeps the names** as pre-filled text in the name fields.
10. **Animation speed.**
    - Fast plays move animations, the collapse and the hit-stop at twice the speed.
    - The setting lasts for the session and is not saved.
11. **Bot opponent.**
    - Setup now starts with a mode menu: "Player vs Player" or "Player vs Bot".
    - In Player vs Bot, Player 1 enters a name and class. Then "Choose the bot's class" offers Warrior, Assassin, Sorcerer and Random.
    - The bot is named "Bot" and plays as Player 2.
    - On its turn, the bot waits about 0.7 s, then picks a move by the C++ `battleMenuBot` heuristics:
      - **Warrior:** drink a potion if HP is under 1/3 and potions remain; else raise the shield if it is at 0 and stamina allows, with a 1-in-3 chance; else Attack if stamina allows; else Rest.
      - **Assassin:** potion, as for the Warrior; else Crit if stamina allows, with a 1-in-2 chance; else Attack; else Rest.
      - **Sorcerer:** potion; else a mana potion if mana is under 10 and potions remain; else Fire orb if mana and stamina allow; else Rest.
    - The bot only picks available moves. Randomness uses a seeded RNG so tests are deterministic.
    - The bot's menu is shown but not interactive while it thinks. The header reads "Bot is thinking…".
    - The bot never flees.

## Part B: UI

1. **Move menu.**
   - A header in the active player's colour: "{name}'s move · Turn N/40".
   - Each row has a cost and effect column:

     | Move | Column |
     |---|---|
     | Attack | `10 ST · 10 dmg` |
     | Raise shield | `15 ST · +25 shield` |
     | Crit Attack | `20 ST · 15 dmg` |
     | Fire orb | `10 MP 10 ST · 12 dmg` |
     | Health potion | `+50 HP · {n} left` |
     | Mana potion | `+50 MP · {n} left` |
     | Rest | `+10 HP +25 ST` (`+20 MP` for the Sorcerer) |
     | Flee | `ends the duel` |

   - A dimmed row shows its reason instead, in a muted red.
   - `Menu` gains a separate `reasons` field, split from `notes`.
2. **Feedback for a dimmed move.** Pressing a dimmed row's number or Enter flashes that row and its reason for 0.4 s.
3. **Setup layout.**
   - A block-glyph "CLASH OF VALOR" title at the top, built from `█▀▄` with a small built-in block font.
   - The prompts sit on a backed, framed plate.
   - The class menu sits at the bottom of the screen, like the move menu, so the 3D preview stays visible.
4. **Developer readout.** The fps, raymarch and grid line is hidden by default and toggled with F3. The title alone is never shown in its place.
5. **Help bar per screen**, listing only the keys that apply:
   - Name: `Type a name · Enter confirm · Esc back`.
   - Class or mode: `↑↓ or 1-n choose · Enter confirm · Esc back`.
   - Duel: `↑↓ or 1-5 choose · Enter confirm · Space skip · Esc pause`, plus `· M/N armour` when the active fighter is a Warrior.
   - Result: `↑↓ or 1-3 choose · Enter confirm · Esc pause`.
   - E/F/G/L/F3 still work but are documented only in the README.
6. **Status panels.**
   - The Assassin's third row shows "Crit x1.5" where the bar would be, so there is no empty row.
   - The HP bar is green above 50%, gold from 25–50% and red below 25%, and it pulses below 15%.
7. **Name tags.**
   - Tags are placed above the body's upright position (`pos + root`), not tilted with a fall.
   - Tags are clamped inside the window and kept below the status panels.
   - Floating numbers start 0.35 m above the tag height so they don't overlap it.
8. **Result screen.**
   - A block-glyph banner: the lettering keeps its font, with a band of `▄▄▄`/`▀▀▀` in the winner's colour above and below. The scene is dimmed to about 55% behind it.
   - Below the banner: the score ("Ann 2 – 1 Bo"), the duel stats (turns, damage dealt), and "{name} moves first next".
   - The banner text is fitted to the window.
9. **Small windows.** Below 640×400 the game draws only a centred "Make the window larger" plate. Layout never computes negative sizes.

## Part C: visuals

1. **Solid fighters.**
   - Each fine-grid fighter cell draws a `█` in the cell colour at 0.3× brightness under its glyph.
   - Emissive materials (Crystal, Fire, Orb) skip the backing.
   - Fable tested this with rectangles, and it worked.
2. **Camera angle.**
   - Default pitch 0.3 → 0.16, and look-target height 1.45 → 1.25. The distance stays 5.4.
   - The moon moves so it clears trees and panels (Fable's start point is about `(0.3, 0.2)` before normalising; tune in shots).
   - Re-check that the ridge and treeline sit below head height.
3. **Weapons and slash.**
   - Blade glyphs use `█▓▒` with a luminance floor of 0.75 and glow 0.3.
   - Blade radius grows about 1.5×.
   - Slash trails spawn 3–4 samples along the blade per frame and use the directional glyph. They draw at full coarse size and fade from white to the player's accent.
   - No trail spawns while the hit-stop holds, which fixes the stacked dashes.
4. **Lighting balance.**
   - Player 1's accent becomes `(0.35, 0.65, 1.0)`.
   - Hero materials get a warm, camera-side rim term so both fighters read equally.
   - The campfire moves off the action line, to about `(-0.6, z=-3.0)`, and is retuned in shots.
   - The orb core becomes white-yellow so it separates from the fire.
5. **Grass.**
   - Flowers become rarer (`h > 0.994`) and use `·`/`°` in a warm tint, so they no longer look like stars.
   - Grass brightness is cut about 0.6×.
   - The nearest rows darken by camera depth.
6. **Sky and environment as glyph art.**
   - The moon gets a `●` core with a `○`/`°` rim.
   - Stars pick one representative point per star patch and draw only in the cell that contains it, which stops the shimmer.
   - Halo, treeline and hill texture patches are at least one cell wide.
   - Stone uses `░▒▓` with cracks.
   - Leaves use `░▒` shading without `*`, so `*` stays reserved for sparks.
7. **Small effects fixes.**
   - The shield rim has about 40 strokes, with `▌▐│` on the sides, and fades with the body's dim.
   - Rest text omits zero terms.
   - The crit leaves an accent afterimage of `░` at its origin.
   - `PANEL_BG` alpha goes up to about 0.9.
   - Chainmail uses `▒░`/`o°` instead of `S8S3`.
   - A crit fully absorbed by the shield shows a small "CRIT" tag next to "-N shield".
   - A knocked-out fighter's glyphs dim to about 0.4.
   - The hit-stop also freezes the target's flinch clock.
8. **Idle camera sway.**
   - While a menu waits for input and the player is not orbiting, the camera yaw drifts by ±4° on a slow sine with a period of about 20 s.
   - The distance never changes.
   - Right-drag orbiting suspends the sway, and it resumes from the new yaw.
9. **Other deferred minors:**
   - M/N during setup changes the player currently choosing.
   - Backspace repeats while held (after a 0.4 s delay, then every 0.05 s).

## Non-goals

- Saving characters, XP or levels.
- Sound.
- Networking.
- GPU rendering.
- Saving settings to disk.
- Throttling the CPU while the window is unfocused. macroquad has no focus event, so this is left out and noted as such.

## Anti-goals

- A slower frame. Raymarch at the default view must not regress past noise; the extra glyph draws are measured.
- A bot that looks broken: one that picks dimmed moves, stalls, or acts instantly with no visible think time.
- Pop-ups that leave the screen beneath in a changed state.
- Any change to camera distance.

## Verification

- **Pure logic gets `cargo test`:**
  - Turn limit and time-up winner or draw.
  - First-mover alternation.
  - Orb damage.
  - Damage line.
  - Score and stats.
  - Move log text.
  - Every pop-up's flow: pause, how to play, flee confirm, Esc back in setup.
  - Bot move choice for each class, including that it only picks available moves and that a seeded RNG makes it deterministic.
  - The bot's think delay.
  - Animation speed scaling.
  - `fit` for banner text.
  - Menu cost column strings.
  - HP bar colour thresholds.
  - Sway amplitude and that distance is kept.
  - Star patch stability.
- **Screenshots** for every visual item, before and after, plus 800×500.
- **Raymarch median** compared with the 20 ms baseline.

## Left to the builder

- Exact colours and glyph sets within the above.
- The block font's letter shapes.
- Log placement details.
- The moon and fire positions after shots.
- Bot think-time jitter.
