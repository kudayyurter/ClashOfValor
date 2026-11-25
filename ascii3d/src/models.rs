//! Class models: each fighter is a handful of SDF primitives in its own local space
//! (feet at the origin, facing +z, right hand on -x), rebuilt every frame from a pose.
//!
//! `Pose.root` and `Pose.fall` are applied by `scene::Body` as a rigid transform of the
//! whole body, so the builders here only move limbs.

use crate::rules::Class;
use crate::scene::{Mat, Part, Shape};
use macroquad::math::{Vec3, vec3};

/// Every field is a 0-centred amount; `Default` is the neutral stance.
#[derive(Clone, Copy, Default, Debug)]
pub struct Pose {
    /// Local offset of the whole body; +z steps toward the opponent.
    pub root: Vec3,
    /// Torso lean forward (+) or back.
    pub lean: f32,
    /// Knee bend / lowered stance, 0..1.
    pub crouch: f32,
    /// Arm swing forward (+) or back.
    pub arm_r: f32,
    pub arm_l: f32,
    /// Raises the weapon or staff.
    pub weapon: f32,
    /// Raises the shield or off hand.
    pub guard: f32,
    /// 0 standing .. 1 lying on the ground.
    pub fall: f32,
    /// Small vertical bob.
    pub breath: f32,
}

/// Points effects attach to, in local space.
#[derive(Clone, Copy, Default, Debug)]
pub struct Anchors {
    pub weapon_tip: Vec3,
    pub hand_r: Vec3,
    pub chest: Vec3,
    #[allow(dead_code)] // kept for future head-level effects
    pub head: Vec3,
    /// Staff crystal for the Sorcerer, the chest otherwise.
    pub focus: Vec3,
}

/// Local-space bounding sphere that holds every part of every pose.
pub const BOUND_CENTER: Vec3 = Vec3::new(0.0, 1.0, 0.0);
pub const BOUND_RADIUS: f32 = 1.9;

/// Part ids are `id_base + k`, `k < 40`.
pub fn build(class: Class, pose: &Pose, id_base: u16, chain_legs: bool) -> (Vec<Part>, Anchors) {
    match class {
        Class::Warrior => warrior(pose, id_base, chain_legs),
        Class::Assassin => assassin(pose, id_base),
        Class::Sorcerer => sorcerer(pose, id_base),
    }
}

/// Collects parts with consecutive ids from `id_base`.
struct Kit {
    parts: Vec<Part>,
    next: u16,
}

impl Kit {
    fn new(id_base: u16) -> Kit {
        Kit { parts: Vec::new(), next: id_base }
    }

    fn add(&mut self, mat: Mat, shape: Shape) {
        self.parts.push(Part { id: self.next, mat, shape });
        self.next += 1;
    }
}

/// The armoured knight: plate torso and pauldrons, chainmail coif and arms, a tabard in
/// the player's accent, and a sword.
fn warrior(pose: &Pose, id_base: u16, chain_legs: bool) -> (Vec<Part>, Anchors) {
    // Upper body offset: breathing, crouch and lean move everything above the hips.
    let up = vec3(0.0, pose.breath * 0.02 - pose.crouch * 0.12, pose.lean * 0.12);
    let legs = if chain_legs { Mat::Chain } else { Mat::Plate };
    let hand_r = vec3(-0.33, 0.98 + pose.weapon.max(0.0) * 0.32, pose.arm_r * 0.4) + up;
    let hand_l = vec3(0.33, 0.98 + pose.guard * 0.22, pose.arm_l * 0.3 + pose.guard * 0.22) + up;
    // The sword pitches up as it is raised and down through a cut.
    let pitch = 0.08 + pose.weapon * 1.0;
    let tip = hand_r + vec3(0.0, pitch.sin(), pitch.cos()) * 0.95;
    let hip = vec3(0.0, 0.88 - pose.crouch * 0.12, 0.0);
    let mut k = Kit::new(id_base);
    k.add(legs, Shape::Capsule { a: hip + vec3(0.11, 0.0, 0.0), b: vec3(0.11, 0.1, 0.0), r: 0.085 });
    k.add(legs, Shape::Capsule { a: hip + vec3(-0.11, 0.0, 0.0), b: vec3(-0.11, 0.1, 0.0), r: 0.085 });
    k.add(Mat::Plate, Shape::RBox { c: vec3(0.0, 1.18, 0.0) + up, half: vec3(0.2, 0.26, 0.12), round: 0.06 });
    k.add(Mat::Plate, Shape::Sphere { c: vec3(0.27, 1.42, 0.0) + up, r: 0.105 });
    k.add(Mat::Plate, Shape::Sphere { c: vec3(-0.27, 1.42, 0.0) + up, r: 0.105 });
    k.add(Mat::Chain, Shape::Capsule { a: vec3(0.29, 1.36, 0.0) + up, b: hand_l, r: 0.065 });
    k.add(Mat::Chain, Shape::Capsule { a: vec3(-0.29, 1.36, 0.0) + up, b: hand_r, r: 0.065 });
    // The coif: a hood of chainmail over the head that drapes onto the shoulders.
    k.add(Mat::Chain, Shape::Sphere { c: vec3(0.0, 1.66, 0.0) + up, r: 0.16 });
    k.add(Mat::Chain, Shape::Capsule { a: vec3(0.0, 1.47, 0.0) + up, b: vec3(0.0, 1.53, 0.0) + up, r: 0.17 });
    k.add(Mat::Cloth, Shape::RBox { c: vec3(0.0, 1.1, -0.17) + up, half: vec3(0.22, 0.4, 0.02), round: 0.01 });
    let dir = (tip - hand_r).normalize();
    k.add(Mat::Blade, Shape::Capsule { a: hand_r + dir * 0.1, b: tip, r: 0.042 });
    k.add(Mat::Plate, Shape::Capsule { a: hand_r + dir * 0.08 + vec3(0.11, 0.0, 0.0), b: hand_r + dir * 0.08 - vec3(0.11, 0.0, 0.0), r: 0.02 });
    // Front of the surcoat and a helmet plume, both in the player's accent.
    k.add(Mat::Cloth, Shape::RBox { c: vec3(0.0, 0.95, 0.13) + up, half: vec3(0.16, 0.2, 0.015), round: 0.01 });
    k.add(Mat::Cloak, Shape::Capsule { a: vec3(0.0, 1.8, 0.02) + up, b: vec3(0.0, 1.88, -0.2) + up, r: 0.045 });
    // Kite shield strapped to the left forearm.
    k.add(Mat::Plate, Shape::RBox { c: hand_l + vec3(0.07, 0.1, 0.04), half: vec3(0.03, 0.24, 0.17), round: 0.03 });
    let chest = vec3(0.0, 1.2, 0.12) + up;
    let anchors = Anchors { weapon_tip: tip, hand_r, chest, head: vec3(0.0, 1.66, 0.0) + up, focus: chest };
    (k.parts, anchors)
}

/// Robed spellcaster: a robe flared to the ground, a tall pointed hat, long sleeves and
/// a staff taller than the body with a glowing crystal on top.
fn sorcerer(pose: &Pose, id_base: u16) -> (Vec<Part>, Anchors) {
    let up = vec3(0.0, pose.breath * 0.02, 0.0);
    let lean = vec3(0.0, 0.0, pose.lean * 0.12);
    let hand_r = vec3(-0.3, 1.02 + pose.weapon * 0.28, 0.16 + pose.arm_r * 0.15) + up;
    let hand_l = vec3(0.27, 1.0 + pose.guard * 0.3, 0.1 + pose.arm_l * 0.35) + up;
    // The staff leans toward the opponent as it is raised.
    let tilt = pose.weapon * 0.6;
    let dir = vec3(0.0, tilt.cos(), tilt.sin());
    let top = hand_r + dir * 1.1;
    let crystal = top + dir * 0.1;
    let mut k = Kit::new(id_base);
    k.add(Mat::Robe, Shape::Cone { a: vec3(0.0, 0.1, 0.0), b: vec3(0.0, 1.3, 0.0) + up + lean, ra: 0.36, rb: 0.16 });
    k.add(Mat::Cloak, Shape::Cone { a: vec3(0.0, 0.0, 0.0), b: vec3(0.0, 0.13, 0.0), ra: 0.39, rb: 0.36 });
    k.add(Mat::Robe, Shape::Ellipsoid { c: vec3(0.0, 1.34, 0.0) + up + lean, rad: vec3(0.21, 0.17, 0.14) });
    k.add(Mat::Robe, Shape::Capsule { a: vec3(-0.21, 1.4, 0.0) + up + lean, b: hand_r, r: 0.08 });
    k.add(Mat::Robe, Shape::Capsule { a: vec3(0.21, 1.4, 0.0) + up + lean, b: hand_l, r: 0.08 });
    k.add(Mat::Robe, Shape::Sphere { c: vec3(0.0, 1.6, 0.0) + up + lean, r: 0.135 });
    // Hat: a wide brim and a cone that bends back to a point.
    k.add(Mat::Robe, Shape::Cone { a: vec3(0.0, 1.66, 0.0) + up + lean, b: vec3(0.0, 1.7, 0.0) + up + lean, ra: 0.32, rb: 0.22 });
    k.add(Mat::Robe, Shape::Cone { a: vec3(0.0, 1.69, 0.0) + up + lean, b: vec3(0.0, 2.25, -0.1) + up + lean, ra: 0.19, rb: 0.01 });
    k.add(Mat::Wood, Shape::Capsule { a: hand_r - dir * 0.95, b: top, r: 0.03 });
    k.add(Mat::Crystal, Shape::Sphere { c: crystal, r: 0.075 });
    let chest = vec3(0.0, 1.3, 0.14) + up + lean;
    let anchors = Anchors { weapon_tip: crystal, hand_r, chest, head: vec3(0.0, 1.62, 0.0) + up + lean, focus: crystal };
    (k.parts, anchors)
}

/// Hooded skirmisher: slimmer and shorter than the knight, crouched, in dark leathers
/// with a short accent-coloured cloak and two daggers held low.
fn assassin(pose: &Pose, id_base: u16) -> (Vec<Part>, Anchors) {
    let crouch = 0.35 + pose.crouch;
    let drop = vec3(0.0, -crouch * 0.12 + pose.breath * 0.02, 0.0);
    let lean = vec3(0.0, 0.0, 0.06 + pose.lean * 0.12);
    let body = drop + lean;
    let hand_r = vec3(-0.25, 0.86 + pose.weapon * 0.25, 0.2 + pose.arm_r * 0.35) + drop;
    let hand_l = vec3(0.25, 0.86 + pose.guard * 0.25, 0.16 + pose.arm_l * 0.35) + drop;
    let blade = |h: Vec3| (h + vec3(0.0, 0.0, 0.05), h + vec3(0.0, 0.06, 0.36));
    let mut k = Kit::new(id_base);
    for x in [0.1f32, -0.1] {
        let hip = vec3(x, 0.8, 0.0) + drop;
        let knee = vec3(x * 1.1, 0.44 - crouch * 0.05, 0.08 + crouch * 0.08);
        let foot = vec3(x * 1.2, 0.07, x.signum() * 0.07);
        k.add(Mat::Leather, Shape::Capsule { a: hip, b: knee, r: 0.07 });
        k.add(Mat::Leather, Shape::Capsule { a: knee, b: foot, r: 0.062 });
    }
    k.add(Mat::Leather, Shape::RBox { c: vec3(0.0, 1.06, 0.0) + body, half: vec3(0.16, 0.22, 0.1), round: 0.06 });
    k.add(Mat::Cloak, Shape::Cone { a: vec3(0.0, 1.16, -0.01) + body, b: vec3(0.0, 1.36, 0.0) + body, ra: 0.26, rb: 0.13 });
    k.add(Mat::Cloak, Shape::RBox { c: vec3(0.0, 0.98, -0.13) + body, half: vec3(0.2, 0.3, 0.02), round: 0.015 });
    k.add(Mat::Leather, Shape::Capsule { a: vec3(-0.19, 1.26, 0.0) + body, b: hand_r, r: 0.055 });
    k.add(Mat::Leather, Shape::Capsule { a: vec3(0.19, 1.26, 0.0) + body, b: hand_l, r: 0.055 });
    // Hood with a peak that falls back behind the head.
    k.add(Mat::Cloak, Shape::Sphere { c: vec3(0.0, 1.46, 0.0) + body, r: 0.135 });
    k.add(Mat::Cloak, Shape::Cone { a: vec3(0.0, 1.5, -0.04) + body, b: vec3(0.0, 1.64, -0.18) + body, ra: 0.1, rb: 0.01 });
    let (a, tip) = blade(hand_r);
    k.add(Mat::Blade, Shape::Capsule { a, b: tip, r: 0.03 });
    let (a2, b2) = blade(hand_l);
    k.add(Mat::Blade, Shape::Capsule { a: a2, b: b2, r: 0.03 });
    let chest = vec3(0.0, 1.1, 0.11) + body;
    let anchors = Anchors { weapon_tip: tip, hand_r, chest, head: vec3(0.0, 1.46, 0.0) + body, focus: chest };
    (k.parts, anchors)
}

#[cfg(test)]
pub mod tests {
    use super::*;

    /// A sphere that contains the shape (centre, radius).
    pub fn extent(s: &Shape) -> (Vec3, f32) {
        match *s {
            Shape::Sphere { c, r } => (c, r),
            Shape::Rock { c, r } => (c, r + 0.1),
            Shape::Capsule { a, b, r } => ((a + b) * 0.5, (b - a).length() * 0.5 + r),
            Shape::RBox { c, half, .. } => (c, half.length()),
            Shape::Ellipsoid { c, rad } => (c, rad.max_element()),
            Shape::Cone { a, b, ra, rb } => ((a + b) * 0.5, (b - a).length() * 0.5 + ra.max(rb)),
        }
    }

    pub fn assert_inside_bound(parts: &[Part], what: &str) {
        for p in parts {
            let (c, r) = extent(&p.shape);
            let reach = (c - BOUND_CENTER).length() + r;
            assert!(reach <= BOUND_RADIUS, "{what}: part {} reaches {reach:.2} > {BOUND_RADIUS}", p.id);
        }
    }

    #[test]
    fn ids_in_range_and_unique() {
        for class in Class::ALL {
            let (parts, _) = build(class, &Pose::default(), 50, true);
            assert!(!parts.is_empty());
            let mut ids: Vec<u16> = parts.iter().map(|p| p.id).collect();
            assert!(ids.iter().all(|&i| (50..90).contains(&i)), "{class:?} ids {ids:?}");
            ids.sort();
            ids.dedup();
            assert_eq!(ids.len(), parts.len(), "{class:?} has duplicate ids");
        }
    }

    #[test]
    fn neutral_pose_inside_bound() {
        for class in Class::ALL {
            let (parts, _) = build(class, &Pose::default(), 10, true);
            assert_inside_bound(&parts, &format!("{class:?} neutral"));
        }
    }

    #[test]
    fn warrior_keeps_hero_materials() {
        let (parts, a) = build(Class::Warrior, &Pose::default(), 10, true);
        assert!(parts.iter().all(|p| p.mat.is_hero()));
        assert!(parts.iter().any(|p| p.mat == Mat::Blade));
        assert!(a.weapon_tip.z > 0.5, "sword points forward");
    }

    /// Highest point of the model, found by scanning down a grid of vertical rays.
    fn top(parts: &[Part]) -> f32 {
        let mut y = 3.0;
        while y > 0.0 {
            for xi in -15..=15 {
                for zi in -15..=15 {
                    let p = vec3(xi as f32 * 0.04, y, zi as f32 * 0.04);
                    if parts.iter().any(|q| q.shape.dist(p) <= 0.0) {
                        return y;
                    }
                }
            }
            y -= 0.01;
        }
        0.0
    }

    #[test]
    fn sorcerer_is_tall_pointed_with_crystal_staff() {
        let (w, _) = build(Class::Warrior, &Pose::default(), 10, true);
        let (s, a) = build(Class::Sorcerer, &Pose::default(), 10, true);
        assert!(top(&s) > top(&w) + 0.3, "hat/staff tower over the knight: {} vs {}", top(&s), top(&w));
        let crystal = s.iter().find(|p| p.mat == Mat::Crystal).expect("crystal");
        let (c, _) = extent(&crystal.shape);
        assert!((a.focus - c).length() < 0.15, "focus sits on the crystal");
        assert!(s.iter().any(|p| p.mat == Mat::Robe));
        assert!(s.iter().any(|p| p.mat == Mat::Wood), "staff");
    }

    #[test]
    fn assassin_shorter_with_two_daggers() {
        let (w, _) = build(Class::Warrior, &Pose::default(), 10, true);
        let (s, a) = build(Class::Assassin, &Pose::default(), 10, true);
        assert!(top(&s) < top(&w) * 0.95, "assassin {} vs warrior {}", top(&s), top(&w));
        assert_eq!(s.iter().filter(|p| p.mat == Mat::Blade).count(), 2);
        assert!(s.iter().any(|p| p.mat == Mat::Leather) && s.iter().any(|p| p.mat == Mat::Cloak));
        assert!(a.weapon_tip.y < 1.1, "daggers held low");
    }

    #[test]
    fn warrior_has_shield_on_left_arm() {
        let (w, _) = build(Class::Warrior, &Pose::default(), 10, true);
        assert!(w.iter().any(|p| matches!(p.shape, Shape::RBox { c, half, .. } if p.mat == Mat::Plate && c.x > 0.3 && half.z > 0.12 && half.y > 0.15)));
    }

    #[test]
    fn new_materials_draw_on_hero_grid() {
        for m in [Mat::Robe, Mat::Leather, Mat::Crystal, Mat::Cloak] {
            assert!(m.is_hero(), "{m:?}");
        }
        assert!(!Mat::Orb.is_hero());
    }

    fn extremes() -> Vec<Pose> {
        let mut v = vec![Pose::default()];
        for k in [-1.0f32, 1.0] {
            v.push(Pose { weapon: k, arm_r: k, arm_l: k, guard: k, lean: k * 0.6, crouch: 0.6, breath: k * 2.0, ..Pose::default() });
            v.push(Pose { weapon: k, arm_r: -k, guard: 1.0, lean: 0.6, ..Pose::default() });
        }
        v
    }

    #[test]
    fn extreme_poses_inside_bound() {
        for class in Class::ALL {
            for (n, pose) in extremes().iter().enumerate() {
                let (parts, _) = build(class, pose, 10, true);
                assert_inside_bound(&parts, &format!("{class:?} pose {n}"));
            }
        }
    }

    #[test]
    fn warrior_sword_and_shield_follow_pose() {
        let (_, rest) = build(Class::Warrior, &Pose::default(), 10, true);
        let (_, up) = build(Class::Warrior, &Pose { weapon: 1.0, ..Pose::default() }, 10, true);
        let (_, down) = build(Class::Warrior, &Pose { weapon: -0.7, arm_r: 0.9, ..Pose::default() }, 10, true);
        assert!(up.weapon_tip.y > rest.weapon_tip.y + 0.4, "raised sword");
        assert!(down.weapon_tip.y < rest.weapon_tip.y, "cut down");
        let shield = |p: &Pose| {
            let (parts, _) = build(Class::Warrior, p, 10, true);
            parts.iter().find_map(|q| match q.shape {
                Shape::RBox { c, half, .. } if q.mat == Mat::Plate && half.z > 0.12 => Some(c),
                _ => None,
            }).unwrap()
        };
        assert!(shield(&Pose { guard: 1.0, ..Pose::default() }).z > shield(&Pose::default()).z + 0.15);
        let (_, lean) = build(Class::Warrior, &Pose { lean: 1.0, ..Pose::default() }, 10, true);
        assert!(lean.chest.z > rest.chest.z + 0.05);
    }
}
