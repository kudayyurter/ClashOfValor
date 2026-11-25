# Clash of Valor 3D (prototype)

The direction for the next Clash of Valor: a real-time 3D world drawn entirely in glyphs. This is a Rust prototype of the renderer, inspired by Senje's ASCII MMORPG (@ansenjeo). It is a CPU ray marcher whose output is a grid of glyphs instead of pixels.

```sh
cd ascii3d
cargo run --release
```

Unlike the current game, this opens its own window (macroquad) rather than drawing in the terminal. The font is JetBrains Mono (Nerd Font build), shipped under the SIL Open Font License in `assets/OFL.txt`.

Controls: WASD move, right mouse drag to turn the camera, wheel to zoom, M/N cycle the armour metal, L toggles chainmail/plate legs, E toggles outlines, F toggles the fine hero grid, G cycles glyph size, Esc quits.

`--shot out.png [--yaw deg] [--dist d] [--metal i] [--walk]` renders one frame to a PNG and exits.

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
