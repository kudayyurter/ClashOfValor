//! Glyph effects: free-floating particles (embers, sparks, trails, sparkles, smoke),
//! floating numbers, the fire orb's flight and the Warrior's shield rim.
//!
//! Particles and text are projected from 3D and depth-tested against the coarse grid,
//! the way the campfire embers always were. The fire orb itself is a real SDF part
//! (`Mat::Orb`) that main.rs adds to the scene while `Fx::orb` is set.

use crate::anim::Action;
use crate::ascii::Cam;
use crate::rules::{Class, Event, Move};
use crate::scene::Body;
use macroquad::prelude::*;

pub const RED: Vec3 = Vec3::new(1.0, 0.3, 0.25);
pub const GREEN: Vec3 = Vec3::new(0.4, 1.0, 0.45);
pub const CYAN: Vec3 = Vec3::new(0.35, 0.9, 1.0);
pub const BLUE: Vec3 = Vec3::new(0.45, 0.65, 1.0);
pub const GOLD: Vec3 = Vec3::new(1.0, 0.82, 0.35);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Ember,
    Spark,
    Trail,
    Sparkle,
    Smoke,
    Mote,
}

#[derive(Clone, Debug)]
pub struct Particle {
    pub p: Vec3,
    pub v: Vec3,
    pub life: f32,
    pub max: f32,
    pub kind: Kind,
    pub rgb: Vec3,
    /// Velocity damping per second.
    pub drag: f32,
    /// Upward acceleration (negative falls).
    pub lift: f32,
    /// Trails: the blade's direction of travel, which picks the glyph.
    pub dir: Vec3,
}

#[derive(Clone, Debug)]
pub struct FloatText {
    pub p: Vec3,
    pub text: String,
    pub rgb: Vec3,
    pub life: f32,
    pub max: f32,
    pub scale: f32,
}

#[derive(Default)]
pub struct Fx {
    pub particles: Vec<Particle>,
    pub texts: Vec<FloatText>,
    /// The fire orb's position while it flies.
    pub orb: Option<Vec3>,
    /// Shield rim per fighter: 1 while the shield holds.
    pub ward: [f32; 2],
    /// Brightens the rim when it absorbs a hit or is raised; decays to 0.
    pub ward_flash: [f32; 2],
    prev_phase: f32,
    prev_tip: Option<Vec3>,
}

fn rnd(a: f32, b: f32) -> f32 {
    rand::gen_range(a, b)
}

fn rnd_dir() -> Vec3 {
    vec3(rnd(-1.0, 1.0), rnd(-1.0, 1.0), rnd(-1.0, 1.0)).normalize_or_zero()
}

/// Rises over a..b of the phase.
fn window(p0: f32, p1: f32, a: f32, b: f32) -> bool {
    p1 >= a && p0 < b
}

impl Fx {
    pub fn update(&mut self, dt: f32) {
        for p in &mut self.particles {
            p.life += dt;
            p.v *= (1.0 - p.drag * dt).max(0.0);
            p.v.y += p.lift * dt;
            p.p += p.v * dt;
        }
        self.particles.retain(|p| p.life < p.max);
        for t in &mut self.texts {
            t.life += dt;
            t.p.y += dt * 0.8 / 1.2;
        }
        self.texts.retain(|t| t.life < t.max);
        for f in &mut self.ward_flash {
            *f = (*f - dt * 2.5).max(0.0);
        }
    }

    /// The campfire's ember stream.
    pub fn campfire(&mut self, fire: Vec3, time: f32, dt: f32) {
        let n = (dt * 35.0 + rnd(0.0, 1.0)) as usize;
        for _ in 0..n {
            self.particles.push(Particle {
                p: fire + vec3(rnd(-0.15, 0.15), 0.35, rnd(-0.15, 0.15)),
                v: vec3(rnd(-0.3, 0.3), rnd(0.8, 1.8), rnd(-0.3, 0.3)),
                life: 0.0,
                max: rnd(0.8, 2.0),
                kind: Kind::Ember,
                rgb: vec3(1.0, 0.6, 0.2),
                drag: 0.6,
                lift: 0.6,
                dir: Vec3::ZERO,
            });
        }
        // A little sideways drift, as before.
        for p in self.particles.iter_mut().filter(|p| p.kind == Kind::Ember) {
            p.v.x += (time * 3.0 + p.p.y * 4.0).sin() * dt * 0.8;
        }
    }

    pub fn text(&mut self, at: Vec3, s: String, rgb: Vec3, scale: f32) {
        self.texts.push(FloatText { p: at, text: s, rgb, life: 0.0, max: 1.2, scale });
    }

    pub fn burst(&mut self, at: Vec3, n: usize, kind: Kind, rgb: Vec3) {
        for _ in 0..n {
            let (v, max, drag, lift) = match kind {
                Kind::Spark => (rnd_dir() * rnd(1.5, 3.5), rnd(0.25, 0.6), 3.0, -3.0),
                Kind::Ember => (rnd_dir() * rnd(0.8, 2.6) + vec3(0.0, 0.6, 0.0), rnd(0.6, 1.6), 1.2, 0.4),
                Kind::Sparkle => (vec3(rnd(-0.3, 0.3), rnd(0.4, 1.1), rnd(-0.3, 0.3)), rnd(0.7, 1.4), 0.5, 0.2),
                Kind::Smoke => (vec3(rnd(-0.4, 0.4), rnd(0.2, 0.7), rnd(-0.4, 0.4)), rnd(0.6, 1.3), 1.0, 0.3),
                Kind::Mote => (vec3(rnd(-0.15, 0.15), rnd(0.25, 0.6), rnd(-0.15, 0.15)), rnd(0.9, 1.6), 0.3, 0.05),
                Kind::Trail => (Vec3::ZERO, 0.3, 0.0, 0.0),
            };
            let jitter = if matches!(kind, Kind::Sparkle | Kind::Mote) { vec3(rnd(-0.35, 0.35), 0.0, rnd(-0.35, 0.35)) } else { Vec3::ZERO };
            self.particles.push(Particle { p: at + jitter, v, life: 0.0, max, kind, rgb, drag, lift, dir: Vec3::ZERO });
        }
    }

    /// One fading blade-trail glyph; `dir` is the blade's motion.
    pub fn trail(&mut self, at: Vec3, dir: Vec3, rgb: Vec3) {
        self.particles.push(Particle { p: at, v: Vec3::ZERO, life: 0.0, max: 0.3, kind: Kind::Trail, rgb, drag: 0.0, lift: 0.0, dir });
    }

    /// Per-frame choreography for the move playing now.
    pub fn on_action(&mut self, a: &Action, bodies: &[Body], dt: f32, impact_now: bool) {
        let p1 = a.phase();
        // A new action starts from the beginning.
        let p0 = if p1 + 1e-6 < self.prev_phase { -1.0 } else { self.prev_phase };
        self.prev_phase = p1;
        let actor = a.res.actor;
        let target = 1 - actor;
        let me = &bodies[actor];
        let them = &bodies[target];
        let world = |b: &Body, local: Vec3| b.to_world(local);
        let tip = world(me, me.anchors.weapon_tip);
        let tip_dir = self.prev_tip.map_or(Vec3::ZERO, |q| tip - q);
        self.prev_tip = Some(tip);
        let steel = vec3(0.85, 0.9, 1.0);

        match a.res.mv {
            Move::Attack if me.class == Class::Assassin => {
                if window(p0, p1, 0.4, 0.5) {
                    self.trail(tip, tip_dir, steel);
                }
            }
            Move::Attack => {
                if window(p0, p1, 0.3, 0.6) {
                    self.trail(tip, tip_dir, steel);
                    self.trail((tip + world(me, me.anchors.hand_r)) * 0.5, tip_dir, steel * 0.8);
                }
            }
            Move::CritAttack => {
                if window(p0, p1, 0.1, 0.35) && rnd(0.0, 1.0) < 0.6 {
                    self.burst(world(me, me.anchors.chest), 2, Kind::Smoke, vec3(0.45, 0.45, 0.55));
                }
                if p0 < 0.42 && p1 >= 0.42 {
                    self.burst(world(me, me.anchors.chest), 12, Kind::Smoke, vec3(0.4, 0.4, 0.5));
                }
            }
            Move::FireOrb => {
                if window(p0, p1, 0.2, 0.4) {
                    self.burst(world(me, me.anchors.focus), 1, Kind::Mote, vec3(1.0, 0.7, 0.3));
                }
                let (from, to) = (world(me, me.anchors.focus), world(them, them.anchors.chest));
                if (0.4..0.7).contains(&p1) && !impact_now {
                    let s = (p1 - 0.4) / 0.3;
                    let pos = from.lerp(to, s) + vec3(0.0, 0.35 * (s * std::f32::consts::PI).sin(), 0.0);
                    self.orb = Some(pos);
                    let n = (dt * 60.0).ceil() as usize;
                    self.burst(pos, n, Kind::Ember, vec3(1.0, 0.55, 0.15));
                } else {
                    self.orb = None;
                }
            }
            Move::Flee
                if window(p0, p1, 0.2, 1.0) && rnd(0.0, 1.0) < 0.7 => {
                    self.burst(world(me, me.anchors.chest), 2, Kind::Smoke, vec3(0.5, 0.5, 0.58));
                }
            _ => {}
        }

        if !impact_now {
            return;
        }
        self.orb = None;
        if a.res.mv == Move::FireOrb {
            let at = world(them, them.anchors.chest);
            self.burst(at, 40, Kind::Ember, vec3(1.0, 0.55, 0.15));
            self.burst(at, 12, Kind::Spark, vec3(1.0, 0.85, 0.5));
        }
        let feet = |b: &Body| b.to_world(vec3(0.0, 0.1, 0.0));
        let above = |b: &Body| b.to_world(b.anchors.head + vec3(0.0, 0.35, 0.0));
        for e in &a.res.events {
            match *e {
                Event::Hit { target, absorbed, damage, crit } => {
                    let b = &bodies[target];
                    let chest = world(b, b.anchors.chest);
                    self.burst(chest, if crit { 30 } else { 14 }, Kind::Spark, if crit { RED } else { vec3(1.0, 0.9, 0.6) });
                    if damage > 0 {
                        let (s, scale) = if crit { (format!("CRIT {damage}"), 1.6) } else { (format!("-{damage}"), 1.0) };
                        self.text(above(b), s, RED, scale);
                    }
                    if absorbed > 0 {
                        self.text(above(b) + vec3(0.0, if damage > 0 { 0.25 } else { 0.0 }, 0.0), format!("-{absorbed} shield"), BLUE, 1.0);
                        self.ward_flash[target] = 1.0;
                    }
                }
                Event::Healed { who, hp } => {
                    self.burst(feet(&bodies[who]), 24, Kind::Sparkle, GREEN);
                    self.text(above(&bodies[who]), format!("+{hp} HP"), GREEN, 1.0);
                }
                Event::Mana { who, mp } => {
                    self.burst(feet(&bodies[who]), 24, Kind::Sparkle, CYAN);
                    self.text(above(&bodies[who]), format!("+{mp} MP"), CYAN, 1.0);
                }
                Event::Shield { who, amount } => {
                    self.ward[who] = 1.0;
                    self.ward_flash[who] = 1.0;
                    self.burst(world(&bodies[who], bodies[who].anchors.chest), 16, Kind::Sparkle, BLUE);
                    self.text(above(&bodies[who]), format!("+{amount} SH"), BLUE, 1.0);
                }
                Event::Rested { who, hp, stamina, mana } => {
                    self.burst(feet(&bodies[who]), 16, Kind::Mote, GOLD);
                    let mut s = format!("+{hp} HP +{stamina} ST");
                    if mana > 0 {
                        s += &format!(" +{mana} MP");
                    }
                    self.text(above(&bodies[who]), s, GOLD, 1.0);
                }
                Event::Fled { who } => self.burst(world(&bodies[who], bodies[who].anchors.chest), 20, Kind::Smoke, vec3(0.5, 0.5, 0.58)),
                Event::Defeated { .. } => {}
            }
        }
    }
}

/// Glyph for a line through screen direction (dx, dy), y down.
fn line_glyph(dx: f32, dy: f32) -> char {
    let a = (-dy).atan2(dx).rem_euclid(std::f32::consts::PI) / std::f32::consts::PI * 8.0;
    if !(1.0..7.0).contains(&a) {
        '-'
    } else if a < 3.0 {
        '/'
    } else if a < 5.0 {
        '|'
    } else {
        '\\'
    }
}

fn particle_glyph(p: &Particle, f: f32) -> char {
    let pick = |g: &[char]| g[((f * g.len() as f32) as usize).min(g.len() - 1)];
    match p.kind {
        Kind::Ember => pick(&['*', 'o', '.']),
        Kind::Spark => pick(&['*', '+', '.']),
        Kind::Sparkle => pick(&['✶', '*', '+', '·']),
        Kind::Smoke => pick(&['@', 'O', 'o', '.']),
        Kind::Mote => pick(&['*', '·', '.']),
        Kind::Trail => '-',
    }
}

fn glyph(font: &Font, c: char, x: f32, y: f32, size: u16, color: Color) {
    let mut buf = [0u8; 4];
    draw_text_ex(c.encode_utf8(&mut buf), x, y, TextParams { font: Some(font), font_size: size, color, ..Default::default() });
}

/// Draw particles, floating text and shield rims. `depth_at(x, y)` is the scene depth at a
/// screen point.
pub fn draw(fx: &Fx, cam: &Cam, depth_at: &dyn Fn(f32, f32) -> f32, font: &Font, fs: u16, chests: [Vec3; 2]) {
    let visible = |sp: Vec2, depth: f32| depth <= depth_at(sp.x, sp.y) + 0.2;
    // Shield rims: a ring of glyphs facing the camera around each warded fighter.
    for i in 0..2 {
        if fx.ward[i] <= 0.0 {
            continue;
        }
        let bright = (0.5 + 0.7 * fx.ward_flash[i]).min(1.2);
        for k in 0..24 {
            let a = k as f32 / 24.0 * std::f32::consts::TAU;
            let p = chests[i] + (cam.right * a.cos() * 0.7 + cam.up * a.sin() * 0.85) * fx.ward[i];
            let Some((sp, depth)) = cam.project(p) else { continue };
            if !visible(sp, depth) {
                continue;
            }
            // Tangent of the ring on screen picks the glyph.
            let g = line_glyph(-a.sin(), -a.cos());
            let c = BLUE * bright;
            let size = ((fs as f32) * 4.5 / depth).clamp(8.0, 22.0) as u16;
            draw_circle(sp.x, sp.y, size as f32 * 0.6, Color::new(0.3, 0.5, 1.0, 0.05 * bright));
            glyph(font, g, sp.x - size as f32 * 0.3, sp.y + size as f32 * 0.35, size, Color::new(c.x, c.y, c.z, 0.5 + 0.5 * bright.min(1.0)));
        }
    }
    for p in &fx.particles {
        let Some((sp, depth)) = cam.project(p.p) else { continue };
        if !visible(sp, depth) {
            continue;
        }
        let f = p.life / p.max;
        let g = if p.kind == Kind::Trail {
            match cam.project(p.p + p.dir) {
                Some((sp2, _)) if (sp2 - sp).length() > 0.5 => line_glyph(sp2.x - sp.x, sp2.y - sp.y),
                _ => '-',
            }
        } else {
            particle_glyph(p, f)
        };
        let fade = 1.0 - f;
        let rgb = match p.kind {
            Kind::Ember => vec3(1.0, 0.85 - 0.6 * f, 0.35 - 0.3 * f),
            _ => p.rgb,
        };
        let alpha = match p.kind {
            Kind::Smoke => 0.55 * fade,
            _ => 1.0 - f * 0.7,
        };
        if matches!(p.kind, Kind::Ember | Kind::Spark | Kind::Sparkle) {
            let ch = fs as f32 * 1.2;
            draw_circle(sp.x, sp.y, ch * 0.8, Color::new(rgb.x, rgb.y * 0.6, rgb.z * 0.4, 0.07 * fade));
        }
        let size = ((fs as f32) * 5.0 / depth).clamp(6.0, 28.0) as u16;
        glyph(font, g, sp.x - size as f32 * 0.3, sp.y + size as f32 * 0.35, size, Color::new(rgb.x, rgb.y, rgb.z, alpha));
    }
    for t in &fx.texts {
        let Some((sp, _)) = cam.project(t.p) else { continue };
        let f = t.life / t.max;
        let size = (17.0 * t.scale) as u16;
        let w = measure_text(&t.text, Some(font), size, 1.0).width;
        let a = if f < 0.7 { 1.0 } else { 1.0 - (f - 0.7) / 0.3 };
        let params = |c: Color| TextParams { font: Some(font), font_size: size, color: c, ..Default::default() };
        draw_text_ex(&t.text, sp.x - w * 0.5 + 1.5, sp.y + 1.5, params(Color::new(0.0, 0.0, 0.0, 0.8 * a)));
        draw_text_ex(&t.text, sp.x - w * 0.5, sp.y, params(Color::new(t.rgb.x, t.rgb.y, t.rgb.z, a)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::{Class, Resolution};
    use crate::scene::Scene;

    fn scene() -> Scene {
        let mut s = Scene::new();
        s.set_fighters([Class::Sorcerer, Class::Warrior]);
        s.update(0.016);
        s
    }

    fn action(mv: Move, events: Vec<Event>) -> Action {
        Action::new(Resolution { actor: 0, mv, events, narration: vec![String::new()] })
    }

    #[test]
    fn particles_expire() {
        let mut fx = Fx::default();
        fx.burst(Vec3::ZERO, 10, Kind::Spark, RED);
        assert_eq!(fx.particles.len(), 10);
        assert!(fx.particles.iter().all(|p| p.max <= 2.0));
        for _ in 0..25 {
            fx.update(0.1);
        }
        assert!(fx.particles.is_empty());
    }

    #[test]
    fn text_rises_and_expires() {
        let mut fx = Fx::default();
        fx.text(vec3(0.0, 1.0, 0.0), "-10".into(), RED, 1.0);
        fx.update(0.6);
        assert!(fx.texts[0].p.y > 1.2);
        fx.update(0.7);
        assert!(fx.texts.is_empty());
    }

    #[test]
    fn ward_flash_decays() {
        let mut fx = Fx::default();
        fx.ward_flash[0] = 1.0;
        fx.update(1.0);
        assert!(fx.ward_flash[0] < 0.1 && fx.ward_flash[0] >= 0.0);
    }

    #[test]
    fn hits_spawn_numbers_and_sparks() {
        let s = scene();
        let mut fx = Fx::default();
        let a = action(Move::FireOrb, vec![Event::Hit { target: 1, absorbed: 14, damage: 0, crit: false }]);
        fx.on_action(&a, &s.bodies, 0.016, true);
        assert!(fx.texts.iter().any(|t| t.text == "-14 shield" && t.rgb == BLUE));
        assert!(!fx.texts.iter().any(|t| t.text == "-0"), "no zero damage number");
        assert_eq!(fx.ward_flash[1], 1.0);
        assert!(fx.particles.iter().any(|p| p.kind == Kind::Spark));

        let mut fx = Fx::default();
        let a = action(Move::CritAttack, vec![Event::Hit { target: 1, absorbed: 0, damage: 15, crit: true }]);
        fx.on_action(&a, &s.bodies, 0.016, true);
        let t = fx.texts.iter().find(|t| t.text == "CRIT 15").expect("crit text");
        assert!(t.scale > 1.4 && t.rgb == RED);
    }

    #[test]
    fn heals_and_rest_float_numbers() {
        let s = scene();
        let mut fx = Fx::default();
        fx.on_action(&action(Move::HealthPotion, vec![Event::Healed { who: 0, hp: 50 }]), &s.bodies, 0.016, true);
        assert!(fx.texts.iter().any(|t| t.text == "+50 HP" && t.rgb == GREEN));
        fx.on_action(&action(Move::ManaPotion, vec![Event::Mana { who: 0, mp: 30 }]), &s.bodies, 0.016, true);
        assert!(fx.texts.iter().any(|t| t.text == "+30 MP" && t.rgb == CYAN));
        fx.on_action(&action(Move::Rest, vec![Event::Rested { who: 0, hp: 5, stamina: 25, mana: 20 }]), &s.bodies, 0.016, true);
        assert!(fx.texts.iter().any(|t| t.text == "+5 HP +25 ST +20 MP"));
        fx.on_action(&action(Move::Rest, vec![Event::Rested { who: 0, hp: 0, stamina: 25, mana: 0 }]), &s.bodies, 0.016, true);
        assert!(fx.texts.iter().any(|t| t.text == "+0 HP +25 ST"));
    }

    #[test]
    fn orb_flies_then_bursts_on_impact() {
        let s = scene();
        let mut fx = Fx::default();
        let mut a = action(Move::FireOrb, vec![Event::Hit { target: 1, absorbed: 0, damage: 14, crit: false }]);
        a.advance(0.55 * a.dur);
        fx.on_action(&a, &s.bodies, 0.016, false);
        let orb = fx.orb.expect("orb in flight");
        let (x0, x1) = (s.bodies[0].pos.x, s.bodies[1].pos.x);
        assert!(orb.x > x0 && orb.x < x1, "between the fighters");
        let fired = a.advance(a.dur);
        assert!(fired);
        fx.on_action(&a, &s.bodies, 0.016, true);
        assert!(fx.orb.is_none());
        assert!(fx.particles.iter().filter(|p| p.kind == Kind::Ember).count() >= 30);
    }

    #[test]
    fn warrior_swing_leaves_a_trail() {
        let mut s = Scene::new();
        s.set_fighters([Class::Warrior, Class::Sorcerer]);
        s.update(0.016);
        let mut fx = Fx::default();
        let mut a = action(Move::Attack, vec![]);
        for _ in 0..40 {
            a.advance(0.03);
            fx.on_action(&a, &s.bodies, 0.03, false);
        }
        assert!(fx.particles.iter().any(|p| p.kind == Kind::Trail));
    }
}
