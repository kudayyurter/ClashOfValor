//! The computer opponent: the C++ game's `battleMenuBot` heuristics, with a seeded
//! RNG so its choices are reproducible in tests.

use crate::rules::{Class, Fighter, Move, availability};

/// Seconds the bot "thinks" before acting, so its move doesn't appear instantly.
pub const THINK: f32 = 0.7;

/// xorshift64* — small, fast, deterministic.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        // Zero is a fixed point of xorshift; mix the seed so every seed works.
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    pub fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    /// Uniform in `0..n`.
    pub fn below(&mut self, n: u32) -> u32 {
        ((self.next_u64() >> 32) % n.max(1) as u64) as u32
    }
}

/// The bot's move. Always one `availability` allows; Rest is the fallback.
pub fn choose(f: &Fighter, rng: &mut Rng) -> Move {
    let ok = |m: Move| availability(f, m).is_ok();
    let m = if f.health * 3 < f.max_health && ok(Move::HealthPotion) {
        Move::HealthPotion
    } else {
        match f.class {
            Class::Warrior => {
                if f.shield == 0 && ok(Move::RaiseShield) && rng.below(3) == 0 {
                    Move::RaiseShield
                } else if ok(Move::Attack) {
                    Move::Attack
                } else {
                    Move::Rest
                }
            }
            Class::Assassin => {
                if ok(Move::CritAttack) && rng.below(2) == 0 {
                    Move::CritAttack
                } else if ok(Move::Attack) {
                    Move::Attack
                } else {
                    Move::Rest
                }
            }
            Class::Sorcerer => {
                if f.mana < 10 && ok(Move::ManaPotion) {
                    Move::ManaPotion
                } else if ok(Move::FireOrb) {
                    Move::FireOrb
                } else {
                    Move::Rest
                }
            }
        }
    };
    if ok(m) { m } else { Move::Rest }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bot_heals_when_low() {
        for c in Class::ALL {
            let mut f = Fighter::new("Bot", c);
            f.health = 30;
            assert_eq!(choose(&f, &mut Rng::new(1)), Move::HealthPotion, "{c:?}");
        }
    }

    #[test]
    fn bot_rests_when_broke() {
        for c in Class::ALL {
            let mut f = Fighter::new("Bot", c);
            f.stamina = 0;
            f.mana = 0;
            f.mana_potions = 0;
            assert_eq!(choose(&f, &mut Rng::new(2)), Move::Rest, "{c:?}");
        }
    }

    #[test]
    fn sorcerer_drinks_mana_when_dry() {
        let mut f = Fighter::new("Bot", Class::Sorcerer);
        f.mana = 5;
        assert_eq!(choose(&f, &mut Rng::new(3)), Move::ManaPotion);
    }

    #[test]
    fn bot_attacks_at_full_strength() {
        let w = Fighter::new("Bot", Class::Warrior);
        assert_eq!(choose(&w, &mut Rng::new(4)), Move::Attack, "shield is full, so attack");
        let s = Fighter::new("Bot", Class::Sorcerer);
        assert_eq!(choose(&s, &mut Rng::new(4)), Move::FireOrb);
    }

    #[test]
    fn bot_never_picks_unavailable() {
        let mut rng = Rng::new(99);
        for i in 0..2000 {
            let mut f = Fighter::new("Bot", Class::ALL[i % 3]);
            f.health = 1 + rng.below(100) as i32;
            f.stamina = rng.below(40) as i32;
            f.shield = rng.below(26) as i32;
            f.mana = rng.below(51) as i32;
            f.health_potions = rng.below(3) as i32;
            f.mana_potions = rng.below(4) as i32;
            let m = choose(&f, &mut rng);
            assert!(availability(&f, m).is_ok(), "{m:?} for {f:?}");
            assert_ne!(m, Move::Flee);
        }
    }

    #[test]
    fn bot_deterministic() {
        let mut f = Fighter::new("Bot", Class::Assassin);
        f.stamina = 30;
        let f = f;
        let run = |seed| {
            let mut r = Rng::new(seed);
            (0..20).map(|_| choose(&f, &mut r)).collect::<Vec<_>>()
        };
        assert_eq!(run(7), run(7));
        assert!(run(7).contains(&Move::CritAttack) && run(7).contains(&Move::Attack), "coin flip between crit and attack");
    }

    #[test]
    fn rng_ranges() {
        let mut r = Rng::new(0);
        for _ in 0..1000 {
            let x = r.next_f32();
            assert!((0.0..1.0).contains(&x));
            assert!(r.below(3) < 3);
        }
    }
}
