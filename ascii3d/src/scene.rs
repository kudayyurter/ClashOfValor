//! The 3D world: signed distance functions (SDFs), materials, ray marching and lighting.
//!
//! Nothing in this file knows about ASCII. It answers one question: "what does a ray
//! through this pixel hit, and how bright and what colour is it there?". The ASCII layer
//! (ascii.rs) asks that question several times per character cell and turns the answers
//! into a glyph.

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
}

impl Mat {
    /// Hero materials are drawn on the fine (half-size) glyph grid.
    pub fn is_hero(self) -> bool {
        matches!(self, Mat::Plate | Mat::Chain | Mat::Blade | Mat::Cloth)
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
            Shape::Capsule { a, b, .. } => {
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
    pub player: Vec3,
    pub yaw: f32,
    walk: f32,
    stride: f32,
    /// Hero parts in the hero's local space (feet at the origin, facing +z).
    pub hero: Vec<Part>,
    pub props: Vec<Prop>,
    pub fire: Vec3,
    pub flicker: f32,
    pub metal: usize,
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

fn lerp_angle(a: f32, b: f32, t: f32) -> f32 {
    use std::f32::consts::{PI, TAU};
    let d = (b - a + PI).rem_euclid(TAU) - PI;
    a + d * t
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
            (vec3(5.0, 0.1, -2.0), 1.2),
            (vec3(-2.0, 0.0, -5.0), 0.7),
            (vec3(7.0, 0.2, 5.0), 1.0),
            (vec3(-7.0, 0.2, -1.0), 1.3),
            (vec3(1.5, 0.0, 6.0), 0.5),
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

        let fire = vec3(2.5, ground_height(2.5, 2.0), 2.0);
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

        let mut s = Scene {
            time: 0.0,
            player: Vec3::ZERO,
            yaw: 0.4,
            walk: 0.0,
            stride: 0.0,
            hero: Vec::new(),
            props,
            fire,
            flicker: 1.0,
            metal: 0,
            chain_legs: true,
            sun: vec3(0.5, 0.8, 0.3).normalize(),
        };
        s.build_hero();
        s
    }

    pub fn update(&mut self, dt: f32, move_dir: Vec3) {
        self.time += dt;
        if move_dir.length_squared() > 0.0 {
            let d = move_dir.normalize();
            self.player += d * 3.0 * dt;
            self.yaw = lerp_angle(self.yaw, d.x.atan2(d.z), (dt * 12.0).min(1.0));
            self.stride = (self.stride + dt * 6.0).min(1.0);
        } else {
            self.stride = (self.stride - dt * 6.0).max(0.0);
        }
        self.walk += dt * 9.0 * self.stride;
        self.player.y = ground_height(self.player.x, self.player.z);
        self.flicker = 0.85 + 0.15 * (self.time * 13.0).sin() * (self.time * 7.3).cos();
        self.build_hero();
    }

    /// Rebuild the knight from primitives each frame so the limbs can swing.
    fn build_hero(&mut self) {
        let s = self.walk.sin() * 0.22 * self.stride;
        let legs = if self.chain_legs { Mat::Chain } else { Mat::Plate };
        let hand_r = vec3(-0.33, 0.98, s * 0.8);
        let p = |id, mat, shape| Part { id, mat, shape };
        self.hero = vec![
            p(10, legs, Shape::Capsule { a: vec3(0.11, 0.88, 0.0), b: vec3(0.11, 0.1, s), r: 0.085 }),
            p(11, legs, Shape::Capsule { a: vec3(-0.11, 0.88, 0.0), b: vec3(-0.11, 0.1, -s), r: 0.085 }),
            p(12, Mat::Plate, Shape::RBox { c: vec3(0.0, 1.18, 0.0), half: vec3(0.2, 0.26, 0.12), round: 0.06 }),
            p(13, Mat::Plate, Shape::Sphere { c: vec3(0.27, 1.42, 0.0), r: 0.105 }),
            p(14, Mat::Plate, Shape::Sphere { c: vec3(-0.27, 1.42, 0.0), r: 0.105 }),
            p(15, Mat::Chain, Shape::Capsule { a: vec3(0.29, 1.36, 0.0), b: vec3(0.33, 0.98, -s * 0.8), r: 0.065 }),
            p(16, Mat::Chain, Shape::Capsule { a: vec3(-0.29, 1.36, 0.0), b: hand_r, r: 0.065 }),
            // The coif: a hood of chainmail over the head that drapes onto the shoulders.
            p(17, Mat::Chain, Shape::Sphere { c: vec3(0.0, 1.66, 0.0), r: 0.16 }),
            p(18, Mat::Chain, Shape::Capsule { a: vec3(0.0, 1.47, 0.0), b: vec3(0.0, 1.53, 0.0), r: 0.17 }),
            p(19, Mat::Cloth, Shape::RBox { c: vec3(0.0, 1.1, -0.17), half: vec3(0.22, 0.4, 0.02), round: 0.01 }),
            p(20, Mat::Blade, Shape::Capsule { a: hand_r + vec3(0.0, 0.0, 0.1), b: hand_r + vec3(0.0, 0.08, 0.95), r: 0.028 }),
            p(21, Mat::Plate, Shape::RBox { c: hand_r + vec3(0.0, 0.0, 0.08), half: vec3(0.11, 0.02, 0.02), round: 0.01 }),
        ];
    }

    pub fn hero_part(&self, id: u16) -> Option<&Part> {
        self.hero.iter().find(|p| p.id == id)
    }

    /// World offset from the player -> hero local space.
    pub fn to_local(&self, d: Vec3) -> Vec3 {
        let (s, c) = self.yaw.sin_cos();
        vec3(d.x * c - d.z * s, d.y, d.x * s + d.z * c)
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
        let dp = p - self.player;
        if (dp - vec3(0.0, 1.0, 0.0)).length() - 1.7 < best.0 {
            let lp = self.to_local(dp);
            for part in &self.hero {
                let d = part.shape.dist(lp);
                if d < best.0 {
                    best = (d, part.id, part.mat);
                }
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
        let metal = METALS[self.metal].rgb;
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
            Mat::Cloth => vec3(0.2, 0.26, 0.8),
            Mat::Fire | Mat::Sky => Vec3::ONE,
        }
    }

    /// Light one hit point: moonlight with soft shadows, a flickering campfire, metal glints, fog.
    pub fn shade(&self, ro: Vec3, rd: Vec3, hit: &Hit) -> Shade {
        let p = ro + rd * hit.t;
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
        if matches!(hit.mat, Mat::Plate | Mat::Blade | Mat::Chain) {
            let r = rd - n * 2.0 * rd.dot(n);
            let spec = r.dot(self.sun).max(0.0).powf(24.0) * 0.7 * (0.3 + 0.7 * diff);
            rgb += Vec3::splat(spec);
        }
        rgb *= 1.0 - (hit.t / MAX_DIST).powf(0.7);
        let lum = rgb.dot(vec3(0.3, 0.59, 0.11)).clamp(0.0, 1.0);
        Shade { rgb, lum, p }
    }
}
