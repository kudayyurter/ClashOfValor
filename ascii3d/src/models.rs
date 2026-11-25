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
    pub head: Vec3,
    /// Staff crystal for the Sorcerer, the chest otherwise.
    pub focus: Vec3,
}

/// Local-space bounding sphere that holds every part of every pose.
pub const BOUND_CENTER: Vec3 = Vec3::new(0.0, 1.0, 0.0);
pub const BOUND_RADIUS: f32 = 1.7;

/// Part ids are `id_base + k`, `k < 40`.
pub fn build(class: Class, pose: &Pose, id_base: u16, chain_legs: bool) -> (Vec<Part>, Anchors) {
    match class {
        // Assassin and Sorcerer get their own models in a later step.
        Class::Warrior | Class::Assassin | Class::Sorcerer => warrior(pose, id_base, chain_legs),
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
    let up = vec3(0.0, pose.breath * 0.02, 0.0);
    let legs = if chain_legs { Mat::Chain } else { Mat::Plate };
    let hand_r = vec3(-0.33, 0.98, pose.arm_r * 0.4) + up;
    let hand_l = vec3(0.33, 0.98, pose.arm_l * 0.4) + up;
    let tip = hand_r + vec3(0.0, 0.08, 0.95);
    let mut k = Kit::new(id_base);
    k.add(legs, Shape::Capsule { a: vec3(0.11, 0.88, 0.0), b: vec3(0.11, 0.1, 0.0), r: 0.085 });
    k.add(legs, Shape::Capsule { a: vec3(-0.11, 0.88, 0.0), b: vec3(-0.11, 0.1, 0.0), r: 0.085 });
    k.add(Mat::Plate, Shape::RBox { c: vec3(0.0, 1.18, 0.0) + up, half: vec3(0.2, 0.26, 0.12), round: 0.06 });
    k.add(Mat::Plate, Shape::Sphere { c: vec3(0.27, 1.42, 0.0) + up, r: 0.105 });
    k.add(Mat::Plate, Shape::Sphere { c: vec3(-0.27, 1.42, 0.0) + up, r: 0.105 });
    k.add(Mat::Chain, Shape::Capsule { a: vec3(0.29, 1.36, 0.0) + up, b: hand_l, r: 0.065 });
    k.add(Mat::Chain, Shape::Capsule { a: vec3(-0.29, 1.36, 0.0) + up, b: hand_r, r: 0.065 });
    // The coif: a hood of chainmail over the head that drapes onto the shoulders.
    k.add(Mat::Chain, Shape::Sphere { c: vec3(0.0, 1.66, 0.0) + up, r: 0.16 });
    k.add(Mat::Chain, Shape::Capsule { a: vec3(0.0, 1.47, 0.0) + up, b: vec3(0.0, 1.53, 0.0) + up, r: 0.17 });
    k.add(Mat::Cloth, Shape::RBox { c: vec3(0.0, 1.1, -0.17) + up, half: vec3(0.22, 0.4, 0.02), round: 0.01 });
    k.add(Mat::Blade, Shape::Capsule { a: hand_r + vec3(0.0, 0.0, 0.1), b: tip, r: 0.028 });
    k.add(Mat::Plate, Shape::RBox { c: hand_r + vec3(0.0, 0.0, 0.08), half: vec3(0.11, 0.02, 0.02), round: 0.01 });
    let chest = vec3(0.0, 1.2, 0.12) + up;
    let anchors = Anchors { weapon_tip: tip, hand_r, chest, head: vec3(0.0, 1.66, 0.0) + up, focus: chest };
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
}
