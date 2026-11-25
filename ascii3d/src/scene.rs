//! The 3D world: signed distance functions (SDFs), materials, ray marching and lighting.
//!
//! Nothing in this file knows about ASCII. It answers one question: "what does a ray
//! through this pixel hit, and how bright and what colour is it there?". The ASCII layer
//! (ascii.rs) asks that question several times per character cell and turns the answers
//! into a glyph.

use crate::models::{self, Anchors, Pose};
use crate::rules::Class;
use macroquad::math::{Vec3, vec3};

pub const MAX_DIST: f32 = 60.0;
/// Part id for rays that hit nothing.
pub const SKY: u16 = 0;
pub const GROUND: u16 = 1;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mat {
    Sky,
    Grass,
    Stone,
    Leaf,
    Wood,
    Fire,
    Plate,
    Chain,
    Blade,
    Cloth,
    /// Sorcerer's robe and hat.
    Robe,
    /// Assassin's leathers.
    Leather,
    /// Hoods and cloaks in the player's accent colour.
    Cloak,
    /// The glowing stone on the Sorcerer's staff.
    Crystal,
    /// The fire orb in flight.
    Orb,
}

impl Mat {
    /// Hero materials are drawn on the fine (half-size) glyph grid.
    pub fn is_hero(self) -> bool {
        matches!(self, Mat::Plate | Mat::Chain | Mat::Blade | Mat::Cloth | Mat::Robe | Mat::Leather | Mat::Cloak | Mat::Crystal)
    }

    /// Light sources: drawn at their own colour, no outlines, with a halo.
    pub fn is_emissive(self) -> bool {
        matches!(self, Mat::Fire | Mat::Crystal | Mat::Orb)
    }
}

/// One craftable metal. Like the "#%: Iron" chips in the reels, each metal owns a
/// three-glyph ramp (bright, mid, dark) and a tint.
pub struct Metal {
    pub name: &'static str,
    pub glyphs: [char; 3],
    pub rgb: Vec3,
}

pub const METALS: [Metal; 10] = [
    Metal { name: "Iron", glyphs: ['#', '%', ':'], rgb: Vec3::new(0.62, 0.64, 0.68) },
    Metal { name: "Steel", glyphs: ['@', '%', '.'], rgb: Vec3::new(0.78, 0.82, 0.9) },
    Metal { name: "Bronze", glyphs: ['&', 'o', ','], rgb: Vec3::new(0.85, 0.55, 0.3) },
    Metal { name: "Silver", glyphs: ['$', 's', '.'], rgb: Vec3::new(0.9, 0.92, 0.95) },
    Metal { name: "Gold", glyphs: ['0', 'o', '.'], rgb: Vec3::new(1.0, 0.8, 0.35) },
    Metal { name: "Moonsteel", glyphs: ['O', '*', '·'], rgb: Vec3::new(0.6, 0.8, 1.0) },
    Metal { name: "Bloodiron", glyphs: ['B', '8', ':'], rgb: Vec3::new(0.85, 0.25, 0.25) },
    Metal { name: "Ossite", glyphs: ['=', '-', '.'], rgb: Vec3::new(0.9, 0.87, 0.75) },
    Metal { name: "Emberstone", glyphs: ['*', '+', '.'], rgb: Vec3::new(1.0, 0.5, 0.2) },
    Metal { name: "Dragonscale", glyphs: ['V', 'A', '>'], rgb: Vec3::new(0.35, 0.85, 0.5) },
];

#[derive(Clone, Copy)]
pub enum Shape {
    Sphere { c: Vec3, r: f32 },
    Capsule { a: Vec3, b: Vec3, r: f32 },
    RBox { c: Vec3, half: Vec3, round: f32 },
    Rock { c: Vec3, r: f32 },
    Ellipsoid { c: Vec3, rad: Vec3 },
    /// Capped cone from `a` (radius `ra`) to `b` (radius `rb`).
    Cone { a: Vec3, b: Vec3, ra: f32, rb: f32 },
}

impl Shape {
    /// Signed distance from `p` to the surface (negative inside).
    pub fn dist(&self, p: Vec3) -> f32 {
        match *self {
            Shape::Sphere { c, r } => (p - c).length() - r,
            Shape::Capsule { a, b, r } => {
                let (pa, ba) = (p - a, b - a);
                let h = (pa.dot(ba) / ba.dot(ba)).clamp(0.0, 1.0);
                (pa - ba * h).length() - r
            }
            Shape::RBox { c, half, round } => {
                let q = (p - c).abs() - half + Vec3::splat(round);
                q.max(Vec3::ZERO).length() + q.max_element().min(0.0) - round
            }
            Shape::Rock { c, r } => {
                let bump = (p.x * 4.0).sin() * (p.y * 4.0 + 1.0).sin() * (p.z * 4.0 + 2.0).sin();
                ((p - c).length() - r + 0.07 * bump) * 0.75
            }
            Shape::Ellipsoid { c, rad } => {
                let q = p - c;
                let k0 = (q / rad).length();
                let k1 = (q / (rad * rad)).length();
                if k1 < 1e-6 { -rad.min_element() } else { k0 * (k0 - 1.0) / k1 }
            }
            Shape::Cone { a, b, ra, rb } => {
                // Inigo Quilez's capped cone between two points.
                let rba = rb - ra;
                let baba = (b - a).dot(b - a);
                let papa = (p - a).dot(p - a);
                let paba = (p - a).dot(b - a) / baba;
                let x = (papa - paba * paba * baba).max(0.0).sqrt();
                let cax = (x - if paba < 0.5 { ra } else { rb }).max(0.0);
                let cay = (paba - 0.5).abs() - 0.5;
                let k = rba * rba + baba;
                let f = ((rba * (x - ra) + paba * baba) / k).clamp(0.0, 1.0);
                let cbx = x - ra - f * rba;
                let cby = paba - f;
                let s = if cbx < 0.0 && cay < 0.0 { -1.0 } else { 1.0 };
                s * (cax * cax + cay * cay * baba).min(cbx * cbx + cby * cby * baba).sqrt()
            }
        }
    }

    /// Coordinates on the surface of this part: (angle around it, distance along it).
    /// Glyph patterns that should "stick" to an object (chainmail rings, cloth
    /// ornaments) are laid out in these coordinates instead of in screen space.
    pub fn surface_uv(&self, p: Vec3) -> (f32, f32) {
        match *self {
            Shape::Sphere { c, .. } | Shape::Rock { c, .. } | Shape::Ellipsoid { c, .. } => {
                let d = p - c;
                (d.z.atan2(d.x), d.y)
            }
            Shape::Capsule { a, b, .. } | Shape::Cone { a, b, .. } => {
                let ba = b - a;
                let len = ba.length();
                let h = ((p - a).dot(ba) / (len * len)).clamp(0.0, 1.0);
                let d = p - (a + ba * h);
                (d.z.atan2(d.x), h * len)
            }
            Shape::RBox { c, .. } => {
                let d = p - c;
                (d.x * 6.0, d.y)
            }
        }
    }
}

#[derive(Clone, Copy)]
pub struct Part {
    /// Unique per primitive. Cells that straddle two ids become outline glyphs.
    pub id: u16,
    pub mat: Mat,
    pub shape: Shape,
}

/// A group of parts with a bounding sphere, so rays far away skip it cheaply.
pub struct Prop {
    pub center: Vec3,
    pub radius: f32,
    pub parts: Vec<Part>,
}

#[derive(Clone, Copy)]
pub struct Hit {
    pub t: f32,
    pub id: u16,
    pub mat: Mat,
}

pub struct Shade {
    pub rgb: Vec3,
    pub lum: f32,
    pub p: Vec3,
}

pub struct Scene {
    pub time: f32,
    /// The two fighters. Parts live in each body's local space.
    pub bodies: Vec<Body>,
    /// Effect parts in world space (ids from `FX_ID`).
    pub fx_parts: Vec<Part>,
    pub props: Vec<Prop>,
    pub fire: Vec3,
    pub flicker: f32,
    pub chain_legs: bool,
    /// Direction towards the moon (the main light).
    pub sun: Vec3,
}

pub fn ground_height(x: f32, z: f32) -> f32 {
    0.12 * (x * 0.35).sin() * (z * 0.3).cos() + 0.05 * (x * 1.1 + z * 0.7).sin()
}

/// Cheap integer hash to a float in [0, 1].
pub fn hash2(x: i32, y: i32) -> f32 {
    let mut h = (x as u32).wrapping_mul(374_761_393) ^ (y as u32).wrapping_mul(668_265_263);
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    ((h ^ (h >> 16)) & 0xffff) as f32 / 65535.0
}

impl Scene {
    pub fn new() -> Self {
        let mut props = Vec::new();
        let mut id = 100u16;
        let mut next = || {
            id += 1;
            id
        };

        let rocks = [
            (vec3(-4.0, 0.2, 3.0), 0.9),
            (vec3(7.5, 0.0, -5.5), 0.8),
            (vec3(-2.0, 0.0, -5.0), 0.7),
            (vec3(7.0, 0.2, 5.0), 1.0),
            (vec3(-7.0, 0.2, -1.0), 1.3),
            (vec3(4.5, 0.0, 6.5), 0.5),
        ];
        for (c, r) in rocks {
            props.push(Prop {
                center: c,
                radius: r + 0.3,
                parts: vec![Part { id: next(), mat: Mat::Stone, shape: Shape::Rock { c, r } }],
            });
        }

        let trees = [
            (-6.0, 6.0),
            (6.0, -7.0),
            (-9.0, -6.0),
            (9.0, 2.0),
            (-3.0, 9.0),
            (3.0, -10.0),
            (10.0, 9.0),
            (-11.0, 3.0),
        ];
        for (x, z) in trees {
            let y = ground_height(x, z);
            props.push(Prop {
                center: vec3(x, y + 1.7, z),
                radius: 2.0,
                parts: vec![
                    Part {
                        id: next(),
                        mat: Mat::Wood,
                        shape: Shape::Capsule { a: vec3(x, y, z), b: vec3(x, y + 1.4, z), r: 0.12 },
                    },
                    Part { id: next(), mat: Mat::Leaf, shape: Shape::Sphere { c: vec3(x, y + 1.7, z), r: 0.8 } },
                    Part { id: next(), mat: Mat::Leaf, shape: Shape::Sphere { c: vec3(x, y + 2.4, z), r: 0.6 } },
                    Part { id: next(), mat: Mat::Leaf, shape: Shape::Sphere { c: vec3(x, y + 2.95, z), r: 0.38 } },
                ],
            });
        }

        let fire = vec3(0.3, ground_height(0.3, -2.0), -2.0);
        props.push(Prop {
            center: fire + vec3(0.0, 0.4, 0.0),
            radius: 0.9,
            parts: vec![
                Part {
                    id: next(),
                    mat: Mat::Wood,
                    shape: Shape::Capsule { a: fire + vec3(-0.35, 0.05, -0.2), b: fire + vec3(0.35, 0.1, 0.2), r: 0.07 },
                },
                Part {
                    id: next(),
                    mat: Mat::Wood,
                    shape: Shape::Capsule { a: fire + vec3(-0.3, 0.1, 0.25), b: fire + vec3(0.3, 0.05, -0.25), r: 0.07 },
                },
                Part {
                    id: next(),
                    mat: Mat::Fire,
                    shape: Shape::Ellipsoid { c: fire + vec3(0.0, 0.42, 0.0), rad: vec3(0.2, 0.42, 0.2) },
                },
            ],
        });

        Scene {
            time: 0.0,
            bodies: Vec::new(),
            fx_parts: Vec::new(),
            props,
            fire,
            flicker: 1.0,
            chain_legs: true,
            sun: vec3(-0.35, 0.75, 0.6).normalize(),
        }
    }

    /// Place the two fighters facing each other across the clearing.
    pub fn set_fighters(&mut self, classes: [Class; 2]) {
        let keep: Vec<usize> = self.bodies.iter().map(|b| b.metal).collect();
        self.bodies = (0..2)
            .map(|i| {
                let x = if i == 0 { -1.9 } else { 1.9 };
                Body {
                    class: classes[i],
                    pos: vec3(x, ground_height(x, 0.0), 0.0),
                    yaw: if i == 0 { std::f32::consts::FRAC_PI_2 } else { -std::f32::consts::FRAC_PI_2 },
                    pose: Pose::default(),
                    metal: keep.get(i).copied().unwrap_or(0),
                    accent: ACCENTS[i],
                    dim: 0.0,
                    flash: 0.0,
                    parts: Vec::new(),
                    anchors: Anchors::default(),
                }
            })
            .collect();
        self.rebuild();
    }

    fn rebuild(&mut self) {
        for (i, b) in self.bodies.iter_mut().enumerate() {
            let (parts, anchors) = models::build(b.class, &b.pose, BODY_ID[i], self.chain_legs);
            b.parts = parts;
            b.anchors = anchors;
        }
    }

    /// Darkening for a ground point near a fighter's feet: about 0.35 underfoot,
    /// 1.0 beyond 0.9 m. Invisible fighters cast none.
    pub fn contact_shadow(&self, p: Vec3) -> f32 {
        let mut m = 1.0f32;
        for b in self.bodies.iter().filter(|b| b.dim < 0.99) {
            let feet = b.to_world(Vec3::ZERO);
            let d = ((p.x - feet.x).powi(2) + (p.z - feet.z).powi(2)).sqrt();
            let f = ((d - 0.25) / 0.65).clamp(0.0, 1.0);
            m = m.min(0.35 + 0.65 * f * f * (3.0 - 2.0 * f));
        }
        m
    }

    /// Which fighter owns a part id.
    pub fn owner(id: u16) -> Option<usize> {
        match id {
            10..=49 => Some(0),
            50..=89 => Some(1),
            _ => None,
        }
    }

    pub fn body_part(&self, id: u16) -> Option<(&Body, &Part)> {
        let b = self.bodies.get(Self::owner(id)?)?;
        Some((b, b.parts.iter().find(|p| p.id == id)?))
    }

    pub fn update(&mut self, dt: f32) {
        self.time += dt;
        self.flicker = 0.85 + 0.15 * (self.time * 13.0).sin() * (self.time * 7.3).cos();
        self.rebuild();
    }

    /// The whole world as one distance function. Returns (distance, part id, material).
    pub fn sdf(&self, p: Vec3) -> (f32, u16, Mat) {
        let mut best = ((p.y - ground_height(p.x, p.z)) * 0.8, GROUND, Mat::Grass);
        for prop in &self.props {
            if (p - prop.center).length() - prop.radius > best.0 {
                continue;
            }
            for part in &prop.parts {
                let d = part.shape.dist(p);
                if d < best.0 {
                    best = (d, part.id, part.mat);
                }
            }
        }
        for b in &self.bodies {
            if b.dim >= 0.99 {
                continue;
            }
            // to_local is rigid, so the bound test can run in local space.
            let lp = b.to_local(p);
            if (lp - models::BOUND_CENTER).length() - models::BOUND_RADIUS > best.0 {
                continue;
            }
            for part in &b.parts {
                let d = part.shape.dist(lp);
                if d < best.0 {
                    best = (d, part.id, part.mat);
                }
            }
        }
        for part in &self.fx_parts {
            let d = part.shape.dist(p);
            if d < best.0 {
                best = (d, part.id, part.mat);
            }
        }
        best
    }

    /// Sphere tracing: step along the ray by the distance to the nearest surface.
    pub fn march(&self, ro: Vec3, rd: Vec3) -> Hit {
        let mut t = 0.02;
        let mut last = (SKY, Mat::Sky);
        for _ in 0..110 {
            let (d, id, mat) = self.sdf(ro + rd * t);
            last = (id, mat);
            if d < 0.001 * t.max(1.0) {
                return Hit { t, id, mat };
            }
            t += d;
            if t > MAX_DIST {
                return Hit { t: MAX_DIST, id: SKY, mat: Mat::Sky };
            }
        }
        // Ran out of steps while skimming a surface (usually far ground): count it as a hit.
        Hit { t, id: last.0, mat: last.1 }
    }

    fn normal(&self, p: Vec3) -> Vec3 {
        let e = 0.002;
        let ks = [vec3(1.0, -1.0, -1.0), vec3(-1.0, -1.0, 1.0), vec3(-1.0, 1.0, -1.0), vec3(1.0, 1.0, 1.0)];
        let mut n = Vec3::ZERO;
        for k in ks {
            n += k * self.sdf(p + k * e).0;
        }
        n.normalize_or_zero()
    }

    fn soft_shadow(&self, p: Vec3, l: Vec3) -> f32 {
        let mut res = 1.0f32;
        let mut t = 0.05;
        for _ in 0..28 {
            let d = self.sdf(p + l * t).0;
            res = res.min(8.0 * d / t);
            if res < 0.02 {
                return 0.0;
            }
            t += d.clamp(0.05, 0.5);
            if t > 12.0 {
                break;
            }
        }
        res.clamp(0.0, 1.0)
    }

    fn base_color(&self, hit: &Hit, p: Vec3) -> Vec3 {
        let owner = Self::owner(hit.id).and_then(|i| self.bodies.get(i));
        let metal = METALS[owner.map_or(0, |b| b.metal)].rgb;
        let accent = owner.map_or(ACCENTS[0], |b| b.accent);
        match hit.mat {
            Mat::Grass => {
                let h = hash2((p.x * 3.0).floor() as i32, (p.z * 3.0).floor() as i32);
                vec3(0.22, 0.52, 0.26) * (0.7 + 0.6 * h)
            }
            Mat::Stone => vec3(0.55, 0.62, 0.78),
            Mat::Leaf => vec3(0.22, 0.6, 0.45),
            Mat::Wood => vec3(0.5, 0.33, 0.2),
            Mat::Plate | Mat::Blade => metal,
            Mat::Chain => metal * 0.9,
            Mat::Cloth => accent * 0.8,
            Mat::Robe => vec3(0.42, 0.2, 0.66),
            Mat::Leather => vec3(0.36, 0.32, 0.3),
            Mat::Cloak => accent * 0.7,
            Mat::Fire | Mat::Sky | Mat::Crystal | Mat::Orb => Vec3::ONE,
        }
    }

    /// Light one hit point: moonlight with soft shadows, a flickering campfire, metal glints, fog.
    pub fn shade(&self, ro: Vec3, rd: Vec3, hit: &Hit) -> Shade {
        let p = ro + rd * hit.t;
        if hit.mat == Mat::Crystal {
            let pulse = 0.85 + 0.15 * (self.time * 4.0).sin();
            return Shade { rgb: vec3(0.55, 0.85, 1.0) * pulse, lum: 0.9 * pulse, p };
        }
        if hit.mat == Mat::Orb {
            let h = hash2((p.x * 30.0 + self.time * 20.0) as i32, (p.y * 30.0) as i32);
            let rgb = vec3(1.0, 0.85, 0.4).lerp(vec3(1.0, 0.3, 0.05), h);
            return Shade { rgb, lum: 1.0 - 0.4 * h, p };
        }
        if hit.mat == Mat::Fire {
            let h = ((p.y - self.fire.y) / 0.8).clamp(0.0, 1.0);
            let rgb = vec3(1.0, 0.9, 0.45).lerp(vec3(1.0, 0.25, 0.05), h) * self.flicker;
            return Shade { rgb, lum: 1.0 - 0.6 * h, p };
        }
        let n = self.normal(p);
        let base = self.base_color(hit, p);
        let diff = n.dot(self.sun).max(0.0) * self.soft_shadow(p + n * 0.01, self.sun);
        let to_fire = self.fire + vec3(0.0, 0.4, 0.0) - p;
        let fd2 = to_fire.length_squared();
        let fire = n.dot(to_fire / fd2.sqrt()).max(0.0) * 2.4 * self.flicker / (1.0 + fd2 * 0.9);
        let sky = 0.5 + 0.5 * n.y;
        let mut rgb = base * (vec3(0.1, 0.12, 0.22) * sky + vec3(0.7, 0.72, 0.85) * diff)
            + vec3(1.0, 0.55, 0.2) * fire * (0.4 + 0.6 * base);
        if hit.mat.is_hero() {
            // Character fill light from the viewer, so fighters read against the night.
            rgb += base * vec3(0.55, 0.55, 0.65) * n.dot(-rd).max(0.0);
        }
        if matches!(hit.mat, Mat::Plate | Mat::Blade | Mat::Chain) {
            let r = rd - n * 2.0 * rd.dot(n);
            let spec = r.dot(self.sun).max(0.0).powf(24.0) * 0.7 * (0.3 + 0.7 * diff);
            rgb += Vec3::splat(spec);
        }
        if hit.mat == Mat::Grass {
            rgb *= self.contact_shadow(p);
        }
        rgb *= 1.0 - (hit.t / MAX_DIST).powf(0.7);
        let lum = rgb.dot(vec3(0.3, 0.59, 0.11)).clamp(0.0, 1.0);
        Shade { rgb, lum, p }
    }
}

/// Fighter `i` owns part ids `10 + 40 * i .. 50 + 40 * i`.
pub const BODY_ID: [u16; 2] = [10, 50];
/// Effect parts (the fire orb) use ids 90..100.
pub const FX_ID: u16 = 90;
/// Player accents: blue, red.
pub const ACCENTS: [Vec3; 2] = [Vec3::new(0.25, 0.45, 1.0), Vec3::new(0.95, 0.25, 0.25)];

/// How far a fully fallen body tilts back (radians).
const FALL_ANGLE: f32 = 1.5;

/// One fighter in the world.
pub struct Body {
    pub class: Class,
    pub pos: Vec3,
    pub yaw: f32,
    pub pose: Pose,
    pub metal: usize,
    pub accent: Vec3,
    /// 0 drawn normally .. 1 invisible.
    pub dim: f32,
    /// Hit flash: 1 at impact, fading to 0; glyphs lerp toward white.
    pub flash: f32,
    pub parts: Vec<Part>,
    pub anchors: Anchors,
}

impl Body {
    /// World point -> model space: undo the yaw, then `pose.root`, then the fall tilt.
    pub fn to_local(&self, world: Vec3) -> Vec3 {
        let d = world - self.pos;
        let (s, c) = self.yaw.sin_cos();
        let q = vec3(d.x * c - d.z * s, d.y, d.x * s + d.z * c) - self.pose.root;
        let (ts, tc) = (self.pose.fall * FALL_ANGLE).sin_cos();
        vec3(q.x, q.y * tc - q.z * ts, q.y * ts + q.z * tc)
    }

    /// Model space -> world point. The fall tilts the body backwards about the feet.
    pub fn to_world(&self, local: Vec3) -> Vec3 {
        let (ts, tc) = (self.pose.fall * FALL_ANGLE).sin_cos();
        let q = vec3(local.x, local.y * tc + local.z * ts, -local.y * ts + local.z * tc) + self.pose.root;
        let (s, c) = self.yaw.sin_cos();
        self.pos + vec3(q.x * c + q.z * s, q.y, -q.x * s + q.z * c)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn two(a: Class, b: Class) -> Scene {
        let mut s = Scene::new();
        s.set_fighters([a, b]);
        s.update(0.016);
        s
    }

    #[test]
    fn owner_of_ids() {
        assert_eq!(Scene::owner(10), Some(0));
        assert_eq!(Scene::owner(49), Some(0));
        assert_eq!(Scene::owner(50), Some(1));
        assert_eq!(Scene::owner(89), Some(1));
        assert_eq!(Scene::owner(90), None);
        assert_eq!(Scene::owner(101), None);
        assert_eq!(Scene::owner(1), None);
        assert_eq!(Scene::owner(0), None);
    }

    #[test]
    fn bodies_have_disjoint_ids() {
        let s = two(Class::Warrior, Class::Warrior);
        assert_eq!(s.bodies.len(), 2);
        for (i, b) in s.bodies.iter().enumerate() {
            assert!(!b.parts.is_empty());
            for p in &b.parts {
                assert_eq!(Scene::owner(p.id), Some(i), "part {} of body {i}", p.id);
                assert_eq!(s.body_part(p.id).map(|(_, q)| q.id), Some(p.id));
            }
        }
    }

    #[test]
    fn local_world_roundtrip() {
        let mut s = two(Class::Warrior, Class::Sorcerer);
        s.bodies[1].pose.root = vec3(0.2, 0.0, 1.3);
        s.bodies[1].pose.fall = 0.6;
        let b = &s.bodies[1];
        for p in [vec3(0.3, 1.2, -0.7), vec3(-2.0, 0.1, 4.0), Vec3::ZERO] {
            assert!((b.to_world(b.to_local(p)) - p).length() < 1e-4);
        }
    }

    #[test]
    fn bodies_face_each_other() {
        let s = two(Class::Warrior, Class::Warrior);
        let f0 = s.bodies[0].to_world(vec3(0.0, 0.0, 1.0)) - s.bodies[0].to_world(Vec3::ZERO);
        let f1 = s.bodies[1].to_world(vec3(0.0, 0.0, 1.0)) - s.bodies[1].to_world(Vec3::ZERO);
        assert!(f0.x > 0.9 && f1.x < -0.9, "{f0:?} {f1:?}");
        assert!(s.bodies[0].pos.x < 0.0 && s.bodies[1].pos.x > 0.0);
        assert!(((s.bodies[1].pos.x - s.bodies[0].pos.x) - 3.8).abs() < 0.01);
    }

    #[test]
    fn root_steps_toward_opponent_and_fall_lies_down() {
        let mut s = two(Class::Warrior, Class::Warrior);
        s.bodies[0].pose.root = vec3(0.0, 0.0, 1.0);
        let feet = s.bodies[0].to_world(Vec3::ZERO);
        assert!((feet.x - (s.bodies[0].pos.x + 1.0)).abs() < 1e-4);
        s.bodies[0].pose.root = Vec3::ZERO;
        s.bodies[0].pose.fall = 1.0;
        let head = s.bodies[0].to_world(vec3(0.0, 1.7, 0.0));
        assert!(head.y - s.bodies[0].pos.y < 0.4, "head near the ground when fallen: {head:?}");
    }

    #[test]
    fn sdf_hits_both_bodies_and_skips_dimmed() {
        let mut s = two(Class::Warrior, Class::Warrior);
        let chest = |s: &Scene, i: usize| s.bodies[i].to_world(vec3(0.0, 1.18, 0.0));
        for i in 0..2 {
            let (d, id, _) = s.sdf(chest(&s, i));
            assert!(d < 0.0 && Scene::owner(id) == Some(i));
        }
        s.bodies[1].dim = 1.0;
        let (_, id, _) = s.sdf(chest(&s, 1));
        assert_ne!(Scene::owner(id), Some(1));
    }

    #[test]
    fn shadow_under_feet() {
        let mut s = two(Class::Warrior, Class::Sorcerer);
        let feet = s.bodies[0].to_world(Vec3::ZERO);
        assert!(s.contact_shadow(feet) < 0.5);
        assert_eq!(s.contact_shadow(feet + vec3(2.0, 0.0, 0.0)), 1.0);
        s.bodies[0].pose.root = vec3(0.0, 0.0, 1.0);
        let moved = s.bodies[0].to_world(Vec3::ZERO);
        assert!(s.contact_shadow(moved) < 0.5, "shadow follows a lunge");
        s.bodies[0].dim = 1.0;
        assert_eq!(s.contact_shadow(moved), 1.0);
    }
}
