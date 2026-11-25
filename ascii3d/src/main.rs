//! Clash of Valor 3D: prototype of a 3D ASCII renderer for the next Clash of Valor.
//!
//! Per frame:
//!   1. Update the world (player, walk cycle, embers).
//!   2. Coarse pass: one glyph per big cell, sampled in parallel with rayon.
//!      Cells that touch the hero are deferred.
//!   3. Fine pass: each deferred cell is split 2x2 and resampled, hero only.
//!   4. Draw: glow halos, coarse glyphs, fine glyphs, free-floating ember glyphs, UI.

mod ascii;
mod models;
mod rules;
mod scene;

use ascii::{Cam, CellOut, Opts, Pass, sample_cell};
use macroquad::prelude::*;
use rayon::prelude::*;
use rules::Class;
use scene::{METALS, Scene};

const BG: Color = Color::new(0.035, 0.035, 0.07, 1.0);
const SIZES: [u16; 3] = [12, 16, 22];

fn conf() -> Conf {
    Conf {
        window_title: "Clash of Valor".to_owned(),
        window_width: 1280,
        window_height: 800,
        platform: miniquad::conf::Platform {
            linux_backend: miniquad::conf::LinuxBackend::WaylandWithX11Fallback,
            ..Default::default()
        },
        ..Default::default()
    }
}

struct Args {
    shot: Option<String>,
    yaw: f32,
    dist: f32,
    metal: usize,
}

fn parse_args() -> Args {
    let mut a = Args { shot: None, yaw: 0.0, dist: 7.0, metal: 0 };
    let mut it = std::env::args().skip(1);
    while let Some(k) = it.next() {
        match k.as_str() {
            "--shot" => a.shot = it.next(),
            "--yaw" => a.yaw = it.next().and_then(|v| v.parse().ok()).unwrap_or(a.yaw),
            "--dist" => a.dist = it.next().and_then(|v| v.parse().ok()).unwrap_or(a.dist),
            "--metal" => a.metal = it.next().and_then(|v| v.parse().ok()).unwrap_or(0),
            _ => {}
        }
    }
    a
}

struct Ember {
    p: Vec3,
    v: Vec3,
    life: f32,
    max: f32,
}

fn update_embers(embers: &mut Vec<Ember>, scene: &Scene, dt: f32) {
    let n = (dt * 35.0 + rand::gen_range(0.0, 1.0)) as usize;
    for _ in 0..n {
        embers.push(Ember {
            p: scene.fire + vec3(rand::gen_range(-0.15, 0.15), 0.35, rand::gen_range(-0.15, 0.15)),
            v: vec3(rand::gen_range(-0.3, 0.3), rand::gen_range(0.8, 1.8), rand::gen_range(-0.3, 0.3)),
            life: 0.0,
            max: rand::gen_range(0.8, 2.0),
        });
    }
    for e in embers.iter_mut() {
        e.life += dt;
        e.v.x += (scene.time * 3.0 + e.p.y * 4.0).sin() * dt * 0.8;
        e.v *= 1.0 - 0.6 * dt;
        e.v.y += 0.6 * dt;
        e.p += e.v * dt;
    }
    embers.retain(|e| e.life < e.max);
}

fn glyph_str(c: char, buf: &mut [u8; 4]) -> &str {
    c.encode_utf8(buf)
}

fn draw_glyph(font: &Font, c: char, x: f32, baseline: f32, size: u16, color: Color) {
    let mut buf = [0u8; 4];
    draw_text_ex(glyph_str(c, &mut buf), x, baseline, TextParams { font: Some(font), font_size: size, color, ..Default::default() });
}

#[macroquad::main(conf)]
async fn main() {
    let args = parse_args();
    let font = load_ttf_font_from_bytes(include_bytes!("../assets/mono.ttf")).expect("font");
    let mut scene = Scene::new();
    scene.set_fighters([Class::Warrior, Class::Warrior]);
    for b in &mut scene.bodies {
        b.metal = args.metal % METALS.len();
    }
    // The fighter whose turn it is; M/N change their metal.
    let active = 0usize;
    let (mut cam_yaw, mut cam_pitch, mut cam_dist) = (args.yaw.to_radians(), 0.3f32, args.dist);
    let mut size_i = 1usize;
    let mut opts = Opts { edges: true, fine: true };
    let mut embers: Vec<Ember> = Vec::new();
    let mut last_mouse: Vec2 = mouse_position().into();
    let mut frame = 0u32;
    if args.shot.is_some() {
        // Warm up the ember stream before the capture.
        for _ in 0..90 {
            scene.update(1.0 / 60.0);
            update_embers(&mut embers, &scene, 1.0 / 60.0);
        }
    }

    loop {
        // Screenshot mode uses a fixed step: a hidden window may only get 1 frame per second.
        let dt = if args.shot.is_some() { 1.0 / 60.0 } else { get_frame_time().min(1.0 / 20.0) };
        frame += 1;

        // ---- input -------------------------------------------------------------
        let m: Vec2 = mouse_position().into();
        if is_mouse_button_down(MouseButton::Right) {
            let d = m - last_mouse;
            cam_yaw -= d.x * 0.008;
            cam_pitch = (cam_pitch + d.y * 0.006).clamp(0.08, 1.3);
        }
        last_mouse = m;
        let wheel = mouse_wheel().1;
        if wheel != 0.0 {
            cam_dist = (cam_dist * if wheel > 0.0 { 0.9 } else { 1.1 }).clamp(3.0, 16.0);
        }
        if is_key_pressed(KeyCode::M) {
            let b = &mut scene.bodies[active];
            b.metal = (b.metal + 1) % METALS.len();
        }
        if is_key_pressed(KeyCode::N) {
            let b = &mut scene.bodies[active];
            b.metal = (b.metal + METALS.len() - 1) % METALS.len();
        }
        if is_key_pressed(KeyCode::E) {
            opts.edges = !opts.edges;
        }
        if is_key_pressed(KeyCode::F) {
            opts.fine = !opts.fine;
        }
        if is_key_pressed(KeyCode::G) {
            size_i = (size_i + 1) % SIZES.len();
        }
        if is_key_pressed(KeyCode::L) {
            scene.chain_legs = !scene.chain_legs;
        }
        if is_key_pressed(KeyCode::Escape) {
            break;
        }
        scene.update(dt);
        update_embers(&mut embers, &scene, dt);

        // ---- camera and grid ---------------------------------------------------
        let target = (scene.bodies[0].pos + scene.bodies[1].pos) * 0.5 + vec3(0.0, 1.0, 0.0);
        let eye = target + vec3(cam_pitch.cos() * cam_yaw.sin(), cam_pitch.sin(), cam_pitch.cos() * cam_yaw.cos()) * cam_dist;
        let (sw, sh) = (screen_width(), screen_height());
        let cam = Cam::look_at(eye, target, 50.0, sw, sh);

        let fs = SIZES[size_i];
        let mut cw = (measure_text("MMMMMMMMMM", Some(&font), fs, 1.0).width / 10.0).round().max(4.0);
        let mut ch = (fs as f32 * 1.2).round();
        // Even sizes so the fine grid splits each cell exactly in half.
        cw += cw % 2.0;
        ch += ch % 2.0;
        let cols = (sw / cw).ceil() as usize;
        let rows = (sh / ch).ceil() as usize;

        let t0 = std::time::Instant::now();
        // ---- coarse pass -------------------------------------------------------
        let coarse: Vec<CellOut> = (0..cols * rows)
            .into_par_iter()
            .map(|i| {
                let (cx, cy) = (i % cols, i / cols);
                sample_cell(&scene, &cam, cx as f32 * cw, cy as f32 * ch, cw, ch, 3, Pass::Coarse, &opts)
            })
            .collect();

        // ---- fine pass (hero only) ---------------------------------------------
        let (fw, fh) = (cw / 2.0, ch / 2.0);
        let deferred: Vec<usize> = (0..coarse.len()).filter(|&i| matches!(coarse[i], CellOut::Defer)).collect();
        let fine: Vec<(f32, f32, CellOut)> = deferred
            .par_iter()
            .flat_map_iter(|&i| {
                let (bx, by) = ((i % cols) as f32 * cw, (i / cols) as f32 * ch);
                let (scene, cam, opts) = (&scene, &cam, &opts);
                (0..4).map(move |q| {
                    let (x, y) = (bx + (q % 2) as f32 * fw, by + (q / 2) as f32 * fh);
                    (x, y, sample_cell(scene, cam, x, y, fw, fh, 3, Pass::Fine, opts))
                })
            })
            .collect();

        let render_ms = t0.elapsed().as_secs_f32() * 1000.0;
        // ---- draw --------------------------------------------------------------
        // Screenshot mode draws into an off-screen texture (reading back the window's
        // framebuffer is unreliable on Wayland).
        let capture = args.shot.is_some() && frame == 6;
        let shot_rt = capture.then(|| {
            let rt = render_target(sw as u32, sh as u32);
            set_camera(&Camera2D { render_target: Some(rt.clone()), ..Camera2D::from_display_rect(Rect::new(0.0, 0.0, sw, sh)) });
            rt
        });
        clear_background(BG);

        for (i, c) in coarse.iter().enumerate() {
            if let CellOut::Glyph(cell) = c {
                if cell.glow > 0.0 {
                    let (x, y) = ((i % cols) as f32 * cw + cw * 0.5, (i / cols) as f32 * ch + ch * 0.5);
                    let col = Color::new(cell.color.r, cell.color.g * 0.7, cell.color.b * 0.4, 0.05 * cell.glow);
                    draw_circle(x, y, ch * 1.8, col);
                }
            }
        }
        for (i, c) in coarse.iter().enumerate() {
            if let CellOut::Glyph(cell) = c {
                let (x, y) = ((i % cols) as f32 * cw, (i / cols) as f32 * ch);
                draw_glyph(&font, cell.glyph, x, y + ch * 0.78, fs, cell.color);
            }
        }
        for (x, y, c) in &fine {
            if let CellOut::Glyph(cell) = c {
                if cell.glow > 0.0 {
                    draw_circle(x + fw * 0.5, y + fh * 0.5, fh * 1.6, Color::new(cell.color.r, cell.color.g, cell.color.b, 0.06 * cell.glow));
                }
                draw_glyph(&font, cell.glyph, *x, y + fh * 0.78, fs / 2, cell.color);
            }
        }

        // Embers are free-floating glyphs (not snapped to the grid), depth-tested
        // against the coarse cells.
        for e in &embers {
            let Some((sp, depth)) = cam.project(e.p) else { continue };
            let (cx, cy) = ((sp.x / cw) as usize, (sp.y / ch) as usize);
            if cx >= cols || cy >= rows {
                continue;
            }
            let wall = match coarse[cy * cols + cx] {
                CellOut::Glyph(c) => c.depth,
                CellOut::Empty(d) => d,
                CellOut::Defer => f32::MAX,
            };
            if depth > wall + 0.2 {
                continue;
            }
            let f = e.life / e.max;
            let g = if f < 0.33 { '*' } else if f < 0.66 { 'o' } else { '.' };
            let col = Color::new(1.0, 0.85 - 0.6 * f, 0.35 - 0.3 * f, 1.0 - f * 0.7);
            draw_circle(sp.x, sp.y, ch * 0.9, Color::new(1.0, 0.5, 0.15, 0.08 * (1.0 - f)));
            let size = ((fs as f32) * 5.0 / depth).clamp(6.0, 28.0) as u16;
            draw_glyph(&font, g, sp.x - size as f32 * 0.3, sp.y + size as f32 * 0.35, size, col);
        }

        // Name tag over each fighter, in their accent colour.
        for (i, b) in scene.bodies.iter().enumerate() {
            let name = ["Player 1", "Player 2"][i];
            if let Some((sp, _)) = cam.project(b.to_world(b.anchors.head + vec3(0.0, 0.5, 0.0))) {
                let w = measure_text(name, Some(&font), 14, 1.0).width;
                let c = Color::new(b.accent.x, b.accent.y, b.accent.z, 1.0);
                draw_text_ex(name, sp.x - w * 0.5, sp.y, TextParams { font: Some(&font), font_size: 14, color: c, ..Default::default() });
            }
        }

        draw_ui(&font, &scene, &opts, fs, cols, rows, deferred.len(), render_ms);

        if let (Some(rt), Some(path)) = (shot_rt, &args.shot) {
            set_default_camera();
            rt.texture.get_texture_data().export_png(path);
            eprintln!("saved {path} (raymarch {render_ms:.1} ms)");
            break;
        }
        next_frame().await;
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_ui(font: &Font, scene: &Scene, opts: &Opts, fs: u16, cols: usize, rows: usize, deferred: usize, render_ms: f32) {
    let (sw, sh) = (screen_width(), screen_height());
    let tp = |color| TextParams { font: Some(font), font_size: 13, color, ..Default::default() };
    let dim = Color::new(0.6, 0.62, 0.7, 1.0);

    draw_rectangle(0.0, 0.0, sw, 22.0, Color::new(0.0, 0.0, 0.0, 0.75));
    let info = format!(
        "Clash of Valor 3D  {} fps  raymarch {:.1} ms  grid {}x{} @{}px  fine cells {}  edges {}  fine {}",
        get_fps(),
        render_ms,
        cols,
        rows,
        fs,
        deferred * 4,
        if opts.edges { "on" } else { "off" },
        if opts.fine { "on" } else { "off" },
    );
    draw_text_ex(&info, 8.0, 15.0, tp(dim));

    let bar_y = sh - 46.0;
    draw_rectangle(0.0, bar_y, sw, 46.0, Color::new(0.0, 0.0, 0.0, 0.8));
    draw_text_ex("Metal of the armour", 8.0, bar_y + 14.0, tp(dim));
    let mut x = 8.0;
    for (i, m) in METALS.iter().enumerate() {
        let g: String = m.glyphs.iter().collect();
        let label = format!("{g} {}", m.name);
        let w = measure_text(&label, Some(font), 13, 1.0).width + 12.0;
        let c = Color::new(m.rgb.x, m.rgb.y, m.rgb.z, 1.0);
        if i == scene.bodies[0].metal {
            draw_rectangle_lines(x - 2.0, bar_y + 19.0, w, 20.0, 1.0, c);
        }
        draw_text_ex(&label, x + 4.0, bar_y + 33.0, tp(if i == scene.bodies[0].metal { c } else { dim }));
        x += w + 6.0;
    }
    let help = "RMB drag turn  wheel zoom  M/N metal  L legs  E edges  F fine grid  G glyph size  Esc quit";
    let w = measure_text(help, Some(font), 13, 1.0).width;
    draw_text_ex(help, sw - w - 8.0, bar_y + 14.0, tp(dim));
}
