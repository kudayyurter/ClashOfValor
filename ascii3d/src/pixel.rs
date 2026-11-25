//! THROWAWAY SPIKE: pixel-art renderers used only to compare art styles in screenshots.
//!
//! 1 Chunky: the 3D scene ray-marched at 320x200, one ray per pixel, outlines, Bayer
//!   dither and a fixed palette.
//! 4 Mono: the same render mapped to four cold shades.
//! 3 Sprites: only the fighters come from the 3D models (fixed side camera); the world
//!   is a flat 2D backdrop painted in screen space.

use crate::ascii::{Cam, MOON, STAR_PATCH, flash_rgb, grass_light};
use crate::effects::{Fx, Kind};
use crate::scene::{MAX_DIST, Mat, SKY, Scene, hash2};
use macroquad::prelude::*;
use rayon::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Style {
    Chunky,
    Mono,
    Sprites,
}

impl Style {
    pub fn parse(s: &str) -> Option<Style> {
        match s {
            "chunky" | "1" => Some(Style::Chunky),
            "mono" | "4" => Some(Style::Mono),
            "sprites" | "3" => Some(Style::Sprites),
            _ => None,
        }
    }
}

#[derive(Clone, Copy)]
struct Px {
    rgb: Vec3,
    id: u16,
    mat: Mat,
    t: f32,
    owner: Option<usize>,
    /// Transparent (sprites style: not part of a fighter).
    clear: bool,
}

const fn hex(v: u32) -> Vec3 {
    Vec3::new(((v >> 16) & 255) as f32 / 255.0, ((v >> 8) & 255) as f32 / 255.0, (v & 255) as f32 / 255.0)
}

const PALETTE: [Vec3; 34] = [
    hex(0x0b0b14), hex(0x141428), hex(0x1e2340), hex(0x2b3560), hex(0x3d4f86), hex(0x5a74b0), hex(0x8aa6d8), hex(0xc8d8f0),
    hex(0x0f2a1e), hex(0x1c4430), hex(0x2e6640), hex(0x4a8c4c), hex(0x7cb85a),
    hex(0x2a1a14), hex(0x4a2e20), hex(0x7a4a2c), hex(0xa87444),
    hex(0x8a2a14), hex(0xc8481c), hex(0xf07c28), hex(0xffb848), hex(0xfff0a8),
    hex(0x5a1428), hex(0xa02838), hex(0xe04848),
    hex(0x3a1e50), hex(0x6a3490), hex(0x9a5cc0),
    hex(0x4a4e5a), hex(0x7a8090), hex(0xb4bac6),
    hex(0xe8d0a8), hex(0xfffaf0), hex(0x58c8e8),
];

const MONO: [Vec3; 4] = [hex(0x0e0f1e), hex(0x33406a), hex(0x7a92b8), hex(0xdce8f0)];

const BAYER: [f32; 16] = [0.0, 8.0, 2.0, 10.0, 12.0, 4.0, 14.0, 6.0, 3.0, 11.0, 1.0, 9.0, 15.0, 7.0, 13.0, 5.0];

fn bayer(x: usize, y: usize) -> f32 {
    BAYER[(y % 4) * 4 + x % 4] / 16.0 - 0.47
}

fn nearest(pal: &[Vec3], c: Vec3) -> Vec3 {
    let w = vec3(0.3, 0.59, 0.11);
    let mut best = pal[0];
    let mut bd = f32::MAX;
    for &p in pal {
        let d = (p - c) * (p - c);
        let d = d.dot(w) + 0.15 * ((p - c).dot(Vec3::ONE)).powi(2) / 9.0;
        if d < bd {
            bd = d;
            best = p;
        }
    }
    best
}

fn lum(c: Vec3) -> f32 {
    c.dot(vec3(0.3, 0.59, 0.11))
}

// ---- sky (3D styles) -------------------------------------------------------

fn ridge(a: f32) -> f32 {
    0.016 + 0.01 * ((a * 7.0).sin() + 0.6 * (a * 17.0 + 1.3).sin() + 0.3 * (a * 41.0).sin()) / 1.9
}

fn sky_px(dir: Vec3, time: f32) -> Vec3 {
    let el = dir.y.clamp(-1.0, 1.0).asin();
    let az = dir.z.atan2(dir.x);
    let r = ridge(az);
    // Pine treeline: pointed tips along the crest.
    let u = az * 70.0;
    let (i, f) = (u.floor() as i32, u.fract());
    let ht = hash2(i, 17);
    let tip = r + if ht > 0.25 { ht * 0.014 * (1.0 - (2.0 * f - 1.0).abs()) } else { 0.0 };
    if el < tip && el >= r {
        return hex(0x0f2a1e) * 0.8;
    }
    if el < r {
        let k = ((r - el) / 0.03).clamp(0.0, 1.0);
        return hex(0x1e2340).lerp(hex(0x141428), k);
    }
    let m = MOON.normalize();
    let a = dir.angle_between(m);
    let base = hex(0x1e2340).lerp(hex(0x0b0b14), ((el - r) / 0.16).clamp(0.0, 1.0));
    if a < 0.026 {
        let rm = m.cross(Vec3::Y).normalize();
        let um = rm.cross(m);
        let (x, y) = (dir.dot(rm) / 0.026, dir.dot(um) / 0.026);
        let mut c = hex(0xfff0a8).lerp(hex(0xfffaf0), 0.5);
        for (cx, cy, cr) in [(0.3, 0.25, 0.28), (-0.35, -0.1, 0.22), (0.1, -0.5, 0.16), (-0.2, 0.5, 0.12)] {
            if (x - cx).hypot(y - cy) < cr {
                c = hex(0xe8d0a8);
            }
        }
        // Shaded limb on one side.
        if (x + 0.35).hypot(y - 0.2) > 1.15 {
            c = hex(0xe8d0a8) * 0.9;
        }
        return c;
    }
    if a < 0.07 {
        let g = 1.0 - (a - 0.026) / 0.044;
        return base.lerp(hex(0x5a74b0), 0.45 * g * g);
    }
    let (ci, cj) = ((az / STAR_PATCH).floor() as i32, (el / STAR_PATCH).floor() as i32);
    let h = hash2(ci + 31, cj - 77);
    if h > 0.6 && el > r + 0.01 {
        let (ox, oy) = (hash2(ci - 5, cj + 9) - 0.5, hash2(ci + 13, cj - 3) - 0.5);
        let sx = (ci as f32 + 0.5 + ox * 0.6) * STAR_PATCH;
        let sy = (cj as f32 + 0.5 + oy * 0.6) * STAR_PATCH;
        let (dx, dy) = ((az - sx).abs(), (el - sy).abs());
        let k = hash2(ci, cj);
        let tw = 0.7 + 0.3 * (time * (1.5 + 2.0 * k) + h * 60.0).sin();
        if dx < 0.0026 && dy < 0.0026 {
            return hex(0xc8d8f0) * tw;
        }
        // Bright stars get a little cross.
        if k > 0.8 && ((dx < 0.008 && dy < 0.0024) || (dy < 0.008 && dx < 0.0024)) {
            return hex(0x8aa6d8) * tw;
        }
    }
    base
}

// ---- one pixel of the 3D scene --------------------------------------------

fn shade_px(scene: &Scene, cam: &Cam, x: f32, y: f32, fighters_only: bool) -> Px {
    let rd = cam.ray(x + 0.5, y + 0.5);
    let hit = scene.march(cam.pos, rd);
    let owner = Scene::owner(hit.id);
    if hit.id == SKY || hit.t >= MAX_DIST {
        return Px { rgb: sky_px(rd, scene.time), id: SKY, mat: Mat::Sky, t: MAX_DIST, owner: None, clear: fighters_only };
    }
    let keep = owner.is_some() || matches!(hit.mat, Mat::Orb) || (matches!(hit.mat, Mat::Fire | Mat::Wood) && (cam.pos + rd * hit.t - scene.fire).length() < 1.2);
    let sh = scene.shade(cam.pos, rd, &hit);
    let mut rgb = sh.rgb;
    let p = sh.p;
    match hit.mat {
        Mat::Grass => {
            let centre = (scene.bodies[0].pos + scene.bodies[1].pos) * 0.5;
            let dist = vec3(p.x - centre.x, 0.0, p.z - centre.z).length();
            let h = hash2((p.x * 14.0).floor() as i32, (p.z * 22.0).floor() as i32);
            let tone = if h < 0.2 { 0.6 } else if h < 0.75 { 0.9 } else { 1.2 };
            let near = (((p - cam.pos).length() - 2.5) / 3.5).clamp(0.0, 1.0);
            rgb *= grass_light(dist) * tone * 0.75 * (0.6 + 0.4 * near);
            if h > 0.992 {
                rgb = hex(0xffb848) * (sh.lum * 2.5).clamp(0.3, 1.0);
            }
        }
        Mat::Stone => {
            let h = hash2((p.x * 8.0 + p.z * 3.0).floor() as i32, (p.y * 8.0).floor() as i32);
            rgb *= 0.85 + 0.3 * h;
        }
        Mat::Leaf => {
            let h = hash2((p.x * 10.0).floor() as i32, (p.y * 10.0 + p.z * 6.0).floor() as i32);
            rgb *= 0.75 + 0.5 * h;
        }
        Mat::Fire | Mat::Orb => rgb *= 1.3,
        Mat::Blade => {
            let l = lum(rgb).max(1e-3);
            rgb *= (0.75 / l).max(1.0);
        }
        _ => {}
    }
    if fighters_only && owner.is_some() {
        rgb = rgb * 0.6 + rgb.normalize_or_zero() * 0.35;
    }
    if let Some(b) = owner.and_then(|i| scene.bodies.get(i)) {
        rgb = flash_rgb(rgb, b.flash, false) * (1.0 - b.dim);
    }
    Px { rgb, id: hit.id, mat: hit.mat, t: hit.t, owner, clear: fighters_only && !keep }
}

// ---- 2D backdrop (sprites style) ------------------------------------------

fn backdrop(w: usize, h: usize, ground_y: f32, time: f32) -> Vec<Vec3> {
    let mut out = vec![Vec3::ZERO; w * h];
    let horizon = ground_y - 6.0;
    for y in 0..h {
        for x in 0..w {
            let (fx, fy) = (x as f32, y as f32);
            // Sky in hard bands, the classic way.
            let band = ((horizon - fy) / 14.0).floor().clamp(0.0, 4.0) as usize;
            let mut c = [hex(0x2b3560), hex(0x1e2340), hex(0x141428), hex(0x141428), hex(0x0b0b14)][band];
            // Moon.
            let (mx, my) = (w as f32 * 0.8, 30.0);
            let d = (fx - mx).hypot(fy - my);
            if d < 30.0 && band < 4 {
                c = c.lerp(hex(0x3d4f86), 0.5 * (1.0 - d / 30.0).powf(0.5).min(1.0) * ((d < 20.0) as u8 as f32 * 0.4 + 0.6));
            }
            let hs = hash2(x as i32 * 3 + 1, y as i32 * 7 - 4);
            if hs > 0.994 && fy < horizon - 20.0 {
                let tw = 0.7 + 0.3 * (time * 2.0 + hs * 900.0).sin();
                c = hex(0xc8d8f0) * tw;
            }
            if d < 11.0 {
                c = hex(0xfffaf0).lerp(hex(0xfff0a8), 0.4);
                for (cx, cy, cr) in [(3.0, -3.0, 3.0), (-4.0, 1.0, 2.5), (1.0, 5.0, 1.8)] {
                    if (fx - mx - cx).hypot(fy - my - cy) < cr {
                        c = hex(0xe8d0a8);
                    }
                }
            }
            // Far mountains, then nearer hills with pines, then the meadow.
            let tri = |v: f32| 1.0 - (2.0 * v.fract() - 1.0).abs();
            let far = horizon - 14.0 - 26.0 * tri(fx / 90.0 + 0.3) * (0.6 + 0.4 * (fx * 0.013).sin()) - 8.0 * tri(fx / 31.0);
            if fy > far {
                c = hex(0x1e2340);
                // Snow caps.
                if fy < far + 3.0 && far < horizon - 30.0 {
                    c = hex(0x3d4f86);
                }
            }
            let hill = horizon - 8.0 - 5.0 * (fx * 0.045 + 2.0).sin() - 3.0 * (fx * 0.11).sin();
            let u = fx / 7.0;
            let ti = u.floor() as i32;
            let tf = u.fract();
            let th = hash2(ti, 3) * 12.0 + 4.0;
            let tree_top = hill - th * (1.0 - (2.0 * tf - 1.0).abs()) * (hash2(ti, 9) > 0.2) as u8 as f32;
            if fy > tree_top {
                c = if fy > hill { hex(0x0f2a1e) } else { hex(0x0f2a1e) * 0.75 };
                // Moonlit right edge of each pine.
                if fy <= hill && tf > 0.5 && tf < 0.62 {
                    c = hex(0x1c4430);
                }
            }
            if fy >= horizon {
                // Meadow rows: darker far, brighter near, with tufts.
                let row = fy - horizon;
                let base = if row < 10.0 { hex(0x0f2a1e) } else if row < 30.0 { hex(0x1c4430) } else { hex(0x0f2a1e) };
                c = base;
                let tuft = hash2((fx / 3.0).floor() as i32, (fy / 2.0).floor() as i32);
                if tuft > 0.8 && ((x + y) % 3 == 0) {
                    c = if row < 30.0 { hex(0x2e6640) } else { hex(0x1c4430) };
                }
                if tuft < 0.08 {
                    c = hex(0x0f2a1e) * 0.7;
                }
                // A worn dirt strip where they fight.
                let path = (fy - ground_y).abs() < 5.0 + 1.5 * (fx * 0.2).sin();
                if path {
                    c = if (x * 7 + y * 3) % 11 == 0 { hex(0x7a4a2c) } else { hex(0x4a2e20) };
                    if fy < ground_y - 3.0 {
                        c = hex(0x2a1a14);
                    }
                }
                if tuft > 0.996 {
                    c = hex(0xffb848);
                }
            }
            out[y * w + x] = c;
        }
    }
    out
}

// ---- main entry -------------------------------------------------------------

pub fn render(style: Style, scene: &Scene, fx: &Fx, eye: Vec3, target: Vec3, w: usize, h: usize) -> Image {
    let (eye, target, fov) = match style {
        // Fixed side view, long lens so the fighters look flat like sprites.
        Style::Sprites => {
            let t = (scene.bodies[0].pos + scene.bodies[1].pos) * 0.5 + vec3(0.0, 1.0, 0.0);
            (t + vec3(0.0, 0.7, 14.0), t, 22.0)
        }
        _ => (eye, target, 50.0),
    };
    let cam = Cam::look_at(eye, target, fov, w as f32, h as f32);
    let fighters_only = style == Style::Sprites;
    let mut px: Vec<Px> = (0..w * h).into_par_iter().map(|i| shade_px(scene, &cam, (i % w) as f32, (i / w) as f32, fighters_only)).collect();

    // Particles as single pixels, depth-tested.
    for p in &fx.particles {
        let Some((sp, depth)) = cam.project(p.p) else { continue };
        let (x, y) = (sp.x as isize, sp.y as isize);
        if x < 0 || y < 0 || x >= w as isize || y >= h as isize {
            continue;
        }
        let i = y as usize * w + x as usize;
        if depth > px[i].t + 0.2 && !px[i].clear {
            continue;
        }
        let k = (p.life / p.max).clamp(0.0, 1.0);
        let rgb = match p.kind {
            Kind::Smoke => p.rgb * 0.6 * k,
            _ => p.rgb * (0.6 + 0.8 * k),
        };
        px[i] = Px { rgb, id: 999, mat: Mat::Fire, t: depth, owner: None, clear: false };
        if matches!(p.kind, Kind::Spark | Kind::Sparkle) && k > 0.5 && x + 1 < w as isize {
            px[i + 1] = px[i];
        }
    }
    // Shield rim: a ring of pixels.
    for i in 0..2 {
        if fx.ward[i] <= 0.0 {
            continue;
        }
        let c = scene.bodies[i].to_world(scene.bodies[i].anchors.chest);
        for k in 0..120 {
            let a = k as f32 / 120.0 * std::f32::consts::TAU;
            let p = c + (cam.right * a.cos() * 0.7 + cam.up * a.sin() * 0.85) * fx.ward[i];
            if let Some((sp, _)) = cam.project(p) {
                let (x, y) = (sp.x as usize, sp.y as usize);
                if x < w && y < h {
                    px[y * w + x] = Px { rgb: hex(0x58c8e8), id: 998, mat: Mat::Crystal, t: 0.0, owner: None, clear: false };
                }
            }
        }
    }

    // Glow: soft light around emissive pixels (dither turns it into rings).
    let mut glow = vec![Vec3::ZERO; w * h];
    for (i, p) in px.iter().enumerate() {
        if p.clear || !matches!(p.mat, Mat::Fire | Mat::Orb | Mat::Crystal) {
            continue;
        }
        let (x, y) = ((i % w) as isize, (i / w) as isize);
        let s = if p.mat == Mat::Orb { 0.03 } else { 0.012 };
        for dy in -7..=7isize {
            for dx in -7..=7isize {
                let (nx, ny) = (x + dx, y + dy);
                if nx < 0 || ny < 0 || nx >= w as isize || ny >= h as isize {
                    continue;
                }
                let d2 = (dx * dx + dy * dy) as f32;
                if d2 > 49.0 {
                    continue;
                }
                glow[ny as usize * w + nx as usize] += vec3(1.0, 0.55, 0.2) * s * (1.0 - d2 / 49.0);
            }
        }
    }

    // Inner lines: a part sitting behind a nearer part of the same fighter darkens.
    let n4 = |i: usize| -> [Option<usize>; 4] {
        let (x, y) = (i % w, i / w);
        [(x > 0).then(|| i - 1), (x + 1 < w).then(|| i + 1), (y > 0).then(|| i - w), (y + 1 < h).then(|| i + w)]
    };
    let mut rgb: Vec<Vec3> = px.iter().zip(&glow).map(|(p, g)| p.rgb + *g).collect();
    for i in 0..w * h {
        let p = px[i];
        if p.owner.is_none() {
            continue;
        }
        if n4(i).iter().flatten().any(|&j| px[j].owner == p.owner && px[j].id != p.id && px[j].t < p.t - 0.03) {
            rgb[i] *= 0.55;
        }
    }

    // Backdrop for the sprites style.
    let bg = (style == Style::Sprites).then(|| {
        let feet = scene.bodies.iter().filter_map(|b| cam.project(b.pos)).map(|(s, _)| s.y).sum::<f32>() / 2.0;
        let mut bg = backdrop(w, h, feet, scene.time);
        // Drop shadows under the fighters.
        for b in &scene.bodies {
            if let Some((s, _)) = cam.project(b.to_world(Vec3::ZERO)) {
                for y in 0..h {
                    for x in 0..w {
                        let (dx, dy) = ((x as f32 - s.x) / 9.0, (y as f32 - s.y) / 2.2);
                        if dx * dx + dy * dy < 1.0 {
                            bg[y * w + x] *= 0.45;
                        }
                    }
                }
            }
        }
        bg
    });

    // Dither and snap to the palette.
    let mut out = vec![Vec3::ZERO; w * h];
    for i in 0..w * h {
        let (x, y) = (i % w, i / w);
        let b = bayer(x, y);
        out[i] = if px[i].clear {
            bg.as_ref().map_or(Vec3::ZERO, |bg| bg[i])
        } else {
            match style {
                Style::Mono => {
                    let gain = if px[i].owner.is_some() || matches!(px[i].mat, Mat::Fire | Mat::Orb | Mat::Crystal) { 1.5 } else if px[i].mat == Mat::Sky { 0.9 } else { 0.95 };
                    let l = (lum(rgb[i]).max(0.0).powf(0.65) * gain + b * 0.22).clamp(0.0, 0.999);
                    MONO[(l * 4.0) as usize]
                }
                _ => nearest(&PALETTE, rgb[i] + Vec3::splat(b * 0.09)),
            }
        };
        // Sprites style: leave the backdrop pixels un-dithered (already palette colours).
        if px[i].clear && bg.is_none() {
            out[i] = nearest(&PALETTE, rgb[i]);
        }
    }

    // Silhouette outline: a pixel just outside a fighter (and behind it) goes near-black.
    let ink = if style == Style::Mono { MONO[0] } else { PALETTE[0] };
    let mut final_ = out.clone();
    for i in 0..w * h {
        let p = px[i];
        let edge = n4(i).iter().flatten().any(|&j| px[j].owner.is_some() && px[j].owner != p.owner && (p.clear || px[j].t < p.t - 0.1));
        if edge {
            final_[i] = ink;
        }
    }

    let mut img = Image::gen_image_color(w as u16, h as u16, BLACK);
    for i in 0..w * h {
        let c = final_[i];
        img.set_pixel((i % w) as u32, (i / w) as u32, Color::new(c.x, c.y, c.z, 1.0));
    }
    img
}
