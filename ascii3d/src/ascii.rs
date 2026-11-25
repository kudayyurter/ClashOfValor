//! The ASCII layer: turns rays into a grid of glyphs.
//!
//! For every character cell we shoot an s x s grid of rays (sub-samples) and look at what
//! they hit. That gives three decisions:
//!
//! 1. Which object owns the cell (the most common part id among the sub-samples).
//! 2. Edge or fill: if that object covers only part of the cell, the cell sits on its
//!    outline. The covered sub-samples' centroid gives the outline's direction, which picks
//!    one of `- / | \ _`.
//! 3. Fill glyph: chosen per material. Most materials use a brightness ramp. Chainmail and
//!    cloth use a pattern laid out in the object's own surface coordinates, so the rows of
//!    rings wrap around the body instead of lining up with the screen.

use crate::scene::{Hit, MAX_DIST, METALS, Mat, SKY, Scene, Shade, hash2};
use macroquad::prelude::{Color, Vec2, Vec3, vec2, vec3};
use std::f32::consts::{PI, TAU};

pub struct Cam {
    pub pos: Vec3,
    fwd: Vec3,
    pub right: Vec3,
    up: Vec3,
    tan_half: f32,
    aspect: f32,
    w: f32,
    h: f32,
}

impl Cam {
    pub fn look_at(pos: Vec3, target: Vec3, fov_y_deg: f32, w: f32, h: f32) -> Self {
        let fwd = (target - pos).normalize();
        let right = fwd.cross(Vec3::Y).normalize();
        let up = right.cross(fwd);
        Cam { pos, fwd, right, up, tan_half: (fov_y_deg.to_radians() * 0.5).tan(), aspect: w / h, w, h }
    }

    /// Direction of the ray through screen pixel (px, py).
    pub fn ray(&self, px: f32, py: f32) -> Vec3 {
        let x = (2.0 * px / self.w - 1.0) * self.tan_half * self.aspect;
        let y = (1.0 - 2.0 * py / self.h) * self.tan_half;
        (self.fwd + self.right * x + self.up * y).normalize()
    }

    /// World point -> (screen pixel, depth).
    pub fn project(&self, p: Vec3) -> Option<(Vec2, f32)> {
        let v = p - self.pos;
        let z = v.dot(self.fwd);
        if z < 0.1 {
            return None;
        }
        let x = v.dot(self.right) / (z * self.tan_half * self.aspect);
        let y = v.dot(self.up) / (z * self.tan_half);
        Some((vec2((x + 1.0) * 0.5 * self.w, (1.0 - y) * 0.5 * self.h), z))
    }
}

#[derive(Clone, Copy)]
pub struct Cell {
    pub glyph: char,
    pub color: Color,
    pub depth: f32,
    pub glow: f32,
}

#[derive(Clone, Copy)]
pub enum Pass {
    /// Big grid for the world. Cells that touch the hero are handed to the fine pass.
    Coarse,
    /// Half-size grid that only draws the hero.
    Fine,
}

pub enum CellOut {
    Empty(f32),
    Glyph(Cell),
    Defer,
}

pub struct Opts {
    pub edges: bool,
    pub fine: bool,
}

#[allow(clippy::too_many_arguments)]
pub fn sample_cell(scene: &Scene, cam: &Cam, x0: f32, y0: f32, cw: f32, ch: f32, s: usize, pass: Pass, opts: &Opts) -> CellOut {
    let n = s * s;
    let mut ids = [SKY; 16];
    let mut mats = [Mat::Sky; 16];
    let mut ts = [MAX_DIST; 16];
    let mut hero_n = 0;
    let sub = |k: usize| {
        let (sx, sy) = (k % s, k / s);
        (x0 + (sx as f32 + 0.5) / s as f32 * cw, y0 + (sy as f32 + 0.5) / s as f32 * ch)
    };
    for k in 0..n {
        let (px, py) = sub(k);
        let hit = scene.march(cam.pos, cam.ray(px, py));
        ids[k] = hit.id;
        mats[k] = hit.mat;
        ts[k] = hit.t;
        if hit.mat.is_hero() {
            hero_n += 1;
        }
    }
    if matches!(pass, Pass::Coarse) && opts.fine && hero_n > 0 {
        return CellOut::Defer;
    }

    // 1. Which object owns this cell?
    let mut best_id = SKY;
    let mut best_c = 0;
    for k in 0..n {
        let id = ids[k];
        if id == SKY || (matches!(pass, Pass::Fine) && !mats[k].is_hero()) {
            continue;
        }
        let c = ids[..n].iter().filter(|&&j| j == id).count();
        if c > best_c {
            best_c = c;
            best_id = id;
        }
    }
    let nearest = ts[..n].iter().copied().fold(MAX_DIST, f32::min);
    let frac = best_c as f32 / n as f32;
    if best_id == SKY || frac < 0.2 {
        return CellOut::Empty(nearest);
    }

    // 2. Coverage centroid (y up) and the covered sub-sample nearest the cell centre.
    let mid = (s as f32 - 1.0) * 0.5;
    let (mut gx, mut gy) = (0.0f32, 0.0f32);
    let (mut rep, mut rep_d2) = (0, f32::MAX);
    for k in 0..n {
        if ids[k] != best_id {
            continue;
        }
        let dx = (k % s) as f32 - mid;
        let dy = mid - (k / s) as f32;
        gx += dx;
        gy += dy;
        let d2 = dx * dx + dy * dy;
        if d2 < rep_d2 {
            rep_d2 = d2;
            rep = k;
        }
    }
    let (px, py) = sub(rep);
    let rd = cam.ray(px, py);
    let hit = Hit { t: ts[rep], id: best_id, mat: mats[rep] };
    let sh = scene.shade(cam.pos, rd, &hit);

    let is_edge = opts.edges && frac < 0.85 && gx * gx + gy * gy > 0.01 && hit.mat != Mat::Fire;
    let (glyph, rgb) = if is_edge {
        (edge_glyph(gx, gy, frac), sh.rgb * 1.2)
    } else {
        fill_glyph(scene, cam, &hit, &sh)
    };
    if glyph == ' ' {
        return CellOut::Empty(hit.t);
    }
    let glow = match hit.mat {
        Mat::Fire => 1.0,
        Mat::Blade if matches!(METALS[scene.metal].name, "Moonsteel" | "Emberstone") => 0.5,
        _ => 0.0,
    };
    CellOut::Glyph(Cell { glyph, color: to_color(rgb), depth: hit.t, glow })
}

/// Pick an outline glyph from the direction of the coverage centroid.
/// (gx, gy) points from the cell centre towards the covered side; the outline is
/// perpendicular to it.
pub fn edge_glyph(gx: f32, gy: f32, frac: f32) -> char {
    let mut a = gx.atan2(-gy); // angle of the outline, in [-pi, pi]
    if a < 0.0 {
        a += PI;
    }
    let eighths = a / PI * 8.0;
    if !(1.0..7.0).contains(&eighths) {
        // Horizontal. A thin sliver of object at the bottom of the cell reads better as '_'.
        if gy < 0.0 && frac < 0.5 { '_' } else { '-' }
    } else if eighths < 3.0 {
        '/'
    } else if eighths < 5.0 {
        '|'
    } else {
        '\\'
    }
}

fn ramp(chars: &[char], x: f32) -> char {
    chars[((x.clamp(0.0, 0.999)) * chars.len() as f32) as usize]
}

fn fill_glyph(scene: &Scene, cam: &Cam, hit: &Hit, sh: &Shade) -> (char, Vec3) {
    let l = sh.lum;
    match hit.mat {
        Mat::Grass => grass_glyph(scene, cam, sh),
        Mat::Stone => (ramp(&['.', ':', 'o', 'd', 'Q', 'B'], l * 1.8), sh.rgb),
        Mat::Leaf => (ramp(&['.', ':', ';', 'd', 'U', 'Q'], l * 2.2), sh.rgb),
        Mat::Wood => (ramp(&['.', ':', ')', '|', ']'], l * 2.5), sh.rgb),
        Mat::Fire => {
            let h = hash2((sh.p.y * 9.0 + scene.time * 10.0) as i32, (sh.p.x * 9.0 + sh.p.z * 7.0) as i32);
            (ramp(&['\'', '^', '*', '"', '^'], h), sh.rgb * 1.3)
        }
        Mat::Plate | Mat::Blade => {
            let [bright, mid, dark] = METALS[scene.metal].glyphs;
            let g = if l > 0.42 {
                bright
            } else if l > 0.2 {
                mid
            } else if l > 0.05 {
                dark
            } else {
                ' '
            };
            (g, sh.rgb)
        }
        Mat::Chain => chain_glyph(scene, hit, sh),
        Mat::Cloth => cloth_glyph(scene, hit, sh),
        Mat::Sky => (' ', Vec3::ZERO),
    }
}

/// Grass: each cell is a blade whose lean comes from a wind field plus a push away from
/// the player. Hashing on world position (not screen position) keeps the sparse
/// cells and flowers from swimming when the camera moves.
fn grass_glyph(scene: &Scene, cam: &Cam, sh: &Shade) -> (char, Vec3) {
    let p = sh.p;
    // Small hash cells so a flower or a gap covers about one glyph, not a clump.
    let h = hash2((p.x * 20.0).floor() as i32, (p.z * 20.0).floor() as i32);
    let t = scene.time;
    let mut lean = (p.x * 0.6 + t * 1.7 + (p.z * 0.4).sin()).sin() * 0.5 + (p.z * 1.3 - t * 1.1).sin() * 0.25;
    let d = vec3(p.x - scene.player.x, 0.0, p.z - scene.player.z);
    let dl = d.length();
    if dl < 1.4 && dl > 1e-3 {
        lean += d.dot(cam.right) / dl * (1.4 - dl) * 2.0;
    }
    if h > 0.985 {
        let flower = vec3(0.6, 0.75, 1.0) * (sh.lum * 2.5).min(1.0);
        return (if h > 0.993 { 'o' } else { '*' }, flower);
    }
    let g = if h < 0.06 {
        '.'
    } else if h < 0.12 {
        ','
    } else if lean > 0.3 {
        '/'
    } else if lean < -0.3 {
        '\\'
    } else {
        '|'
    };
    (g, sh.rgb)
}

/// The surface coordinates of the hit, in the hero part's own frame.
fn hero_uv(scene: &Scene, hit: &Hit, sh: &Shade) -> (f32, f32) {
    match scene.hero_part(hit.id) {
        Some(part) => part.shape.surface_uv(scene.to_local(sh.p - scene.player)),
        None => (0.0, 0.0),
    }
}

/// Chainmail: rows of rings that follow the surface ("custom grid and symbols").
/// Alternating columns swap glyphs so neighbouring rings interlock.
fn chain_glyph(scene: &Scene, hit: &Hit, sh: &Shade) -> (char, Vec3) {
    let (u, v) = hero_uv(scene, hit, sh);
    let row = (v * 22.0).floor();
    let col = (u / TAU * 16.0 + row * 0.5).floor();
    let k = ((row + col) as i32).rem_euclid(2) as usize;
    let l = sh.lum;
    let g = if l > 0.38 {
        ['S', '8'][k]
    } else if l > 0.18 {
        ['3', 'S'][k]
    } else if l > 0.05 {
        ['(', 'c'][k]
    } else {
        ':'
    };
    (g, sh.rgb)
}

/// Cloth: a blue fill with gold '$' ornaments stitched in surface space.
fn cloth_glyph(scene: &Scene, hit: &Hit, sh: &Shade) -> (char, Vec3) {
    let (u, v) = hero_uv(scene, hit, sh);
    let row = (v * 14.0).floor() as i32;
    let col = (u * 2.0).floor() as i32;
    if row.rem_euclid(2) == 0 && (row + col).rem_euclid(4) == 0 {
        return ('$', vec3(1.0, 0.78, 0.3) * (0.3 + sh.lum * 2.0).min(1.2));
    }
    (ramp(&['.', '-', '=', '%'], sh.lum * 2.5), sh.rgb)
}

/// Thin glyphs on a dark background look dimmer than the colour they are drawn in, so
/// lift the darks with a gamma curve.
fn to_color(rgb: Vec3) -> Color {
    let c = rgb.max(Vec3::ZERO).powf(0.6).min(Vec3::ONE);
    Color::new(c.x, c.y, c.z, 1.0)
}
