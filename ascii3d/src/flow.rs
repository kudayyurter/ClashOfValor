//! The duel's screen flow as a pure state machine: setup, choosing a move, the result.
//!
//! `main.rs` turns each frame's key presses into a list of `Input`s and calls `step`.
//! At most one screen change happens per frame: once an input causes a transition, the
//! rest of that frame's inputs are dropped, so a single Enter can never both accept a
//! name and confirm the next menu.

use crate::rules::{Class, Duel, End, Fighter, Resolution, availability, moves};
use crate::ui::{Key, Menu, NameField};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Input {
    Up,
    Down,
    Enter,
    Space,
    Backspace,
    Esc,
    Digit(u8),
    /// Printable text, only used while typing a name.
    Char(char),
}

pub enum Screen {
    Setup { who: usize, naming: bool, field: NameField, menu: Menu, names: [String; 2], classes: [Class; 2] },
    Choose { menu: Menu },
    Result { menu: Menu },
}

pub struct Flow {
    pub screen: Screen,
    pub duel: Option<Duel>,
    /// The numbers on screen (lag the duel until a move's impact).
    pub shown: Option<[Fighter; 2]>,
    pub caption: Vec<String>,
    /// The last move played, for effects.
    pub last: Option<Resolution>,
    pub quit: bool,
}

impl Flow {
    /// Start at the setup screen.
    pub fn new() -> Flow {
        Flow { screen: setup_screen(), duel: None, shown: None, caption: Vec::new(), last: None, quit: false }
    }

    /// Skip setup: straight into a duel.
    pub fn with_duel(names: [&str; 2], classes: [Class; 2]) -> Flow {
        let mut f = Flow::new();
        f.start_duel(Duel::new(Fighter::new(names[0], classes[0]), Fighter::new(names[1], classes[1])));
        f
    }

    fn start_duel(&mut self, duel: Duel) {
        self.shown = Some(duel.f.clone());
        self.duel = Some(duel);
        self.caption.clear();
        self.last = None;
        self.screen = Screen::Choose { menu: self.move_menu() };
    }

    /// The active fighter's moves, unaffordable ones dimmed with the reason.
    fn move_menu(&self) -> Menu {
        let d = self.duel.as_ref().expect("duel");
        let f = &d.f[d.turn];
        let ms = moves(f.class);
        let checks: Vec<Result<(), String>> = ms.iter().map(|&m| availability(f, m)).collect();
        Menu::new(
            ms.iter().map(|m| m.label().to_string()).collect(),
            checks.iter().map(|c| c.clone().err().unwrap_or_default()).collect(),
            checks.iter().map(|c| c.is_ok()).collect(),
        )
    }

    /// A coarse id of the current screen; a change means a transition happened.
    fn stage(&self) -> (u8, usize, bool) {
        match &self.screen {
            Screen::Setup { who, naming, .. } => (0, *who, *naming),
            Screen::Choose { .. } => (1, self.active(), false),
            Screen::Result { .. } => (2, 0, false),
        }
    }

    /// Apply one frame of input. Stops at the first input that changes the screen.
    pub fn step(&mut self, inputs: &[Input]) {
        for &i in inputs {
            if i == Input::Esc {
                self.quit = true;
            }
            if self.quit {
                return;
            }
            let before = self.stage();
            self.apply(i);
            if self.stage() != before || self.quit {
                return;
            }
        }
    }

    fn apply(&mut self, i: Input) {
        match &mut self.screen {
            Screen::Setup { who, naming, field, menu, names, classes } => {
                if *naming {
                    match i {
                        Input::Char(c) => field.push(c),
                        Input::Backspace => field.backspace(),
                        Input::Enter => {
                            if let Some(name) = field.accept() {
                                names[*who] = name;
                                *naming = false;
                                *menu = class_menu();
                            }
                        }
                        _ => {}
                    }
                    return;
                }
                let Some(k) = menu_key(i) else { return };
                let Some(pick) = menu.handle(k) else { return };
                classes[*who] = Class::ALL[pick];
                if *who == 0 {
                    *who = 1;
                    *naming = true;
                    *field = NameField::default();
                } else {
                    let duel = Duel::new(Fighter::new(&names[0], classes[0]), Fighter::new(&names[1], classes[1]));
                    self.start_duel(duel);
                }
            }
            Screen::Choose { menu } => {
                let Some(k) = menu_key(i) else { return };
                let Some(pick) = menu.handle(k) else { return };
                let duel = self.duel.as_mut().expect("duel");
                let mv = moves(duel.f[duel.turn].class)[pick];
                let Some(res) = duel.take_turn(mv) else { return };
                self.shown = Some(duel.f.clone());
                self.caption = res.narration.clone();
                self.last = Some(res);
                self.screen = if self.duel.as_ref().is_some_and(|d| d.end.is_some()) {
                    Screen::Result { menu: Menu::plain(&["Rematch", "New duel", "Quit"]) }
                } else {
                    Screen::Choose { menu: self.move_menu() }
                };
            }
            Screen::Result { menu } => {
                let Some(k) = menu_key(i) else { return };
                match menu.handle(k) {
                    Some(0) => {
                        let mut d = self.duel.take().expect("duel");
                        d.rematch();
                        self.start_duel(d);
                    }
                    Some(1) => *self = Flow::new(),
                    Some(_) => self.quit = true,
                    None => {}
                }
            }
        }
    }

    /// Classes and dim amounts to show at each fighter's spot.
    pub fn preview(&self) -> ([Class; 2], [f32; 2]) {
        match &self.screen {
            Screen::Setup { who, naming, menu, classes, .. } => {
                let mut c = [Class::Warrior, Class::Warrior];
                for (i, slot) in c.iter_mut().enumerate() {
                    if i < *who {
                        *slot = classes[i];
                    } else if i == *who && !*naming {
                        *slot = Class::ALL[menu.sel];
                    }
                }
                (c, [0.0, if *who == 0 { 0.6 } else { 0.0 }])
            }
            _ => {
                let d = self.duel.as_ref().expect("duel");
                ([d.f[0].class, d.f[1].class], [0.0, 0.0])
            }
        }
    }

    /// The active fighter while a duel runs.
    pub fn active(&self) -> usize {
        self.duel.as_ref().map_or(0, |d| d.turn)
    }

    /// True while a name is being typed: letter hotkeys must not fire.
    pub fn typing(&self) -> bool {
        matches!(self.screen, Screen::Setup { naming: true, .. })
    }

    /// Names to show over each fighter.
    pub fn names(&self) -> [String; 2] {
        let fallback = |i: usize| format!("Player {}", i + 1);
        match (&self.screen, &self.duel) {
            (Screen::Setup { names, .. }, _) => {
                [0, 1].map(|i| if names[i].is_empty() { fallback(i) } else { names[i].clone() })
            }
            (_, Some(d)) => [d.f[0].name.clone(), d.f[1].name.clone()],
            _ => [fallback(0), fallback(1)],
        }
    }

    /// Result banner text and the player whose accent colours it.
    pub fn banner(&self) -> Option<(String, usize)> {
        if !matches!(self.screen, Screen::Result { .. }) {
            return None;
        }
        let d = self.duel.as_ref()?;
        match d.end? {
            End::Won { winner } => Some((format!("{} wins!", d.f[winner].name), winner)),
            End::Fled { who } => Some((format!("{} fled the fight!", d.f[who].name), who)),
        }
    }
}

fn setup_screen() -> Screen {
    Screen::Setup {
        who: 0,
        naming: true,
        field: NameField::default(),
        menu: class_menu(),
        names: [String::new(), String::new()],
        classes: [Class::Warrior, Class::Warrior],
    }
}

fn class_menu() -> Menu {
    Menu::new(
        Class::ALL.iter().map(|c| c.name().to_string()).collect(),
        Class::ALL.iter().map(|c| c.blurb().to_string()).collect(),
        vec![true; 3],
    )
}

fn menu_key(i: Input) -> Option<Key> {
    match i {
        Input::Up => Some(Key::Up),
        Input::Down => Some(Key::Down),
        Input::Enter => Some(Key::Enter),
        Input::Digit(d) => Some(Key::Digit(d)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::Move;
    use Input::*;

    fn typed(s: &str) -> Vec<Input> {
        s.chars().map(Char).collect()
    }

    /// Feed each input as its own frame.
    fn frames(f: &mut Flow, inputs: &[Input]) {
        for i in inputs {
            f.step(std::slice::from_ref(i));
        }
    }

    fn stage(f: &Flow) -> String {
        match &f.screen {
            Screen::Setup { who, naming, .. } => format!("setup {who} {}", if *naming { "name" } else { "class" }),
            Screen::Choose { .. } => format!("choose {}", f.active()),
            Screen::Result { .. } => "result".into(),
        }
    }

    #[test]
    fn setup_full_flow() {
        let mut f = Flow::new();
        assert_eq!(stage(&f), "setup 0 name");
        frames(&mut f, &typed("Ann"));
        frames(&mut f, &[Enter]);
        assert_eq!(stage(&f), "setup 0 class");
        frames(&mut f, &[Digit(1)]);
        assert_eq!(stage(&f), "setup 1 name");
        frames(&mut f, &typed("Bo"));
        frames(&mut f, &[Enter, Down, Down, Enter]);
        assert_eq!(stage(&f), "choose 0");
        let d = f.duel.as_ref().unwrap();
        assert_eq!((d.f[0].name.as_str(), d.f[0].class), ("Ann", Class::Warrior));
        assert_eq!((d.f[1].name.as_str(), d.f[1].class), ("Bo", Class::Sorcerer));
        assert_eq!(f.shown.as_ref().unwrap()[1].mana, 50);
    }

    #[test]
    fn typing_and_names() {
        let mut f = Flow::new();
        assert!(f.typing());
        assert_eq!(f.names(), ["Player 1".to_string(), "Player 2".to_string()]);
        frames(&mut f, &typed("Ann"));
        frames(&mut f, &[Enter]);
        assert!(!f.typing());
        assert_eq!(f.names()[0], "Ann");
    }

    #[test]
    fn empty_name_is_not_accepted() {
        let mut f = Flow::new();
        frames(&mut f, &[Char(' '), Enter]);
        assert_eq!(stage(&f), "setup 0 name");
    }

    #[test]
    fn enter_accepting_name_does_not_confirm_class() {
        let mut f = Flow::new();
        frames(&mut f, &typed("Ann"));
        f.step(&[Enter, Enter]);
        assert_eq!(stage(&f), "setup 0 class");
    }

    #[test]
    fn digit_picking_class_does_not_land_in_next_name() {
        let mut f = Flow::new();
        frames(&mut f, &typed("Ann"));
        frames(&mut f, &[Enter]);
        // A digit key press arrives as both Digit and Char in the same frame.
        f.step(&[Digit(2), Char('2')]);
        assert_eq!(stage(&f), "setup 1 name");
        match &f.screen {
            Screen::Setup { field, classes, .. } => {
                assert_eq!(field.text, "");
                assert_eq!(classes[0], Class::Assassin);
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn digits_type_into_names() {
        let mut f = Flow::new();
        f.step(&[Digit(7), Char('7')]);
        match &f.screen {
            Screen::Setup { field, naming: true, .. } => assert_eq!(field.text, "7"),
            _ => panic!("still naming"),
        }
    }

    #[test]
    fn class_preview_follows_highlight() {
        let mut f = Flow::new();
        assert_eq!(f.preview(), ([Class::Warrior, Class::Warrior], [0.0, 0.6]));
        frames(&mut f, &typed("Ann"));
        frames(&mut f, &[Enter, Down]);
        assert_eq!(f.preview().0[0], Class::Assassin);
        frames(&mut f, &[Down, Enter]);
        frames(&mut f, &typed("Bo"));
        frames(&mut f, &[Enter, Down]);
        assert_eq!(f.preview(), ([Class::Sorcerer, Class::Assassin], [0.0, 0.0]));
    }

    #[test]
    fn choosing_a_move_plays_it_and_passes_the_turn() {
        let mut f = Flow::with_duel(["A", "B"], [Class::Warrior, Class::Sorcerer]);
        frames(&mut f, &[Digit(1)]);
        assert_eq!(stage(&f), "choose 1");
        assert_eq!(f.shown.as_ref().unwrap()[1].health, 90);
        assert_eq!(f.caption[0], "A unleashes a devastating slash with their blade, cleaving through with raw power!");
        assert_eq!(f.last.as_ref().unwrap().mv, Move::Attack);
    }

    #[test]
    fn dimmed_move_cannot_be_picked() {
        let mut f = Flow::with_duel(["A", "B"], [Class::Warrior, Class::Sorcerer]);
        // Raise shield (2) is dimmed at full shield.
        frames(&mut f, &[Digit(2)]);
        assert_eq!(stage(&f), "choose 0");
        match &f.screen {
            Screen::Choose { menu } => {
                assert!(!menu.enabled[1]);
                assert_eq!(menu.notes[1], "shield already full");
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn knockout_and_rematch() {
        let mut f = Flow::with_duel(["A", "B"], [Class::Warrior, Class::Assassin]);
        f.duel.as_mut().unwrap().f[1].health = 10;
        frames(&mut f, &[Digit(1)]);
        assert_eq!(stage(&f), "result");
        assert_eq!(f.banner(), Some(("A wins!".to_string(), 0)));
        frames(&mut f, &[Digit(1)]);
        assert_eq!(stage(&f), "choose 0");
        assert_eq!(f.shown.as_ref().unwrap()[1].health, 100);
    }

    #[test]
    fn flee_then_new_duel() {
        let mut f = Flow::with_duel(["A", "B"], [Class::Sorcerer, Class::Assassin]);
        frames(&mut f, &[Digit(5)]);
        assert_eq!(f.banner(), Some(("A fled the fight!".to_string(), 0)));
        frames(&mut f, &[Down, Enter]);
        assert_eq!(stage(&f), "setup 0 name");
        assert!(f.duel.is_none());
    }

    #[test]
    fn esc_quits_anywhere() {
        let mut f = Flow::new();
        f.step(&[Esc]);
        assert!(f.quit);
        let mut g = Flow::with_duel(["A", "B"], [Class::Sorcerer, Class::Assassin]);
        g.step(&[Esc]);
        assert!(g.quit);
    }

    #[test]
    fn result_quit_item() {
        let mut f = Flow::with_duel(["A", "B"], [Class::Sorcerer, Class::Assassin]);
        frames(&mut f, &[Digit(5), Digit(3)]);
        assert!(f.quit);
    }

    #[test]
    fn flee_records_end() {
        let mut f = Flow::with_duel(["A", "B"], [Class::Sorcerer, Class::Assassin]);
        frames(&mut f, &[Digit(5)]);
        assert_eq!(f.duel.as_ref().unwrap().end, Some(End::Fled { who: 0 }));
    }
}
