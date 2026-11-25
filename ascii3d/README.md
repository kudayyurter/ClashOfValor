# Clash of Valor 3D (prototype)

The direction for the next Clash of Valor: a real-time 3D world drawn entirely in glyphs, inspired by Senje's ASCII MMORPG (@ansenjeo). This prototype is a two-player, turn-based duel in that world. It is a CPU ray marcher whose output is a grid of glyphs instead of pixels.

```sh
cd ascii3d
cargo run --release
```

Unlike the terminal game, this opens its own window (macroquad). The font is JetBrains Mono (Nerd Font build), shipped under the SIL Open Font License in `assets/OFL.txt`.

## The duel

A turn-based duel for two players on one keyboard, or one player against the bot.

1. Pick a mode: Player vs Player or Player vs Bot.
2. Each player types a name and picks a class; the class previews in 3D as you browse. Against the bot, you also pick the bot's class (or Random).
   - **Warrior:** a knight with sword and shield. The shield absorbs damage before health.
   - **Assassin:** a hooded fighter with twin daggers and a critical strike.
   - **Sorcerer:** a robed caster whose staff hurls fire orbs.
3. On your turn, pick a move. The menu header says whose move it is and the turn number. Each move shows its cost and effect, and moves you can't afford are dimmed with the reason. Every move plays out in 3D: slash arcs, sparks, the flying fire orb, the shield rim, the crit vanish, potion sparkles and floating numbers. The last four moves are listed under the left panel.
4. Each fighter gets 40 turns. If nobody is knocked out by then, the higher health percentage wins; equal is a draw.
5. After the duel: Rematch (the other player moves first), New duel (names are kept) or Quit. The result shows the running score and duel stats.

The rules and narration come from the C++ game (`src/rules.rs`), with two changes: the fire orb deals 12, and the turn limit above. The bot uses the C++ bot's heuristics (`src/bot.rs`).

Controls:
- Arrow keys move the highlight, Enter confirms, and number keys pick a menu option directly.
- Space or Enter skips the move animation.
- Esc pauses (Resume, How to play, animation speed, New duel, Quit). During setup Esc steps back. Esc never quits; only the Quit menu items do.
- Flee asks for confirmation.
- M/N change the armour metal of the Warrior whose turn it is (or who is choosing).
- Right-mouse drag orbits the camera. The camera always stays at the same distance; there is no zoom. Between turns it sways gently.
- F3 shows the developer readout (fps, raymarch time, grid). E toggles outlines, F the fine fighter grid, G the glyph size, L chainmail or plate legs.

When frames get slow, only the fighter whose turn it is draws on the fine grid. The readout shows `fine both`, `fine active` or `fine off`. Below 640x400 the game asks for a larger window.

## Screenshots

`--shot out.png` renders one frame to a PNG and exits. Options:

- `--duel warrior,sorcerer` skips setup.
- `--moves 1,2` plays those menu choices in turn order first (Flee is auto-confirmed). An unavailable move prints `move N unavailable` and exits with status 1.
- `--at 0.6` freezes the last scripted move at that point of its animation.
- `--ff 39` pretends both fighters already used 39 turns, to reach the turn limit quickly.
- `--keys "1Ann|2Bo|3"` types scripted keys, one per frame: `|` is Enter, `<` and `>` are Up and Down, `#` is Backspace, `~` is Esc.
- `--size 800x500` sets the window size.
- `--yaw deg` and `--dist d` set the camera, `--metal i` sets the armour metal, and `--noedges` turns outlines off.

## What the reels show, and how this copies it

| Seen in the reels | How ascii-forge does it | Where |
|---|---|---|
| A world made of monospace glyphs on a dark background, coloured per cell | One glyph per screen cell. Each cell fires a 3x3 grid of rays into an SDF scene. | `ascii.rs` `sample_cell` |
| Shapes outlined with `/ \ | _ -` | If the owning object covers only part of the cell, the covered sub-samples' centroid gives the outline direction, which picks the glyph. | `ascii.rs` `edge_glyph` |
| Every armour metal has its own glyph set (`#%: Iron`, `0o. Gold`, `VA> Dragonscale`...) | A three-glyph ramp per metal, picked by brightness. | `scene.rs` `METALS` |
| Chainmail "using a custom grid and symbols" | The ring pattern is laid out in the part's own surface coordinates, so rows of `S 8 3 ( c` wrap around the body. | `ascii.rs` `chain_glyph` |
| Characters drawn with smaller glyphs than the world, with a dark halo | Cells that touch the hero are deferred and redrawn as 2x2 half-size cells, hero only. | `main.rs` fine pass |
| Grass that sways and parts around the player | Each grass cell is a blade whose lean comes from a wind field plus a push away from the player. | `ascii.rs` `grass_glyph` |
| Glowing fire and embers | Emissive glyphs with soft halo circles, plus free-floating ember glyphs depth-tested against the grid. | `main.rs` |

## What is a guess

The reels show the output, not the code. A news article says the original is written in JavaScript and uses "a special shader" for 3D characters. In some close-ups the chainmail glyphs change size with depth and follow the surface. That suggests glyphs are textured onto the meshes in UV space on the GPU. This prototype uses a screen-space grid and only lays the pattern out in surface space.

## Where to go next

- Move the work to the GPU: render the scene to a low-res G-buffer (one texel per cell), then a fullscreen pass picks glyphs from a font atlas.
- True UV-space glyphs: sample a glyph atlas with the mesh's UVs so glyphs shrink with distance and bend with the surface.
- Bloom, water reflections, weather.
