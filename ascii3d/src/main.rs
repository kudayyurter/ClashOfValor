//! Clash of Valor 3D: a turn-based duel drawn entirely in glyphs.
//!
//! Per frame:
//!   1. Input goes to the duel flow (flow.rs); the world updates (poses, embers).
//!   2. Coarse pass: one glyph per big cell, sampled in parallel with rayon.
//!      Cells that touch the hero are deferred.
//!   3. Fine pass: each deferred cell is split 2x2 and resampled, hero only.
//!   4. Draw: glow halos, coarse glyphs, fine glyphs, free-floating ember glyphs, UI.
//!
//! `--shot out.png` renders one frame to a PNG and exits. With `--duel a,b` the duel
//! starts without setup; `--moves 1,2` plays menu choices first; `--keys` types
//! scripted keys (one per frame; `|` Enter, `<` Up, `>` Down, `#` Backspace).

mod anim;
mod ascii;
mod effects;
mod flow;
mod models;
mod rules;
mod scene;
mod ui;

use ascii::{Cam, CellOut, FineGovernor, Opts, Pass, sample_cell};
use macroquad::prelude::*;
use rayon::prelude::*;
use flow::{Flow, Input, Screen};
use rules::Class;
use effects::{Fx, Kind, Shake};
use scene::{ACCENTS, FX_ID, METALS, Mat, Part, Scene, Shape};

const BG: Color = Color::new(0.035, 0.035, 0.07, 1.0);
const SIZES: [u16; 3] = [12, 16, 22];

/// `--size WxH` sets the window size (read here because `conf` runs before `main`).
fn window_size() -> (i32, i32) {
    let args: Vec<String> = std::env::args().collect();
    args.iter()
        .position(|a| a == "--size")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.split_once('x'))
        .and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)))
        .unwrap_or((1280, 800))
}

fn conf() -> Conf {
    let (w, h) = window_size();
    Conf {
        window_title: "Clash of Valor".to_owned(),
        window_width: w,
        window_height: h,
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
    duel: Option<[Class; 2]>,
    moves: Vec<u8>,
    keys: String,
    at: Option<f32>,
    noedges: bool,
}

fn parse_class(s: &str) -> Option<Class> {
    Class::ALL.into_iter().find(|c| c.name().eq_ignore_ascii_case(s.trim()))
}

fn parse_args() -> Args {
    let mut a = Args { shot: None, yaw: 0.0, dist: 5.4, metal: 0, duel: None, moves: Vec::new(), keys: String::new(), at: None, noedges: false };
    let mut it = std::env::args().skip(1);
    while let Some(k) = it.next() {
        match k.as_str() {
            "--shot" => a.shot = it.next(),
            "--yaw" => a.yaw = it.next().and_then(|v| v.parse().ok()).unwrap_or(a.yaw),
            "--dist" => a.dist = it.next().and_then(|v| v.parse().ok()).unwrap_or(a.dist),
            "--duel" => {
                let v = it.next().unwrap_or_default();
                let c: Vec<Class> = v.split(',').filter_map(parse_class).collect();
                if c.len() == 2 {
                    a.duel = Some([c[0], c[1]]);
                } else {
                    eprintln!("--duel wants two of warrior,assassin,sorcerer (got {v:?})");
                    std::process::exit(2);
                }
            }
            "--noedges" => a.noedges = true,
            "--moves" => a.moves = it.next().unwrap_or_default().split(',').filter_map(|m| m.trim().parse().ok()).collect(),
            "--keys" => a.keys = it.next().unwrap_or_default(),
            "--at" => a.at = it.next().and_then(|v| v.parse().ok()),
            "--metal" => a.metal = it.next().and_then(|v| v.parse().ok()).unwrap_or(0),
            _ => {}
        }
    }
    a
}

/// The world-side state that follows the duel: fighter bodies and effects.
struct World {
    scene: Scene,
    fx: Fx,
    shake: Shake,
    staged: [Class; 2],
}

impl World {
    /// One simulation step. `advance` moves the duel's animation on (screenshots freeze it).
    fn tick(&mut self, flow: &mut Flow, dt: f32, advance: bool) {
        // Effects first, so an impact fired by a skip this frame is still seen while the
        // move is on screen.
        let impact = flow.take_impact();
        // The last move's targets flash white for 0.12 s after its impact. Driven by
        // the flow's impact clock, so a frozen screenshot shows it too.
        let flash = (1.0 - flow.since_impact / 0.12).max(0.0);
        for b in &mut self.scene.bodies {
            b.flash = 0.0;
        }
        for e in flow.last.iter().flat_map(|r| &r.events) {
            if let rules::Event::Hit { target, .. } = *e {
                self.scene.bodies[target].flash = flash;
            }
        }
        // Jolt the camera on impact: harder for crits and fire orbs, softer when the
        // shield takes it all.
        if impact && let Some(res) = &flow.last {
            for e in &res.events {
                if let rules::Event::Hit { absorbed, damage, crit, .. } = *e {
                    let k = if crit || res.mv == rules::Move::FireOrb {
                        0.11
                    } else if damage == 0 && absorbed > 0 {
                        0.03
                    } else {
                        0.05
                    };
                    self.shake.kick(k);
                }
            }
        }
        self.shake.update(dt);
        if let Screen::Animate { action, .. } = &flow.screen {
            self.fx.on_action(action, &self.scene.bodies, dt, impact);
        } else {
            self.fx.orb = None;
        }
        if advance {
            flow.update(dt);
        }
        let (classes, _) = flow.preview();
        if classes != self.staged {
            self.scene.set_fighters(classes);
            self.staged = classes;
        }
        let gap = (self.scene.bodies[1].pos - self.scene.bodies[0].pos).length();
        let (poses, dims) = flow.body_state(self.scene.time, gap);
        for (i, ((b, p), d)) in self.scene.bodies.iter_mut().zip(poses).zip(dims).enumerate() {
            // A little dust when a fallen fighter hits the ground.
            if b.pose.fall < 0.99 && p.fall >= 0.99 {
                self.fx.burst(b.to_world(vec3(0.0, 0.05, -1.0)), 18, Kind::Smoke, vec3(0.5, 0.45, 0.4));
            }
            b.pose = p;
            b.dim = d;
            self.fx.ward[i] = match &flow.shown {
                Some(shown) if shown[i].shield > 0 && d < 0.99 => 1.0,
                _ => 0.0,
            };
        }
        self.scene.fx_parts = self
            .fx
            .orb
            .map(|c| Part { id: FX_ID, mat: Mat::Orb, shape: Shape::Sphere { c, r: 0.16 } })
            .into_iter()
            .collect();
        self.scene.update(dt);
        self.fx.campfire(self.scene.fire, self.scene.time, dt);
        self.fx.update(dt);
    }
}

/// This frame's key presses as flow inputs. A digit arrives as both `Digit` and `Char`;
/// the flow uses whichever fits the screen.
fn read_inputs() -> Vec<Input> {
    let mut v = Vec::new();
    let keys = [
        (KeyCode::Up, Input::Up),
        (KeyCode::Down, Input::Down),
        (KeyCode::Enter, Input::Enter),
        (KeyCode::KpEnter, Input::Enter),
        (KeyCode::Space, Input::Space),
        (KeyCode::Backspace, Input::Backspace),
        (KeyCode::Escape, Input::Esc),
    ];
    for (k, i) in keys {
        if is_key_pressed(k) {
            v.push(i);
        }
    }
    let digits = [
        (KeyCode::Key1, KeyCode::Kp1),
        (KeyCode::Key2, KeyCode::Kp2),
        (KeyCode::Key3, KeyCode::Kp3),
        (KeyCode::Key4, KeyCode::Kp4),
        (KeyCode::Key5, KeyCode::Kp5),
        (KeyCode::Key6, KeyCode::Kp6),
        (KeyCode::Key7, KeyCode::Kp7),
        (KeyCode::Key8, KeyCode::Kp8),
        (KeyCode::Key9, KeyCode::Kp9),
    ];
    for (n, (a, b)) in digits.iter().enumerate() {
        if is_key_pressed(*a) || is_key_pressed(*b) {
            v.push(Input::Digit(n as u8 + 1));
        }
    }
    while let Some(c) = get_char_pressed() {
        if !c.is_control() {
            v.push(Input::Char(c));
        }
    }
    v
}

/// One scripted `--keys` character as one frame of input, the way `read_inputs` would
/// report the key.
fn scripted(c: char) -> Vec<Input> {
    match c {
        '|' => vec![Input::Enter],
        '<' => vec![Input::Up],
        '>' => vec![Input::Down],
        '#' => vec![Input::Backspace],
        '1'..='9' => vec![Input::Digit(c as u8 - b'0'), Input::Char(c)],
        ' ' => vec![Input::Space, Input::Char(' ')],
        _ => vec![Input::Char(c)],
    }
}

/// Health, stamina and class-bar values for a fighter's panel.
fn bar_targets(f: &rules::Fighter) -> [f32; 3] {
    let extra = match f.class {
        Class::Warrior => f.shield,
        Class::Sorcerer => f.mana,
        Class::Assassin => 0,
    };
    [f.health as f32, f.stamina as f32, extra as f32]
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
    let mut flow = match args.duel {
        Some(classes) => Flow::with_duel(["Player 1", "Player 2"], classes),
        None => Flow::new(),
    };
    let mut world = World { scene: Scene::new(), fx: Fx::default(), shake: Shake::default(), staged: flow.preview().0 };
    world.scene.set_fighters(world.staged);
    for b in &mut world.scene.bodies {
        b.metal = args.metal % METALS.len();
    }
    const STEP: f32 = 1.0 / 60.0;
    if args.shot.is_some() {
        // Warm up the ember stream before the capture.
        for _ in 0..90 {
            world.tick(&mut flow, STEP, false);
        }
    }
    // Scripted moves play out fully, except that `--at` freezes the last one part-way.
    for (n, &m) in args.moves.iter().enumerate() {
        let ok = matches!(&flow.screen, Screen::Choose { menu } if (m as usize).checked_sub(1).is_some_and(|i| menu.enabled.get(i) == Some(&true)));
        if !ok {
            eprintln!("move {m} unavailable");
            std::process::exit(1);
        }
        flow.step(&[Input::Digit(m)]);
        let last = n + 1 == args.moves.len();
        match (last, args.at, &flow.screen) {
            (true, Some(at), Screen::Animate { action, .. }) => {
                let target = at.clamp(0.0, 1.0) * action.dur;
                let mut t = 0.0;
                while t + 1e-4 < target {
                    let step = (target - t).min(STEP);
                    world.tick(&mut flow, step, true);
                    t += step;
                }
            }
            _ => {
                for _ in 0..400 {
                    if !matches!(flow.screen, Screen::Animate { .. }) {
                        break;
                    }
                    world.tick(&mut flow, STEP, true);
                }
                // Wait out the next menu's input grace.
                for _ in 0..20 {
                    world.tick(&mut flow, STEP, true);
                }
            }
        }
    }
    let mut script: std::collections::VecDeque<char> = args.keys.chars().collect();
    let mut bars: Option<[[f32; 3]; 2]> = None;
    // The camera distance is fixed: there is no zoom.
    let (mut cam_yaw, mut cam_pitch, cam_dist) = (args.yaw.to_radians(), 0.3f32, args.dist);
    let mut size_i = 1usize;
    let mut opts = Opts { edges: !args.noedges, fine: true, fine_mask: [true, true] };
    let mut governor = FineGovernor::new();
    let mut last_mouse: Vec2 = mouse_position().into();
    let mut frame = 0u32;

    loop {
        // Screenshot mode uses a fixed step: a hidden window may only get 1 frame per second.
        let dt = if args.shot.is_some() { 1.0 / 60.0 } else { get_frame_time().min(1.0 / 20.0) };
        frame += 1;

        // ---- input -------------------------------------------------------------
        let inputs = if args.shot.is_some() { script.pop_front().map(scripted).unwrap_or_default() } else { read_inputs() };
        flow.step(&inputs);
        if flow.quit {
            break;
        }
        // Screenshots are frozen at the scripted moment.
        world.tick(&mut flow, dt, args.shot.is_none());
        let scene = &mut world.scene;
        let active = flow.active();
        let hotkeys = !flow.typing() && args.shot.is_none();
        let m: Vec2 = mouse_position().into();
        if is_mouse_button_down(MouseButton::Right) {
            let d = m - last_mouse;
            cam_yaw -= d.x * 0.008;
            cam_pitch = (cam_pitch + d.y * 0.006).clamp(0.08, 1.3);
        }
        last_mouse = m;
        if hotkeys && is_key_pressed(KeyCode::M) {
            let b = &mut scene.bodies[active];
            b.metal = (b.metal + 1) % METALS.len();
        }
        if hotkeys && is_key_pressed(KeyCode::N) {
            let b = &mut scene.bodies[active];
            b.metal = (b.metal + METALS.len() - 1) % METALS.len();
        }
        if hotkeys && is_key_pressed(KeyCode::E) {
            opts.edges = !opts.edges;
        }
        if hotkeys && is_key_pressed(KeyCode::F) {
            opts.fine = !opts.fine;
        }
        if hotkeys && is_key_pressed(KeyCode::G) {
            size_i = (size_i + 1) % SIZES.len();
        }
        if hotkeys && is_key_pressed(KeyCode::L) {
            scene.chain_legs = !scene.chain_legs;
        }
        // Panel bars ease toward the numbers on screen (snap in screenshots).
        bars = flow.shown.as_ref().map(|shown| {
            let target = [bar_targets(&shown[0]), bar_targets(&shown[1])];
            let k = if args.shot.is_some() { 1.0 } else { (dt * 6.0).min(1.0) };
            let mut b = bars.unwrap_or(target);
            for i in 0..2 {
                for j in 0..3 {
                    b[i][j] += (target[i][j] - b[i][j]) * k;
                }
            }
            b
        });

        // ---- camera and grid ---------------------------------------------------
        let target = (scene.bodies[0].pos + scene.bodies[1].pos) * 0.5 + vec3(0.0, 1.45, 0.0);
        let eye = target + vec3(cam_pitch.cos() * cam_yaw.sin(), cam_pitch.sin(), cam_pitch.cos() * cam_yaw.cos()) * cam_dist;
        let (eye, target) = effects::shaken(eye, target, world.shake.offset());
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
                sample_cell(scene, &cam, cx as f32 * cw, cy as f32 * ch, cw, ch, 3, Pass::Coarse, &opts)
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
        // Slow frames drop the waiting fighter to the coarse grid (screenshots keep both).
        if args.shot.is_none() {
            opts.fine_mask = governor.update(render_ms, active);
        }
        // ---- draw --------------------------------------------------------------
        // Screenshot mode draws into an off-screen texture (reading back the window's
        // framebuffer is unreliable on Wayland).
        let capture = args.shot.is_some() && frame == 6 + args.keys.chars().count() as u32;
        let shot_rt = capture.then(|| {
            let rt = render_target(sw as u32, sh as u32);
            set_camera(&Camera2D { render_target: Some(rt.clone()), ..Camera2D::from_display_rect(Rect::new(0.0, 0.0, sw, sh)) });
            rt
        });
        clear_background(BG);

        for (i, c) in coarse.iter().enumerate() {
            if let CellOut::Glyph(cell) = c
                && cell.glow > 0.0 {
                    let (x, y) = ((i % cols) as f32 * cw + cw * 0.5, (i / cols) as f32 * ch + ch * 0.5);
                    let col = Color::new(cell.color.r, cell.color.g * 0.7, cell.color.b * 0.4, 0.05 * cell.glow);
                    draw_circle(x, y, ch * 1.8, col);
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

        // Effects are free-floating glyphs (not snapped to the grid), depth-tested
        // against the coarse cells.
        let depth_at = |x: f32, y: f32| {
            let (cx, cy) = ((x / cw) as usize, (y / ch) as usize);
            if x < 0.0 || y < 0.0 || cx >= cols || cy >= rows {
                return f32::MAX;
            }
            match coarse[cy * cols + cx] {
                CellOut::Glyph(c) => c.depth,
                CellOut::Empty(d) => d,
                CellOut::Defer => f32::MAX,
            }
        };
        let chests = [0, 1].map(|i| scene.bodies[i].to_world(scene.bodies[i].anchors.chest));
        effects::draw(&world.fx, &cam, &depth_at, &font, fs, chests);

        // Name tag over each fighter, in their accent colour; a marker over the active one.
        let names = flow.names();
        for (i, b) in scene.bodies.iter().enumerate() {
            let name = &names[i];
            if let Some((sp, _)) = cam.project(b.to_world(vec3(0.0, 2.5, 0.0))) {
                let w = measure_text(name, Some(&font), 14, 1.0).width;
                let c = Color::new(b.accent.x, b.accent.y, b.accent.z, 1.0 - b.dim * 0.6);
                draw_text_ex(name, sp.x - w * 0.5, sp.y, TextParams { font: Some(&font), font_size: 14, color: c, ..Default::default() });
                if flow.duel.is_some() && i == active && flow.banner().is_none() {
                    let m = "▼";
                    let mw = measure_text(m, Some(&font), 14, 1.0).width;
                    draw_text_ex(m, sp.x - mw * 0.5, sp.y + 16.0, TextParams { font: Some(&font), font_size: 14, color: c, ..Default::default() });
                }
            }
        }

        draw_ui(&font, &opts, fs, cols, rows, deferred.len(), render_ms);
        draw_duel_ui(&font, &flow, scene, bars, frame);

        if let (Some(rt), Some(path)) = (shot_rt, &args.shot) {
            set_default_camera();
            rt.texture.get_texture_data().export_png(path);
            eprintln!("saved {path} (raymarch {render_ms:.1} ms)");
            break;
        }
        next_frame().await;
    }
}

fn draw_ui(font: &Font, opts: &Opts, fs: u16, cols: usize, rows: usize, deferred: usize, render_ms: f32) {
    let (sw, sh) = (screen_width(), screen_height());
    let tp = |color| TextParams { font: Some(font), font_size: 13, color, ..Default::default() };
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
        match (opts.fine, opts.fine_mask) {
            (false, _) => "off",
            (true, [true, true]) => "both",
            _ => "active",
        },
    );
    draw_text_ex(&info, 8.0, 15.0, tp(ui::DIM));
    let help = "arrows/digits choose  Enter confirm  M/N armour  RMB orbit  E edges  F fine  G glyphs  Esc quit";
    draw_rectangle(0.0, sh - 18.0, sw, 18.0, Color::new(0.0, 0.0, 0.0, 0.75));
    ui::text_centered(font, help, sw * 0.5, sh - 5.0, 12, Color::new(0.5, 0.52, 0.6, 1.0));
}

/// Setup prompts, status panels, the move menu, the caption and the result banner.
fn draw_duel_ui(font: &Font, flow: &Flow, scene: &Scene, bars: Option<[[f32; 3]; 2]>, frame: u32) {
    let (sw, sh) = (screen_width(), screen_height());
    let cx = sw * 0.5;
    let menu_bottom = sh - 22.0;
    if let (Some(shown), Some(bars)) = (&flow.shown, bars) {
        let active = flow.active();
        let views = [0, 1].map(|i| ui::PanelView {
            fighter: &shown[i],
            bars: bars[i],
            accent: scene.bodies[i].accent,
            active: i == active && flow.banner().is_none(),
            metal: (shown[i].class == Class::Warrior).then(|| METALS[scene.bodies[i].metal].name),
        });
        ui::draw_panels(font, views);
    }
    match &flow.screen {
        Screen::Setup { who, naming, field, menu, .. } => {
            let accent = ACCENTS[*who];
            let y = sh * 0.6;
            if *naming {
                let title = format!("Player {}, enter your name", who + 1);
                ui::draw_name_prompt(font, &title, field, accent, y, (frame / 30).is_multiple_of(2));
            } else {
                let title = format!("{}, choose your class", flow.names()[*who]);
                ui::text_centered(font, &title, cx, y, ui::FONT, ui::rgb(accent));
                ui::draw_menu(font, menu, cx, y + 14.0);
            }
        }
        Screen::Choose { menu } => {
            let top = menu_bottom - ui::menu_height(menu);
            ui::draw_caption(font, &flow.caption, top - 10.0);
            ui::draw_menu(font, menu, cx, top);
        }
        Screen::Animate { .. } => {
            ui::draw_caption(font, &flow.caption, menu_bottom - 10.0);
        }
        Screen::Result { menu } => {
            let top = menu_bottom - ui::menu_height(menu);
            ui::draw_caption(font, &flow.caption, top - 10.0);
            ui::draw_menu(font, menu, cx, top);
            if let Some((text, who)) = flow.banner() {
                ui::draw_banner(font, &text, scene.bodies[who].accent);
            }
        }
    }
}
