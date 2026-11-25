//! Duel rules, ported from the C++ game (`src/Character.cpp`, `src/Warrior.cpp`,
//! `src/Assassin.cpp`, `src/Sorcerer.cpp`). Pure logic: no drawing, no input.
//!
//! One deliberate change from C++: a move the fighter cannot pay for is reported by
//! `availability` so the menu can dim it, instead of wasting the turn.

const ATTACK_STAMINA: i32 = 10;
const CRIT_STAMINA: i32 = 20;
const SHIELD_STAMINA: i32 = 15;
const SPELL_MANA: i32 = 10;
const SPELL_BONUS: i32 = 4;
const SHIELD_PER_LEVEL: i32 = 25;
const POTION_HEALTH: i32 = 50;
const POTION_MANA: i32 = 50;
const REST_HEALTH: i32 = 10;
const REST_STAMINA: i32 = 25;
const REST_MANA: i32 = 20;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Class {
    Warrior,
    Assassin,
    Sorcerer,
}

impl Class {
    pub const ALL: [Class; 3] = [Class::Warrior, Class::Assassin, Class::Sorcerer];

    pub fn name(self) -> &'static str {
        match self {
            Class::Warrior => "Warrior",
            Class::Assassin => "Assassin",
            Class::Sorcerer => "Sorcerer",
        }
    }

    /// The one-line description from the C++ class menu.
    pub fn blurb(self) -> &'static str {
        match self {
            Class::Warrior => "carries a shield that absorbs damage before health",
            Class::Assassin => "trades survivability for a high-damage critical strike",
            Class::Sorcerer => "spends mana on fire orbs that hit harder than steel",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Fighter {
    pub name: String,
    pub class: Class,
    pub health: i32,
    pub max_health: i32,
    pub stamina: i32,
    pub damage: i32,
    pub health_potions: i32,
    pub shield: i32,
    pub max_shield: i32,
    pub crit: f32,
    pub mana: i32,
    pub max_mana: i32,
    pub mana_potions: i32,
}

impl Fighter {
    /// A level-1 fighter with the starting stats from the C++ `newCharacter`.
    pub fn new(name: &str, class: Class) -> Fighter {
        let mut f = Fighter {
            name: name.to_string(),
            class,
            health: 100,
            max_health: 100,
            stamina: 100,
            damage: 10,
            health_potions: 2,
            shield: 0,
            max_shield: 0,
            crit: 0.0,
            mana: 0,
            max_mana: 0,
            mana_potions: 0,
        };
        match class {
            Class::Warrior => (f.shield, f.max_shield) = (SHIELD_PER_LEVEL, SHIELD_PER_LEVEL),
            Class::Assassin => f.crit = 1.5,
            Class::Sorcerer => (f.mana, f.max_mana, f.mana_potions) = (50, 50, 3),
        }
        f
    }

    pub fn alive(&self) -> bool {
        self.health > 0
    }

    /// Returns (absorbed by shield, health lost). A Warrior's shield soaks damage first.
    pub fn take_damage(&mut self, amount: i32) -> (i32, i32) {
        let amount = amount.max(0);
        let absorbed = if self.class == Class::Warrior { self.shield.min(amount) } else { 0 };
        self.shield -= absorbed;
        let lost = (amount - absorbed).min(self.health);
        self.health -= lost;
        (absorbed, lost)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Move {
    Attack,
    RaiseShield,
    CritAttack,
    FireOrb,
    HealthPotion,
    ManaPotion,
    Rest,
    Flee,
}

impl Move {
    /// The C++ battle menu text.
    pub fn label(self) -> &'static str {
        match self {
            Move::Attack => "Attack",
            Move::RaiseShield => "Raise shield",
            Move::CritAttack => "Crit Attack",
            Move::FireOrb => "Cast fire orb",
            Move::HealthPotion => "Drink health potion",
            Move::ManaPotion => "Drink mana potion",
            Move::Rest => "Rest",
            Move::Flee => "Flee",
        }
    }
}

/// Each class's battle menu, in C++ order.
pub fn moves(class: Class) -> &'static [Move] {
    use Move::*;
    match class {
        Class::Warrior => &[Attack, RaiseShield, HealthPotion, Rest, Flee],
        Class::Assassin => &[Attack, CritAttack, HealthPotion, Rest, Flee],
        Class::Sorcerer => &[FireOrb, HealthPotion, ManaPotion, Rest, Flee],
    }
}

/// `Err(reason)` when the move would be refused. Checks run in the same order as C++.
pub fn availability(f: &Fighter, m: Move) -> Result<(), String> {
    let need = |ok: bool, why: &str| if ok { Ok(()) } else { Err(why.to_string()) };
    match m {
        Move::Attack => need(f.stamina >= ATTACK_STAMINA, "needs 10 stamina"),
        Move::CritAttack => need(f.stamina >= CRIT_STAMINA, "needs 20 stamina"),
        Move::RaiseShield => {
            need(f.stamina >= SHIELD_STAMINA, "needs 15 stamina")?;
            need(f.shield < f.max_shield, "shield already full")
        }
        Move::FireOrb => {
            need(f.mana >= SPELL_MANA, "needs 10 mana")?;
            need(f.stamina >= ATTACK_STAMINA, "needs 10 stamina")
        }
        Move::HealthPotion => {
            need(f.health_potions > 0, "no potions left")?;
            need(f.health < f.max_health, "already at full health")
        }
        Move::ManaPotion => {
            need(f.mana_potions > 0, "no mana potions left")?;
            need(f.mana < f.max_mana, "mana already full")
        }
        Move::Rest | Move::Flee => Ok(()),
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Hit { target: usize, absorbed: i32, damage: i32, crit: bool },
    Healed { who: usize, hp: i32 },
    Mana { who: usize, mp: i32 },
    Shield { who: usize, amount: i32 },
    Rested { who: usize, hp: i32, stamina: i32, mana: i32 },
    Fled { who: usize },
    Defeated { who: usize },
}

#[derive(Clone, Debug)]
pub struct Resolution {
    pub actor: usize,
    pub mv: Move,
    pub events: Vec<Event>,
    pub narration: Vec<String>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum End {
    Won { winner: usize },
    Fled { who: usize },
}

#[derive(Clone)]
pub struct Duel {
    pub f: [Fighter; 2],
    pub turn: usize,
    pub end: Option<End>,
}

impl Duel {
    pub fn new(a: Fighter, b: Fighter) -> Duel {
        Duel { f: [a, b], turn: 0, end: None }
    }

    /// Play the active fighter's move. `None` (and no change) if the duel is over, the
    /// move is not on this class's menu, or it cannot be paid for.
    pub fn take_turn(&mut self, m: Move) -> Option<Resolution> {
        let (me, them) = (self.turn, 1 - self.turn);
        if self.end.is_some() || !moves(self.f[me].class).contains(&m) || availability(&self.f[me], m).is_err() {
            return None;
        }
        let mut events = Vec::new();
        let mut narration = Vec::new();
        let n = self.f[me].name.clone();
        let t = self.f[them].name.clone();

        let mut hit = |duel: &mut Duel, amount: i32, crit: bool, events: &mut Vec<Event>, narration: &mut Vec<String>| {
            let target = &mut duel.f[them];
            let (absorbed, damage) = target.take_damage(amount);
            events.push(Event::Hit { target: them, absorbed, damage, crit });
            if absorbed > 0 {
                narration.push(format!(
                    "{t}'s shield absorbs {absorbed} damage ({}/{} remaining).",
                    target.shield, target.max_shield
                ));
            }
        };

        match m {
            Move::Attack => {
                self.f[me].stamina -= ATTACK_STAMINA;
                narration.push(match self.f[me].class {
                    Class::Assassin => format!("{n} slips through the guard and lands a swift, clean strike!"),
                    _ => format!("{n} unleashes a devastating slash with their blade, cleaving through with raw power!"),
                });
                let dmg = self.f[me].damage;
                hit(self, dmg, false, &mut events, &mut narration);
            }
            Move::CritAttack => {
                self.f[me].stamina -= CRIT_STAMINA;
                let dmg = (self.f[me].damage as f32 * self.f[me].crit) as i32;
                narration.push(format!("{n} vanishes into the shadows and strikes a vital spot for {dmg} damage!"));
                hit(self, dmg, true, &mut events, &mut narration);
            }
            Move::FireOrb => {
                self.f[me].stamina -= ATTACK_STAMINA;
                self.f[me].mana -= SPELL_MANA;
                narration.push(format!("{n} conjures a fiery orb of energy and hurls it, exploding on impact!"));
                let dmg = self.f[me].damage + SPELL_BONUS;
                hit(self, dmg, false, &mut events, &mut narration);
            }
            Move::RaiseShield => {
                let f = &mut self.f[me];
                f.stamina -= SHIELD_STAMINA;
                let before = f.shield;
                f.shield = (f.shield + SHIELD_PER_LEVEL).min(f.max_shield);
                let x = f.shield - before;
                events.push(Event::Shield { who: me, amount: x });
                narration.push(format!(
                    "{n} braces their shield against the next blow. +{x} shield ({}/{}).",
                    f.shield, f.max_shield
                ));
            }
            Move::HealthPotion => {
                let f = &mut self.f[me];
                f.health_potions -= 1;
                let before = f.health;
                f.health = (f.health + POTION_HEALTH).min(f.max_health);
                let g = f.health - before;
                events.push(Event::Healed { who: me, hp: g });
                narration.push(format!(
                    "{n} drinks a healing potion, mending their wounds. +{g} health ({} left).",
                    f.health_potions
                ));
            }
            Move::ManaPotion => {
                let f = &mut self.f[me];
                f.mana_potions -= 1;
                let before = f.mana;
                f.mana = (f.mana + POTION_MANA).min(f.max_mana);
                let g = f.mana - before;
                events.push(Event::Mana { who: me, mp: g });
                narration.push(format!(
                    "{n} drains a shimmering mana potion, arcane energy coursing through them. +{g} mana ({} left).",
                    f.mana_potions
                ));
            }
            Move::Rest => {
                let f = &mut self.f[me];
                let (h0, m0) = (f.health, f.mana);
                f.health = (f.health + REST_HEALTH).min(f.max_health);
                f.stamina += REST_STAMINA;
                if f.class == Class::Sorcerer {
                    f.mana = (f.mana + REST_MANA).min(f.max_mana);
                }
                let (hp, mana) = (f.health - h0, f.mana - m0);
                events.push(Event::Rested { who: me, hp, stamina: REST_STAMINA, mana });
                narration.push(format!(
                    "{n} takes a moment to breathe, stepping back and regaining strength as the battle rages on."
                ));
                narration.push(format!("+{hp} health, +{REST_STAMINA} stamina."));
                if mana > 0 {
                    narration.push(format!("+{mana} mana."));
                }
            }
            Move::Flee => {
                events.push(Event::Fled { who: me });
                narration.push(match self.f[me].class {
                    Class::Warrior => format!("{n} breaks off and flees the fight!"),
                    Class::Assassin => format!("{n} melts into the shadows and flees the fight!"),
                    Class::Sorcerer => format!("{n} melts into a puff of smoke and flees the fight!"),
                });
                self.end = Some(End::Fled { who: me });
            }
        }

        if !self.f[them].alive() {
            events.push(Event::Defeated { who: them });
            narration.push(format!("{t} has been defeated! {n} wins!"));
            self.end = Some(End::Won { winner: me });
        }
        if self.end.is_none() {
            self.turn = them;
        }
        Some(Resolution { actor: me, mv: m, events, narration })
    }

    /// Same names and classes, fresh stats, player 1 to move.
    pub fn rematch(&mut self) {
        for f in &mut self.f {
            *f = Fighter::new(&f.name, f.class);
        }
        self.turn = 0;
        self.end = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Class::*;
    use Move::*;

    fn duel(a: Class, b: Class) -> Duel {
        Duel::new(Fighter::new("A", a), Fighter::new("B", b))
    }

    fn err(s: &str) -> Result<(), String> {
        Err(s.to_string())
    }

    #[test]
    fn new_fighter_stats() {
        for c in Class::ALL {
            let f = Fighter::new("X", c);
            assert_eq!((f.health, f.max_health, f.stamina, f.damage, f.health_potions), (100, 100, 100, 10, 2));
            assert_eq!(f.class, c);
            assert_eq!(f.name, "X");
        }
        let w = Fighter::new("W", Warrior);
        assert_eq!((w.shield, w.max_shield), (25, 25));
        let a = Fighter::new("A", Assassin);
        assert_eq!(a.crit, 1.5);
        let s = Fighter::new("S", Sorcerer);
        assert_eq!((s.mana, s.max_mana, s.mana_potions), (50, 50, 3));
        assert_eq!(Warrior.name(), "Warrior");
        assert_eq!(Sorcerer.blurb(), "spends mana on fire orbs that hit harder than steel");
    }

    #[test]
    fn menus_follow_cpp_order() {
        assert_eq!(moves(Warrior), &[Attack, RaiseShield, HealthPotion, Rest, Flee]);
        assert_eq!(moves(Assassin), &[Attack, CritAttack, HealthPotion, Rest, Flee]);
        assert_eq!(moves(Sorcerer), &[FireOrb, HealthPotion, ManaPotion, Rest, Flee]);
        assert_eq!(FireOrb.label(), "Cast fire orb");
        assert_eq!(HealthPotion.label(), "Drink health potion");
    }

    #[test]
    fn warrior_attack_deals_10() {
        let mut d = duel(Warrior, Sorcerer);
        let r = d.take_turn(Attack).unwrap();
        assert_eq!(d.f[1].health, 90);
        assert_eq!(d.f[0].stamina, 90);
        assert_eq!(d.turn, 1);
        assert_eq!(r.actor, 0);
        assert_eq!(r.events, vec![Event::Hit { target: 1, absorbed: 0, damage: 10, crit: false }]);
        assert_eq!(r.narration[0], "A unleashes a devastating slash with their blade, cleaving through with raw power!");
    }

    #[test]
    fn shield_absorbs_first() {
        let mut d = duel(Assassin, Warrior);
        let r = d.take_turn(CritAttack).unwrap();
        assert_eq!(r.events[0], Event::Hit { target: 1, absorbed: 15, damage: 0, crit: true });
        assert_eq!((d.f[1].shield, d.f[1].health), (10, 100));
        assert_eq!(r.narration[1], "B's shield absorbs 15 damage (10/25 remaining).");
        d.take_turn(Rest).unwrap();
        let r = d.take_turn(Attack).unwrap();
        assert_eq!(r.events[0], Event::Hit { target: 1, absorbed: 10, damage: 0, crit: false });
        assert_eq!(d.f[1].shield, 0);
        d.take_turn(Rest).unwrap();
        let r = d.take_turn(CritAttack).unwrap();
        assert_eq!(r.events[0], Event::Hit { target: 1, absorbed: 0, damage: 15, crit: true });
        assert_eq!(d.f[1].health, 85);
        assert_eq!(r.narration.len(), 1, "no absorb line when the shield is empty");
    }

    #[test]
    fn crit_costs_20_deals_15() {
        let mut d = duel(Assassin, Sorcerer);
        let r = d.take_turn(CritAttack).unwrap();
        assert_eq!(d.f[0].stamina, 80);
        assert_eq!(d.f[1].health, 85);
        assert_eq!(r.narration[0], "A vanishes into the shadows and strikes a vital spot for 15 damage!");
        d.f[0].stamina = 15;
        d.turn = 0;
        assert_eq!(availability(&d.f[0], CritAttack), err("needs 20 stamina"));
    }

    #[test]
    fn fire_orb_costs_mana_and_stamina() {
        let mut d = duel(Sorcerer, Assassin);
        let r = d.take_turn(FireOrb).unwrap();
        assert_eq!((d.f[0].mana, d.f[0].stamina, d.f[1].health), (40, 90, 86));
        assert_eq!(r.narration[0], "A conjures a fiery orb of energy and hurls it, exploding on impact!");
        d.take_turn(Rest).unwrap();
        d.f[0].mana = 5;
        d.f[0].stamina = 5;
        assert_eq!(availability(&d.f[0], FireOrb), err("needs 10 mana"), "mana is checked first");
        let before = (d.f[0].clone(), d.f[1].clone(), d.turn);
        assert!(d.take_turn(FireOrb).is_none());
        assert_eq!((d.f[0].mana, d.f[0].stamina, d.f[1].health, d.turn), (before.0.mana, before.0.stamina, before.1.health, before.2));
        d.f[0].mana = 50;
        assert_eq!(availability(&d.f[0], FireOrb), err("needs 10 stamina"));
    }

    #[test]
    fn raise_shield_rules() {
        let mut w = Fighter::new("W", Warrior);
        assert_eq!(availability(&w, RaiseShield), err("shield already full"));
        assert_eq!(w.take_damage(30), (25, 5));
        assert_eq!((w.shield, w.health), (0, 95));
        let mut d = Duel::new(w, Fighter::new("B", Assassin));
        let r = d.take_turn(RaiseShield).unwrap();
        assert_eq!((d.f[0].shield, d.f[0].stamina), (25, 85));
        assert_eq!(r.events, vec![Event::Shield { who: 0, amount: 25 }]);
        assert_eq!(r.narration[0], "W braces their shield against the next blow. +25 shield (25/25).");
        let mut w = Fighter::new("W", Warrior);
        w.stamina = 10;
        assert_eq!(availability(&w, RaiseShield), err("needs 15 stamina"), "stamina is checked before fullness");
        w.shield = 0;
        assert_eq!(availability(&w, RaiseShield), err("needs 15 stamina"));
    }

    #[test]
    fn health_potion_rules() {
        let mut d = duel(Assassin, Sorcerer);
        assert_eq!(availability(&d.f[0], HealthPotion), err("already at full health"));
        d.f[0].health = 30;
        let r = d.take_turn(HealthPotion).unwrap();
        assert_eq!((d.f[0].health, d.f[0].health_potions), (80, 1));
        assert_eq!(r.events, vec![Event::Healed { who: 0, hp: 50 }]);
        d.take_turn(Rest).unwrap();
        d.f[0].health = 90;
        let r = d.take_turn(HealthPotion).unwrap();
        assert_eq!(d.f[0].health, 100);
        assert_eq!(r.narration[0], "A drinks a healing potion, mending their wounds. +10 health (0 left).");
        d.f[0].health = 50;
        assert_eq!(availability(&d.f[0], HealthPotion), err("no potions left"));
        d.f[0].health = 100;
        assert_eq!(availability(&d.f[0], HealthPotion), err("no potions left"), "count is checked before fullness");
    }

    #[test]
    fn mana_potion_rules() {
        let mut d = duel(Sorcerer, Warrior);
        assert_eq!(availability(&d.f[0], ManaPotion), err("mana already full"));
        d.f[0].mana = 20;
        let r = d.take_turn(ManaPotion).unwrap();
        assert_eq!((d.f[0].mana, d.f[0].mana_potions), (50, 2));
        assert_eq!(r.events, vec![Event::Mana { who: 0, mp: 30 }]);
        assert_eq!(r.narration[0], "A drains a shimmering mana potion, arcane energy coursing through them. +30 mana (2 left).");
        d.f[0].mana_potions = 0;
        d.f[0].mana = 0;
        assert_eq!(availability(&d.f[0], ManaPotion), err("no mana potions left"));
    }

    #[test]
    fn rest_rules() {
        let mut d = duel(Warrior, Sorcerer);
        d.f[0].health = 95;
        let r = d.take_turn(Rest).unwrap();
        assert_eq!((d.f[0].health, d.f[0].stamina), (100, 125));
        assert_eq!(r.events, vec![Event::Rested { who: 0, hp: 5, stamina: 25, mana: 0 }]);
        assert_eq!(
            r.narration,
            vec![
                "A takes a moment to breathe, stepping back and regaining strength as the battle rages on.".to_string(),
                "+5 health, +25 stamina.".to_string(),
            ]
        );
        d.f[1].mana = 45;
        let r = d.take_turn(Rest).unwrap();
        assert_eq!(d.f[1].mana, 50);
        assert_eq!(r.narration[2], "+5 mana.");
        d.take_turn(Rest).unwrap();
        let r = d.take_turn(Rest).unwrap();
        assert_eq!(r.narration.len(), 2, "no mana line at full mana");
    }

    #[test]
    fn rest_and_flee_always_available() {
        for c in Class::ALL {
            let mut f = Fighter::new("X", c);
            f.stamina = 0;
            f.mana = 0;
            f.health_potions = 0;
            f.mana_potions = 0;
            f.health = 40;
            f.shield = 0;
            for &m in moves(c) {
                match m {
                    Rest | Flee => assert_eq!(availability(&f, m), Ok(())),
                    _ => assert!(availability(&f, m).is_err(), "{m:?} should be dimmed for {c:?}"),
                }
            }
        }
    }

    #[test]
    fn flee_ends_duel() {
        let mut d = duel(Sorcerer, Assassin);
        let r = d.take_turn(Flee).unwrap();
        assert_eq!(d.end, Some(End::Fled { who: 0 }));
        assert_eq!(r.events, vec![Event::Fled { who: 0 }]);
        assert_eq!(r.narration[0], "A melts into a puff of smoke and flees the fight!");
        assert!(d.take_turn(Rest).is_none());
    }

    #[test]
    fn knockout() {
        let mut d = duel(Warrior, Assassin);
        d.f[1].health = 10;
        let r = d.take_turn(Attack).unwrap();
        assert_eq!(d.f[1].health, 0);
        assert!(!d.f[1].alive());
        assert_eq!(d.end, Some(End::Won { winner: 0 }));
        assert_eq!(r.events.last(), Some(&Event::Defeated { who: 1 }));
        assert_eq!(r.narration.last().unwrap(), "B has been defeated! A wins!");
        assert_eq!(d.turn, 0);
    }

    #[test]
    fn move_not_in_class_is_rejected() {
        let mut d = duel(Warrior, Sorcerer);
        assert!(d.take_turn(FireOrb).is_none());
        assert!(d.take_turn(CritAttack).is_none());
        assert_eq!(d.turn, 0);
    }

    #[test]
    fn rematch_resets() {
        let mut d = duel(Warrior, Sorcerer);
        d.take_turn(Attack).unwrap();
        d.take_turn(FireOrb).unwrap();
        d.take_turn(Flee).unwrap();
        d.rematch();
        assert_eq!((d.turn, d.end), (0, None));
        assert_eq!((d.f[0].name.as_str(), d.f[0].class, d.f[0].shield, d.f[0].stamina), ("A", Warrior, 25, 100));
        assert_eq!((d.f[1].name.as_str(), d.f[1].class, d.f[1].health, d.f[1].mana), ("B", Sorcerer, 100, 50));
    }
}
