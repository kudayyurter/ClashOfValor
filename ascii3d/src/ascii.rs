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
    pub up: Vec3,
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
    /// Which fighters draw on the fine grid.
    pub fine_mask: [bool; 2],
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
        if is_fine(hit.id, hit.mat, opts) {
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
        if id == SKY || (matches!(pass, Pass::Fine) && !is_fine(id, mats[k], opts)) {
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
    if best_id == SKY && nearest >= MAX_DIST && matches!(pass, Pass::Coarse) {
        // Nothing here but sky: hills, the moon, or a star.
        if let Some((glyph, rgb)) = sky_glyph(cam.ray(x0 + cw * 0.5, y0 + ch * 0.5), scene.time) {
            if glyph == ' ' {
                return CellOut::Empty(MAX_DIST);
            }
            return CellOut::Glyph(Cell { glyph, color: to_color(rgb), depth: MAX_DIST, glow: 0.0 });
        }
    }
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

    let is_edge = opts.edges && frac < 0.85 && gx * gx + gy * gy > 0.01 && !hit.mat.is_emissive();
    let (glyph, rgb) = if is_edge {
        (edge_glyph(gx, gy, frac), sh.rgb * 1.2)
    } else {
        fill_glyph(scene, cam, &hit, &sh)
    };
    if glyph == ' ' {
        return CellOut::Empty(hit.t);
    }
    let body = Scene::owner(best_id).and_then(|i| scene.bodies.get(i));
    let mut rgb = rgb;
    if let Some(b) = body {
        if is_edge && hit.mat.is_hero() {
            // Fighter outlines: brighter, tinted with the player's colour.
            rgb = rgb.lerp(b.accent, 0.35);
            let lum = rgb.dot(vec3(0.3, 0.59, 0.11));
            if lum < 0.45 {
                rgb *= (0.45 / lum.max(0.05)).min(4.0);
            }
        }
        rgb = flash_rgb(rgb, b.flash);
    }
    let glow = match hit.mat {
        Mat::Fire | Mat::Orb => 1.0,
        Mat::Crystal => 0.3,
        Mat::Blade if matches!(METALS[body.map_or(0, |b| b.metal)].name, "Moonsteel" | "Emberstone") => 0.5,
        _ => 0.0,
    };
    let fade = 1.0 - body.map_or(0.0, |b| b.dim);
    CellOut::Glyph(Cell { glyph, color: to_color(rgb * fade), depth: hit.t, glow: glow * fade })
}

/// Where the decorative moon hangs: behind the fighters, centred between the status panels.
pub const MOON: Vec3 = Vec3::new(-0.02, 0.193, -0.981);

/// Glyph for a ray that hits nothing: hills on the horizon, the moon, or stars.
/// `None` leaves the cell empty.
pub fn sky_glyph(dir: Vec3, time: f32) -> Option<(char, Vec3)> {
    let el = dir.y.clamp(-1.0, 1.0).asin();
    let az = dir.z.atan2(dir.x);
    // Horizon ridge, in radians of elevation. The camera only sees about 0.14 rad of
    // sky, so the hills stay low.
    let ridge = |a: f32| 0.016 + 0.01 * ((a * 7.0).sin() + 0.6 * (a * 17.0 + 1.3).sin() + 0.3 * (a * 41.0).sin()) / 1.9;
    let r = ridge(az);
    if el < r {
        let col = (az * 400.0).floor() as i32;
        if el > r - 0.007 {
            // A ragged treeline along the crest.
            let h = hash2(col, 17);
            if h > 0.4 {
                return Some((if h > 0.8 { 'A' } else { '^' }, vec3(0.05, 0.13, 0.1)));
            }
        }
        if el > r - 0.004 {
            let slope = (ridge(az + 0.003) - ridge(az - 0.003)) / 0.006;
            let g = if slope > 0.15 {
                '\\'
            } else if slope < -0.15 {
                '/'
            } else {
                '_'
            };
            return Some((g, vec3(0.1, 0.12, 0.2)));
        }
        // Sparse texture on the hillside; mostly a dark silhouette against the stars.
        let h = hash2(col, (el * 400.0).floor() as i32);
        return Some((if h > 0.88 { '.' } else { ' ' }, vec3(0.07, 0.08, 0.14)));
    }
    let a = dir.angle_between(MOON.normalize());
    if a < 0.035 {
        let g = if a < 0.017 { '●' } else if a < 0.028 { 'O' } else { 'o' };
        return Some((g, vec3(0.95, 0.95, 0.82)));
    }
    if a < 0.08 {
        let h = hash2((az * 300.0).floor() as i32, (el * 300.0).floor() as i32 + 5);
        return (h > 0.55).then_some(('.', vec3(0.5, 0.52, 0.5) * (1.0 - (a - 0.035) / 0.045)));
    }
    if el < r + 0.012 {
        return None;
    }
    // Stars: a sparse hash on the world direction, so they hold still as the camera turns.
    let (ci, cj) = ((az / 0.012).floor() as i32, (el / 0.012).floor() as i32);
    let h = hash2(ci + 31, cj - 77);
    if h < 0.98 {
        return None;
    }
    let k = hash2(ci, cj);
    let g = if k > 0.85 { '+' } else if k > 0.55 { '*' } else { '.' };
    let tw = 0.55 + 0.45 * (time * (1.5 + 2.0 * k) + h * 60.0).sin();
    Some((g, vec3(0.75, 0.8, 1.0) * tw))
}

/// Hit flash: lerp a glyph colour toward white.
pub fn flash_rgb(rgb: Vec3, flash: f32) -> Vec3 {
    rgb.lerp(Vec3::splat(1.2), flash.clamp(0.0, 1.0))
}

/// Does this hit belong on the fine grid: a hero material of a masked fighter.
fn is_fine(id: u16, mat: Mat, opts: &Opts) -> bool {
    mat.is_hero() && Scene::owner(id).is_some_and(|i| opts.fine_mask[i])
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
        Mat::Stone => {
            // Shading blocks with the odd crack, so rocks read as stone, not letters.
            let h = hash2((sh.p.x * 6.0 + sh.p.z * 2.0).floor() as i32, (sh.p.y * 6.0).floor() as i32);
            let g = if h > 0.89 && l > 0.08 {
                ['/', '\\', '_'][((h - 0.89) * 27.0) as usize % 3]
            } else {
                ramp(&['.', ':', '-', '=', '#', '░'], l * 1.8)
            };
            (g, sh.rgb)
        }
        Mat::Leaf => {
            let h = hash2((sh.p.x * 9.0).floor() as i32, (sh.p.y * 9.0 + sh.p.z * 5.0).floor() as i32);
            let g = if h > 0.9 { if h > 0.95 { '"' } else { '\'' } } else { ramp(&['.', ',', ';', '*', '%', '&'], l * 2.2) };
            (g, sh.rgb)
        }
        Mat::Wood => (ramp(&['.', ':', ')', '|', ']'], l * 2.5), sh.rgb),
        Mat::Fire => {
            let h = hash2((sh.p.y * 9.0 + scene.time * 10.0) as i32, (sh.p.x * 9.0 + sh.p.z * 7.0) as i32);
            (ramp(&['\'', '^', '*', '"', '^'], h), sh.rgb * 1.3)
        }
        Mat::Plate | Mat::Blade => {
            let metal = Scene::owner(hit.id).and_then(|i| scene.bodies.get(i)).map_or(0, |b| b.metal);
            let [bright, mid, dark] = METALS[metal].glyphs;
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
        Mat::Robe => robe_glyph(scene, hit, sh),
        Mat::Leather => leather_glyph(scene, hit, sh),
        Mat::Cloak => (ramp(&['.', ':', ';', '%', '#'], sh.lum * 2.4), sh.rgb),
        Mat::Crystal => {
            let h = hash2((sh.p.y * 40.0 + scene.time * 6.0) as i32, (sh.p.x * 40.0 + sh.p.z * 33.0) as i32);
            (ramp(&['+', '*', 'o', '*'], h), sh.rgb * 1.2)
        }
        Mat::Orb => {
            let h = hash2((sh.p.y * 25.0 + scene.time * 14.0) as i32, (sh.p.x * 25.0 + sh.p.z * 19.0) as i32);
            (ramp(&['o', '*', '@', '*'], h), sh.rgb * 1.3)
        }
        Mat::Sky => (' ', Vec3::ZERO),
    }
}

/// Chance that a grass cell far from the duel still draws a blade.
pub fn grass_keep(dist: f32) -> f32 {
    let f = ((dist - 3.0) / 9.0).clamp(0.0, 1.0);
    let f = f * f * (3.0 - 2.0 * f);
    let far = (1.0 - 0.65 * f).max(0.35);
    // Beyond 12 m keep easing down to the floor.
    (far - ((dist - 12.0).max(0.0) * 0.004)).max(0.25)
}

/// Grass brightness by distance from the duel, so the fighters stand out.
pub fn grass_light(dist: f32) -> f32 {
    let f = ((dist - 3.0) / 9.0).clamp(0.0, 1.0);
    0.85 - 0.4 * f * f * (3.0 - 2.0 * f)
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
    for b in &scene.bodies {
        let feet = b.to_world(Vec3::ZERO);
        let d = vec3(p.x - feet.x, 0.0, p.z - feet.z);
        let dl = d.length();
        if dl < 1.4 && dl > 1e-3 {
            lean += d.dot(cam.right) / dl * (1.4 - dl) * 2.0;
        }
    }
    // Thin and darken the grass away from the duel so the fighters stand out.
    let centre = match scene.bodies.as_slice() {
        [a, b, ..] => (a.pos + b.pos) * 0.5,
        _ => Vec3::ZERO,
    };
    let dist = vec3(p.x - centre.x, 0.0, p.z - centre.z).length();
    let light = grass_light(dist);
    let h2 = hash2((p.x * 20.0).floor() as i32 + 7919, (p.z * 20.0).floor() as i32 - 104_729);
    if h2 > grass_keep(dist) {
        return (if h2 > 0.985 || h < 0.35 { '.' } else { ' ' }, sh.rgb * light * 0.8);
    }
    if h > 0.985 {
        let flower = vec3(0.6, 0.75, 1.0) * (sh.lum * 2.5).min(1.0) * light;
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
    (g, sh.rgb * light)
}

/// The surface coordinates of the hit, in the hero part's own frame.
fn hero_uv(scene: &Scene, hit: &Hit, sh: &Shade) -> (f32, f32) {
    match scene.body_part(hit.id) {
        Some((body, part)) => part.shape.surface_uv(body.to_local(sh.p)),
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

/// Robe: deep purple with rows of stars and runes, and a hem in the player's accent.
fn robe_glyph(scene: &Scene, hit: &Hit, sh: &Shade) -> (char, Vec3) {
    let (u, v) = hero_uv(scene, hit, sh);
    let accent = scene.body_part(hit.id).map_or(Vec3::ONE, |(b, _)| b.accent);
    let local_y = scene.body_part(hit.id).map_or(1.0, |(b, _)| b.to_local(sh.p).y);
    if local_y < 0.16 {
        return (ramp(&['-', '=', '#'], sh.lum * 2.2), accent * (0.35 + sh.lum * 1.6).min(1.2));
    }
    let row = (v * 9.0).floor() as i32;
    let col = (u * 2.5).floor() as i32;
    if row.rem_euclid(3) == 0 && (row + col).rem_euclid(3) == 0 {
        let rune = ['*', '+', 'x', '%'][(row + col).rem_euclid(4) as usize];
        return (rune, vec3(0.95, 0.85, 1.0) * (0.3 + sh.lum * 2.0).min(1.1));
    }
    (ramp(&['.', ':', '~', '='], sh.lum * 2.5), sh.rgb)
}

/// Leather: wrapped bands that cross the limbs and torso.
fn leather_glyph(scene: &Scene, hit: &Hit, sh: &Shade) -> (char, Vec3) {
    let (u, v) = hero_uv(scene, hit, sh);
    let band = ((v * 18.0) + (u / TAU * 6.0)).floor() as i32;
    let l = sh.lum;
    let g = if l < 0.05 {
        ':'
    } else if band.rem_euclid(3) == 0 {
        if l > 0.3 { 'X' } else { 'x' }
    } else {
        ramp(&['.', ';', '=', 'H'], l * 2.4)
    };
    (g, sh.rgb)
}

/// Thin glyphs on a dark background look dimmer than the colour they are drawn in, so
/// lift the darks with a gamma curve.
fn to_color(rgb: Vec3) -> Color {
    let c = rgb.max(Vec3::ZERO).powf(0.6).min(Vec3::ONE);
    Color::new(c.x, c.y, c.z, 1.0)
}

/// Decides which fighters get the fine grid: both while frames are fast, only the
/// acting fighter while they are slow.
pub struct FineGovernor {
    recent: std::collections::VecDeque<f32>,
    calm: u32,
    /// Calm frames needed to restore full detail; grows each time detail has to drop
    /// again, so a borderline machine settles instead of flickering.
    need: u32,
    pub reduced: bool,
}

impl FineGovernor {
    pub fn new() -> FineGovernor {
        FineGovernor { recent: std::collections::VecDeque::new(), calm: 0, need: 60, reduced: false }
    }

    /// Feed one frame's raymarch time; returns the fine mask for this frame.
    pub fn update(&mut self, ms: f32, active: usize) -> [bool; 2] {
        // Mean of the last 30 frames.
        self.recent.push_back(ms);
        if self.recent.len() > 30 {
            self.recent.pop_front();
        }
        let avg = self.recent.iter().sum::<f32>() / self.recent.len() as f32;
        if !self.reduced && avg > 25.0 {
            if self.calm > 0 {
                // Dropped again after a restore: wait longer next time.
                self.need = (self.need * 4).min(60 * 600);
            }
            self.reduced = true;
            self.calm = 0;
            self.recent.clear();
        } else if self.reduced {
            self.calm = if avg < 18.0 { self.calm + 1 } else { 0 };
            if self.calm >= self.need {
                self.reduced = false;
                self.recent.clear();
            }
        }
        if self.reduced { [active == 0, active == 1] } else { [true, true] }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::Class;

    #[test]
    fn governor_drops_to_active_when_slow_and_recovers() {
        let mut g = FineGovernor::new();
        assert_eq!(g.update(10.0, 0), [true, true]);
        let mut m = [true, true];
        for _ in 0..30 {
            m = g.update(32.0, 1);
        }
        assert_eq!(m, [false, true]);
        for _ in 0..59 {
            m = g.update(12.0, 0);
        }
        assert_eq!(m, [true, false], "not yet: needs 60 calm frames");
        for _ in 0..30 {
            m = g.update(12.0, 0);
        }
        assert_eq!(m, [true, true]);
    }

    #[test]
    fn mask_controls_which_fighter_defers() {
        let mut s = Scene::new();
        s.set_fighters([Class::Warrior, Class::Warrior]);
        s.update(0.016);
        let chest = s.bodies[1].to_world(vec3(0.0, 1.18, 0.0));
        let (w, h) = (200.0, 200.0);
        let cam = Cam::look_at(chest + vec3(0.0, 0.3, 4.0), chest, 50.0, w, h);
        let cell = |mask: [bool; 2]| {
            let opts = Opts { edges: true, fine: true, fine_mask: mask };
            sample_cell(&s, &cam, 96.0, 96.0, 8.0, 8.0, 3, Pass::Coarse, &opts)
        };
        assert!(matches!(cell([true, true]), CellOut::Defer));
        assert!(matches!(cell([false, true]), CellOut::Defer));
        assert!(matches!(cell([true, false]), CellOut::Glyph(_)), "unmasked fighter drawn on the coarse grid");
    }

    #[test]
    fn governor_does_not_flicker_on_a_borderline_machine() {
        // Full detail costs 26 ms, reduced detail 15 ms: right on the thresholds.
        let mut g = FineGovernor::new();
        let mut mask = [true, true];
        let mut toggles = 0;
        for _ in 0..3600 {
            let ms = if mask == [true, true] { 26.0 } else { 15.0 };
            let next = g.update(ms, 0);
            if next != mask {
                toggles += 1;
            }
            mask = next;
        }
        assert!(toggles <= 8, "{toggles} detail switches in a minute");
    }

    #[test]
    fn grass_falls_off_with_distance() {
        assert_eq!(grass_keep(0.0), 1.0);
        assert_eq!(grass_keep(2.9), 1.0);
        assert!(grass_keep(12.0) < 0.45);
        assert!(grass_keep(100.0) >= 0.25);
        let mut last = 1.0;
        for i in 0..60 {
            let k = grass_keep(i as f32 * 0.5);
            assert!(k <= last + 1e-6, "not monotone at {}", i as f32 * 0.5);
            last = k;
        }
        assert!(grass_light(0.0) > grass_light(12.0) + 0.3);
    }

    #[test]
    fn flash_goes_white() {
        let v = vec3(0.2, 0.3, 0.4);
        assert_eq!(flash_rgb(v, 0.0), v);
        let w = flash_rgb(v, 1.0);
        assert!(w.min_element() >= 1.0);
    }

    #[test]
    fn moon_where_expected() {
        let (g, _) = sky_glyph(MOON.normalize(), 0.0).expect("moon");
        assert!(matches!(g, '●' | 'O' | 'o'), "{g}");
    }

    #[test]
    fn hills_below_ridge() {
        let (_, rgb) = sky_glyph(vec3(0.0, 0.002, -1.0).normalize(), 0.0).expect("hill");
        assert!(rgb.max_element() < 0.25, "dark silhouette: {rgb}");
    }

    #[test]
    fn stars_stable_and_sparse() {
        let mut lit = 0;
        let mut total = 0;
        for i in 0..200 {
            for j in 0..100 {
                let az = i as f32 / 200.0 * std::f32::consts::TAU;
                let el = 0.2 + j as f32 / 100.0;
                let d = vec3(el.cos() * az.cos(), el.sin(), el.cos() * az.sin());
                let a = sky_glyph(d, 0.0);
                let b = sky_glyph(d, 5.0);
                assert_eq!(a.map(|x| x.0), b.map(|x| x.0), "same star glyph over time");
                total += 1;
                lit += a.is_some() as u32;
            }
        }
        let f = lit as f32 / total as f32;
        assert!((0.005..0.05).contains(&f), "star fraction {f}");
    }
}
