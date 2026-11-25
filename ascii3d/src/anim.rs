//! Move timelines and poses.
//!
//! An `Action` is one move playing out: a fixed duration with an impact moment. The
//! rules have already resolved the move; the numbers on screen change at the impact.

use crate::models::Pose;
use crate::rules::{Class, Event, Move, Resolution};
use macroquad::math::vec3;

/// Seconds the action freezes at a hit's impact.
pub const HIT_STOP: f32 = 0.08;

pub struct Action {
    pub res: Resolution,
    pub t: f32,
    pub dur: f32,
    pub impact: f32,
    /// Hit-stop time left; while above 0 the clock does not move.
    pub hold: f32,
    impact_done: bool,
}

/// (duration, impact) in seconds.
pub fn timing(mv: Move) -> (f32, f32) {
    match mv {
        Move::Attack => (1.2, 0.55),
        Move::CritAttack => (1.4, 0.75),
        Move::FireOrb => (1.5, 1.05),
        Move::RaiseShield => (1.0, 0.45),
        Move::HealthPotion | Move::ManaPotion => (1.1, 0.5),
        Move::Rest => (1.1, 0.5),
        Move::Flee => (1.2, 1.0),
    }
}

impl Action {
    pub fn new(res: Resolution) -> Action {
        let (dur, impact) = timing(res.mv);
        Action { res, t: 0.0, dur, impact, hold: 0.0, impact_done: false }
    }

    /// True exactly on the call where `t` first reaches `impact`.
    pub fn advance(&mut self, dt: f32) -> bool {
        let mut dt = dt;
        if self.hold > 0.0 {
            let used = dt.min(self.hold);
            self.hold -= used;
            dt -= used;
        }
        let before = self.t;
        self.t = (self.t + dt).min(self.dur);
        let has_hit = self.res.events.iter().any(|e| matches!(e, Event::Hit { .. }));
        if has_hit && !self.impact_done && before < self.impact && self.t >= self.impact {
            // Freeze on the impact frame; any time left over is spent in the hold.
            let over = self.t - self.impact;
            self.t = self.impact;
            self.hold = HIT_STOP;
            let used = over.min(self.hold);
            self.hold -= used;
            self.t += over - used;
        }
        self.fire()
    }

    /// Jump to the end. True if the impact had not fired yet.
    pub fn skip(&mut self) -> bool {
        self.hold = 0.0;
        self.t = self.dur;
        self.fire()
    }

    fn fire(&mut self) -> bool {
        if !self.impact_done && self.t >= self.impact {
            self.impact_done = true;
            return true;
        }
        false
    }

    pub fn done(&self) -> bool {
        self.t >= self.dur
    }

    pub fn phase(&self) -> f32 {
        (self.t / self.dur).clamp(0.0, 1.0)
    }
}

fn smooth(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// 0 before `a`, rising to 1 at `b`.
fn ramp(p: f32, a: f32, b: f32) -> f32 {
    smooth((p - a) / (b - a))
}

/// Rises over a..b, holds, falls over c..d.
fn hump(p: f32, a: f32, b: f32, c: f32, d: f32) -> f32 {
    ramp(p, a, b) * (1.0 - ramp(p, c, d))
}

/// The acting fighter's pose. `gap` is the distance to the opponent.
pub fn actor_pose(class: Class, mv: Move, phase: f32, gap: f32) -> Pose {
    let p = phase.clamp(0.0, 1.0);
    let reach = (gap - 0.9).max(0.0);
    let mut o = Pose::default();
    match mv {
        Move::Attack => {
            let (d, i) = timing(mv);
            let hit = i / d;
            o.root = vec3(0.0, 0.0, reach * hump(p, 0.05, hit - 0.05, hit + 0.12, 0.95));
            o.lean = 0.5 * hump(p, 0.2, hit, hit + 0.1, 0.9);
            if class == Class::Assassin {
                // Quick stab: draw back, then thrust both daggers.
                o.arm_r = -0.6 * hump(p, 0.1, 0.3, 0.3, hit) + 1.0 * hump(p, 0.3, hit, hit + 0.1, 0.9);
                o.arm_l = 0.6 * hump(p, 0.35, hit, hit + 0.1, 0.9);
                o.crouch = 0.2 * hump(p, 0.1, 0.3, hit, 0.9);
            } else {
                // Overhead wind-up, then a cut down through the target.
                let up = hump(p, 0.1, 0.32, 0.36, hit);
                let cut = hump(p, 0.36, hit, hit + 0.1, 0.92);
                o.weapon = up - 0.7 * cut;
                o.arm_r = -0.5 * up + 0.9 * cut;
                o.guard = 0.5 * hump(p, 0.1, 0.3, 0.7, 0.95);
            }
        }
        Move::CritAttack => {
            // Vanish at home, reappear beside the target, strike, slip back.
            let (d, i) = timing(mv);
            let hit = i / d;
            let there = ramp(p, 0.36, 0.42) * (1.0 - ramp(p, 0.78, 0.98));
            o.root = vec3(0.35 * there, 0.0, (gap - 0.7).max(0.0) * there);
            o.arm_r = 1.0 * hump(p, 0.45, hit, hit + 0.05, 0.8);
            o.arm_l = 0.8 * hump(p, 0.47, hit, hit + 0.05, 0.8);
            o.lean = 0.6 * hump(p, 0.45, hit, hit + 0.05, 0.8);
            o.crouch = 0.4 * hump(p, 0.05, 0.3, 0.6, 0.9);
        }
        Move::FireOrb => {
            o.weapon = hump(p, 0.15, 0.35, 0.72, 0.95);
            o.guard = 0.8 * hump(p, 0.3, 0.45, 0.6, 0.8);
            o.arm_l = 0.6 * hump(p, 0.3, 0.45, 0.6, 0.8);
            o.lean = 0.3 * hump(p, 0.35, 0.45, 0.6, 0.85);
        }
        Move::RaiseShield => {
            o.guard = hump(p, 0.1, 0.45, 0.75, 1.0);
            o.arm_l = 0.4 * hump(p, 0.1, 0.45, 0.75, 1.0);
            o.crouch = 0.15 * hump(p, 0.1, 0.45, 0.75, 1.0);
        }
        Move::HealthPotion | Move::ManaPotion => {
            o.guard = hump(p, 0.1, 0.35, 0.6, 0.9);
            o.lean = -0.4 * hump(p, 0.25, 0.4, 0.6, 0.85);
        }
        Move::Rest => {
            o.weapon = -0.5 * hump(p, 0.1, 0.35, 0.7, 1.0);
            o.crouch = 0.3 * hump(p, 0.1, 0.35, 0.7, 1.0);
            o.breath = 2.0 * (p * 12.0).sin() * hump(p, 0.1, 0.3, 0.7, 1.0);
        }
        Move::Flee => {
            o.root = vec3(0.0, 0.0, -1.6 * ramp(p, 0.1, 1.0));
            o.lean = -0.3 * hump(p, 0.0, 0.2, 0.5, 1.0);
        }
    }
    o
}

/// How faded the acting fighter is (crit vanish, flee).
pub fn actor_dim(mv: Move, phase: f32) -> f32 {
    let p = phase.clamp(0.0, 1.0);
    match mv {
        Move::CritAttack => 0.9 * hump(p, 0.1, 0.35, 0.45, 0.6),
        Move::Flee => ramp(p, 0.2, 1.0),
        _ => 0.0,
    }
}

/// A short recoil after being hit.
pub fn flinch_pose(since_impact: f32) -> Pose {
    let k = if since_impact < 0.3 { (since_impact / 0.3 * std::f32::consts::PI).sin() } else { 0.0 };
    Pose { lean: -0.6 * k, root: vec3(0.0, 0.0, -0.18 * k), guard: 0.4 * k, ..Pose::default() }
}

pub fn idle_pose(time: f32, who: usize) -> Pose {
    Pose { breath: (time * 2.1 + who as f32 * 1.9).sin(), ..Pose::default() }
}

pub fn collapse_pose(f: f32) -> Pose {
    // Ease in, as if the legs give out.
    let f = f.clamp(0.0, 1.0);
    Pose { fall: f * f, lean: -0.3 * f, ..Pose::default() }
}

/// Field-wise sum of two poses.
pub fn add(a: Pose, b: Pose) -> Pose {
    Pose {
        root: a.root + b.root,
        lean: a.lean + b.lean,
        crouch: a.crouch + b.crouch,
        arm_r: a.arm_r + b.arm_r,
        arm_l: a.arm_l + b.arm_l,
        weapon: a.weapon + b.weapon,
        guard: a.guard + b.guard,
        fall: a.fall + b.fall,
        breath: a.breath + b.breath,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Move::*;

    fn action(mv: Move) -> Action {
        Action::new(Resolution { actor: 0, mv, events: Vec::new(), narration: Vec::new() })
    }

    #[test]
    fn impact_fires_once() {
        let mut a = action(Attack);
        let mut fired = 0;
        for _ in 0..20 {
            if a.advance(0.1) {
                fired += 1;
            }
        }
        assert_eq!(fired, 1);
        assert!(a.done());
        assert_eq!(a.phase(), 1.0);
    }

    #[test]
    fn skip_before_impact_reports_impact() {
        let mut a = action(FireOrb);
        a.advance(0.1);
        assert!(a.skip());
        assert!(a.done());
        assert!(!a.advance(0.1));
    }

    #[test]
    fn skip_after_impact_does_not_repeat() {
        let mut a = action(Attack);
        let mut fired = false;
        for _ in 0..7 {
            fired |= a.advance(0.1);
        }
        assert!(fired);
        assert!(!a.skip());
    }

    #[test]
    fn timings_per_move() {
        let want = [
            (Attack, 1.2, 0.55),
            (CritAttack, 1.4, 0.75),
            (FireOrb, 1.5, 1.05),
            (RaiseShield, 1.0, 0.45),
            (HealthPotion, 1.1, 0.5),
            (ManaPotion, 1.1, 0.5),
            (Rest, 1.1, 0.5),
            (Flee, 1.2, 1.0),
        ];
        for (m, d, i) in want {
            assert_eq!(timing(m), (d, i), "{m:?}");
            let a = action(m);
            assert_eq!((a.dur, a.impact), (d, i));
            assert!(i < d);
        }
    }

    #[test]
    fn melee_lunge_returns_home() {
        for class in [Class::Warrior, Class::Assassin] {
            assert!(actor_pose(class, Attack, 0.0, 3.8).root.length() < 0.01);
            assert!(actor_pose(class, Attack, 1.0, 3.8).root.length() < 0.01);
            let (d, i) = timing(Attack);
            let z = actor_pose(class, Attack, i / d, 3.8).root.z;
            assert!((2.5..=3.0).contains(&z), "{class:?} at impact z={z}");
        }
    }

    #[test]
    fn crit_vanishes_and_returns() {
        assert_eq!(actor_dim(CritAttack, 0.0), 0.0);
        assert!(actor_dim(CritAttack, 0.4) > 0.8);
        assert_eq!(actor_dim(CritAttack, 1.0), 0.0);
        let (d, i) = timing(CritAttack);
        assert!(actor_pose(Class::Assassin, CritAttack, i / d, 3.8).root.z > 2.5, "beside the target at impact");
        assert!(actor_pose(Class::Assassin, CritAttack, 1.0, 3.8).root.length() < 0.01);
    }

    #[test]
    fn flee_fades_out() {
        assert_eq!(actor_dim(Flee, 0.0), 0.0);
        assert!(actor_dim(Flee, 1.0) >= 0.99);
        assert_eq!(actor_dim(Attack, 0.5), 0.0);
    }

    #[test]
    fn staff_raised_mid_cast() {
        assert!(actor_pose(Class::Sorcerer, FireOrb, 0.4, 3.8).weapon > 0.8);
        assert!(actor_pose(Class::Sorcerer, FireOrb, 0.0, 3.8).weapon.abs() < 0.01);
    }

    #[test]
    fn flinch_settles() {
        assert!(flinch_pose(0.1).lean < -0.1);
        let p = flinch_pose(0.5);
        assert!(p.lean.abs() < 1e-4 && p.root.length() < 1e-4);
    }

    #[test]
    fn collapse_bounds() {
        assert_eq!(collapse_pose(0.0).fall, 0.0);
        assert_eq!(collapse_pose(1.0).fall, 1.0);
        assert!(collapse_pose(0.5).fall > 0.0 && collapse_pose(0.5).fall < 1.0);
    }

    #[test]
    fn idle_breathes_out_of_phase() {
        assert_ne!(idle_pose(1.0, 0).breath, idle_pose(1.0, 1).breath);
        assert!(idle_pose(1.0, 0).breath.abs() <= 1.0);
    }

    fn hit_action(mv: Move) -> Action {
        use crate::rules::Event;
        Action::new(Resolution { actor: 0, mv, events: vec![Event::Hit { target: 1, absorbed: 0, damage: 10, crit: false }], narration: Vec::new() })
    }

    #[test]
    fn hit_stop_freezes_then_resumes() {
        let mut a = hit_action(Attack);
        assert!(a.advance(0.55), "impact on crossing");
        assert_eq!(a.t, 0.55);
        assert!(!a.advance(0.05));
        assert_eq!(a.t, 0.55, "frozen during the hit-stop");
        a.advance(0.05);
        assert!((a.t - 0.57).abs() < 1e-4, "resumes after {HIT_STOP} s: t={}", a.t);
        let mut frames = 0;
        let mut b = hit_action(Attack);
        while !b.done() {
            b.advance(1.0 / 60.0);
            frames += 1;
        }
        let expect = ((1.2 + HIT_STOP) * 60.0f32).round() as i32;
        assert!((frames - expect).abs() <= 1, "{frames} frames vs {expect}");
    }

    #[test]
    fn no_hit_stop_without_hit() {
        let mut a = action(Rest);
        for _ in 0..200 {
            a.advance(1.0 / 60.0);
            assert_eq!(a.hold, 0.0);
        }
    }

    #[test]
    fn skip_clears_hit_stop() {
        let mut a = hit_action(Attack);
        a.advance(0.56);
        assert!(a.hold > 0.0);
        assert!(!a.skip(), "impact already fired");
        assert!(a.done());
        assert_eq!(a.hold, 0.0);
    }
}
