//! The duel's screen flow as a pure state machine: setup, choosing a move, the result,
//! plus pop-ups (pause, how to play, flee confirm) drawn over any screen.
//!
//! `main.rs` turns each frame's key presses into a list of `Input`s and calls `step`.
//! At most one screen change happens per frame: once an input causes a transition, the
//! rest of that frame's inputs are dropped, so a single Enter can never both accept a
//! name and confirm the next menu. No single key quits the game: only the Quit items do.

use crate::anim::{self, Action};
use crate::bot::{self, Rng};
use crate::models::Pose;
use crate::rules::{Class, Duel, End, Event, Fighter, Move, Resolution, TURN_LIMIT, availability, log_line, moves};
use crate::ui::{Key, Menu, NameField};

/// Seconds a newly shown menu ignores input.
const MENU_GRACE: f32 = 0.25;
/// Move-log length.
const LOG_LEN: usize = 4;
pub const BOT_NAME: &str = "Bot";

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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Pvp,
    Bot,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SetupStage {
    Mode,
    Name,
    Class,
}

pub enum Screen {
    Setup { stage: SetupStage, who: usize, field: NameField, menu: Menu, names: [String; 2], classes: [Class; 2] },
    Choose { menu: Menu },
    /// A move playing out; `collapse` runs 0..1 after a knockout.
    Animate { action: Action, collapse: f32 },
    Result { menu: Menu },
}

pub enum Popup {
    Pause { menu: Menu },
    HowTo,
    ConfirmFlee { menu: Menu },
}

pub struct Flow {
    pub screen: Screen,
    pub popup: Option<Popup>,
    pub mode: Mode,
    pub duel: Option<Duel>,
    /// The numbers on screen (lag the duel until a move's impact).
    pub shown: Option<[Fighter; 2]>,
    pub caption: Vec<String>,
    /// The last move played, for effects.
    pub last: Option<Resolution>,
    /// Set when a move's impact happens; read (and cleared) once per frame by `take_impact`.
    impact_pending: bool,
    /// Seconds since the last impact (drives the target's flinch).
    pub since_impact: f32,
    /// A menu that just appeared ignores input this long, so mashing Enter to skip an
    /// animation can't also pick the next player's move.
    menu_wait: f32,
    /// Fast animations: moves play at double speed.
    pub fast: bool,
    /// Wins per player across rematches; reset by New duel.
    pub score: [u32; 2],
    /// The last few moves, newest last.
    pub log: Vec<String>,
    bot_wait: f32,
    pub rng: Rng,
    pub quit: bool,
}

impl Flow {
    /// Start at the setup screen.
    pub fn new() -> Flow {
        Flow {
            screen: setup_screen(SetupStage::Mode, [String::new(), String::new()]),
            popup: None,
            mode: Mode::Pvp,
            duel: None,
            shown: None,
            caption: Vec::new(),
            last: None,
            impact_pending: false,
            since_impact: 10.0,
            menu_wait: 0.0,
            fast: false,
            score: [0, 0],
            log: Vec::new(),
            bot_wait: 0.0,
            rng: Rng::new(1),
            quit: false,
        }
    }

    /// Skip setup: straight into a player-vs-player duel.
    pub fn with_duel(names: [&str; 2], classes: [Class; 2]) -> Flow {
        let mut f = Flow::new();
        f.start_duel(Duel::new(Fighter::new(names[0], classes[0]), Fighter::new(names[1], classes[1])));
        f
    }

    fn start_duel(&mut self, duel: Duel) {
        self.shown = Some(duel.f.clone());
        self.duel = Some(duel);
        self.caption.clear();
        self.log.clear();
        self.last = None;
        self.bot_wait = 0.0;
        self.screen = Screen::Choose { menu: self.move_menu() };
    }

    /// Back to setup for a new duel: same mode, names pre-filled, score reset.
    fn new_duel(&mut self) {
        let names = self.duel.as_ref().map_or([String::new(), String::new()], |d| [d.f[0].name.clone(), d.f[1].name.clone()]);
        let names = [names[0].clone(), if self.mode == Mode::Bot { String::new() } else { names[1].clone() }];
        let (mode, fast, rng) = (self.mode, self.fast, std::mem::replace(&mut self.rng, Rng::new(1)));
        *self = Flow::new();
        self.mode = mode;
        self.fast = fast;
        self.rng = rng;
        self.screen = setup_screen(SetupStage::Mode, names);
    }

    /// The active fighter's moves, unaffordable ones dimmed with the reason.
    pub fn move_menu(&self) -> Menu {
        let d = self.duel.as_ref().expect("duel");
        let f = &d.f[d.turn];
        let ms = moves(f.class);
        let checks: Vec<Result<(), String>> = ms.iter().map(|&m| availability(f, m)).collect();
        let mut menu = Menu::new(
            ms.iter().map(|m| m.label().to_string()).collect(),
            ms.iter().map(|&m| crate::ui::move_cost(f, m)).collect(),
            checks.iter().map(|c| c.is_ok()).collect(),
        );
        menu.reasons = checks.iter().map(|c| c.clone().err().unwrap_or_default()).collect();
        menu
    }

    fn bot_turn(&self) -> bool {
        self.mode == Mode::Bot && self.duel.as_ref().is_some_and(|d| d.turn == 1 && d.end.is_none())
    }

    /// A coarse id of the current screen; a change means a transition happened.
    fn stage(&self) -> (u8, usize, u8, u8) {
        let popup = match &self.popup {
            None => 0,
            Some(Popup::Pause { .. }) => 1,
            Some(Popup::HowTo) => 2,
            Some(Popup::ConfirmFlee { .. }) => 3,
        };
        let s = match &self.screen {
            Screen::Setup { stage, who, .. } => (0, *who, *stage as u8),
            Screen::Choose { .. } => (1, self.active(), 0),
            Screen::Result { .. } => (2, 0, 0),
            Screen::Animate { action, .. } => (3, action.done() as usize, 0),
        };
        (s.0, s.1, s.2, popup)
    }

    /// True once after each impact (effects fire on it).
    pub fn take_impact(&mut self) -> bool {
        std::mem::take(&mut self.impact_pending)
    }

    fn on_impact(&mut self) {
        self.shown = self.duel.as_ref().map(|d| d.f.clone());
        if let Some(res) = &self.last {
            self.caption = res.narration.clone();
        }
        self.impact_pending = true;
        self.since_impact = 0.0;
    }

    /// Advance time by `dt` seconds. Nothing moves while a pop-up is open.
    pub fn update(&mut self, dt: f32) {
        if self.popup.is_some() {
            return;
        }
        self.since_impact += dt;
        self.menu_wait = (self.menu_wait - dt).max(0.0);
        if let Screen::Choose { menu } | Screen::Result { menu } = &mut self.screen {
            menu.tick(dt);
        }
        if matches!(self.screen, Screen::Choose { .. }) && self.bot_turn() {
            self.bot_wait += dt;
            if self.bot_wait >= bot::THINK {
                self.bot_wait = 0.0;
                let f = self.duel.as_ref().expect("duel").f[1].clone();
                let mv = bot::choose(&f, &mut self.rng);
                self.play(mv);
            }
            return;
        }
        let fast = self.fast;
        let Screen::Animate { action, collapse } = &mut self.screen else { return };
        let dt = if fast { dt * 2.0 } else { dt };
        if !action.done() {
            if action.advance(dt) {
                self.on_impact();
            }
            return;
        }
        match self.duel.as_ref().and_then(|d| d.end) {
            Some(End::Won { .. }) if *collapse < 1.0 => *collapse = (*collapse + dt).min(1.0),
            Some(end) => {
                if let End::Won { winner } | End::TimeUp { winner: Some(winner) } = end {
                    self.score[winner] += 1;
                }
                self.screen = Screen::Result { menu: Menu::plain(&["Rematch", "New duel", "Quit"]) };
                self.menu_wait = MENU_GRACE;
            }
            None => {
                self.screen = Screen::Choose { menu: self.move_menu() };
                self.menu_wait = MENU_GRACE;
                self.bot_wait = 0.0;
            }
        }
    }

    /// Play a move for the active fighter (human pick or bot).
    fn play(&mut self, mv: Move) {
        let duel = self.duel.as_mut().expect("duel");
        let Some(res) = duel.take_turn(mv) else { return };
        self.log.push(log_line(&res, duel));
        if self.log.len() > LOG_LEN {
            self.log.remove(0);
        }
        // `shown` and the result lines wait for the impact.
        self.caption = res.narration[..1].to_vec();
        self.last = Some(res.clone());
        self.screen = Screen::Animate { action: Action::new(res), collapse: 0.0 };
    }

    /// Poses and dim amounts for both fighters right now.
    pub fn body_state(&self, time: f32, gap: f32) -> ([Pose; 2], [f32; 2]) {
        let mut poses = [anim::idle_pose(time, 0), anim::idle_pose(time, 1)];
        let mut dims = self.preview().1;
        // The last target keeps flinching briefly, even after the menu comes back.
        if let Some(res) = &self.last {
            for e in &res.events {
                if let Event::Hit { target, .. } = e {
                    poses[*target] = anim::add(poses[*target], anim::flinch_pose(self.since_impact));
                }
            }
        }
        let end = self.duel.as_ref().and_then(|d| d.end);
        match &self.screen {
            Screen::Animate { action, collapse } => {
                let (a, p) = (action.res.actor, action.phase());
                let class = self.duel.as_ref().map_or(Class::Warrior, |d| d.f[a].class);
                poses[a] = anim::add(poses[a], anim::actor_pose(class, action.res.mv, p, gap));
                dims[a] = anim::actor_dim(action.res.mv, p);
                if let Some(End::Won { winner }) = end {
                    poses[1 - winner] = anim::add(poses[1 - winner], anim::collapse_pose(*collapse));
                }
            }
            Screen::Result { .. } => match end {
                Some(End::Won { winner }) => poses[1 - winner] = anim::collapse_pose(1.0),
                Some(End::Fled { who }) => dims[who] = 1.0,
                Some(End::TimeUp { .. }) | None => {}
            },
            _ => {}
        }
        (poses, dims)
    }

    /// Apply one frame of input. Stops at the first input that changes the screen.
    pub fn step(&mut self, inputs: &[Input]) {
        for &i in inputs {
            if self.quit {
                return;
            }
            let before = self.stage();
            if self.popup.is_some() {
                self.apply_popup(i);
            } else if i == Input::Esc {
                self.escape();
            } else {
                self.apply(i);
            }
            if self.stage() != before || self.quit {
                return;
            }
        }
    }

    /// Esc with no pop-up open: back one setup step, or pause.
    fn escape(&mut self) {
        let mode = self.mode;
        let Screen::Setup { stage, who, field, menu, names, classes } = &mut self.screen else {
            self.popup = Some(pause_popup(false, self.fast));
            return;
        };
        match (*stage, *who) {
            (SetupStage::Mode, _) => self.popup = Some(pause_popup(true, self.fast)),
            (SetupStage::Name, 0) => {
                *stage = SetupStage::Mode;
                *menu = mode_menu(mode);
            }
            (SetupStage::Class, 0) => {
                *stage = SetupStage::Name;
                *field = NameField { text: names[0].clone() };
            }
            (SetupStage::Name, _) => {
                *who = 0;
                *stage = SetupStage::Class;
                *menu = class_menu_at(classes[0]);
            }
            (SetupStage::Class, _) => {
                if mode == Mode::Bot {
                    *who = 0;
                    *menu = class_menu_at(classes[0]);
                } else {
                    *stage = SetupStage::Name;
                    *field = NameField { text: names[1].clone() };
                }
            }
        }
    }

    fn apply_popup(&mut self, i: Input) {
        let in_setup = matches!(self.screen, Screen::Setup { .. });
        match self.popup.as_mut().expect("popup") {
            Popup::HowTo => self.popup = Some(pause_popup(false, self.fast)),
            Popup::ConfirmFlee { menu } => {
                if i == Input::Esc {
                    self.popup = None;
                    return;
                }
                let Some(k) = menu_key(i) else { return };
                match menu.handle(k) {
                    Some(1) => {
                        self.popup = None;
                        self.play(Move::Flee);
                    }
                    Some(_) => self.popup = None,
                    None => {}
                }
            }
            Popup::Pause { menu } => {
                if i == Input::Esc {
                    self.popup = None;
                    return;
                }
                let Some(k) = menu_key(i) else { return };
                let Some(pick) = menu.handle(k) else { return };
                let item = menu.items[pick].clone();
                if item == "Resume" {
                    self.popup = None;
                } else if item == "Quit" {
                    self.quit = true;
                } else if item == "How to play" {
                    self.popup = Some(Popup::HowTo);
                } else if item.starts_with("Animations") {
                    self.fast = !self.fast;
                    menu.items[pick] = speed_label(self.fast);
                } else if item == "New duel" && !in_setup {
                    self.new_duel();
                }
            }
        }
    }

    fn apply(&mut self, i: Input) {
        // While a move plays, only Space/Enter (skip) do anything.
        if let Screen::Animate { action, collapse } = &mut self.screen {
            if matches!(i, Input::Space | Input::Enter) {
                if !action.done() {
                    if action.skip() {
                        self.on_impact();
                    }
                } else {
                    *collapse = 1.0;
                }
            }
            return;
        }
        let mode = self.mode;
        match &mut self.screen {
            Screen::Setup { stage, who, field, menu, names, classes } => match *stage {
                SetupStage::Mode => {
                    let Some(k) = menu_key(i) else { return };
                    let Some(pick) = menu.handle(k) else { return };
                    self.mode = if pick == 0 { Mode::Pvp } else { Mode::Bot };
                    *stage = SetupStage::Name;
                    *who = 0;
                    *field = NameField { text: names[0].clone() };
                }
                SetupStage::Name => match i {
                    Input::Char(c) => field.push(c),
                    Input::Backspace => field.backspace(),
                    Input::Enter => {
                        if let Some(name) = field.accept() {
                            names[*who] = name;
                            *stage = SetupStage::Class;
                            *menu = class_menu_at(classes[*who]);
                        }
                    }
                    _ => {}
                },
                SetupStage::Class => {
                    let Some(k) = menu_key(i) else { return };
                    let Some(pick) = menu.handle(k) else { return };
                    // The bot's class menu has a fourth item: Random.
                    let class = if pick >= 3 { Class::ALL[self.rng.below(3) as usize] } else { Class::ALL[pick] };
                    classes[*who] = class;
                    if *who == 0 {
                        *who = 1;
                        if mode == Mode::Bot {
                            names[1] = BOT_NAME.to_string();
                            *menu = bot_class_menu();
                        } else {
                            *stage = SetupStage::Name;
                            *field = NameField { text: names[1].clone() };
                        }
                    } else {
                        let duel = Duel::new(Fighter::new(&names[0], classes[0]), Fighter::new(&names[1], classes[1]));
                        self.start_duel(duel);
                        self.menu_wait = MENU_GRACE;
                    }
                }
            },
            Screen::Choose { .. } | Screen::Result { .. } if self.menu_wait > 0.0 => {}
            Screen::Choose { .. } if self.mode == Mode::Bot && self.duel.as_ref().is_some_and(|d| d.turn == 1) => {}
            Screen::Choose { menu } => {
                let Some(k) = menu_key(i) else { return };
                let Some(pick) = menu.handle(k) else {
                    // A dimmed row: flash it so the press isn't silently lost.
                    if let Key::Digit(d) = k {
                        menu.flash_row((d as usize).wrapping_sub(1));
                    } else if k == Key::Enter {
                        let sel = menu.sel;
                        menu.flash_row(sel);
                    }
                    return;
                };
                let duel = self.duel.as_ref().expect("duel");
                let mv = moves(duel.f[duel.turn].class)[pick];
                if mv == Move::Flee {
                    self.popup = Some(Popup::ConfirmFlee { menu: Menu::plain(&["Stay", "Flee"]) });
                } else {
                    self.play(mv);
                }
            }
            Screen::Animate { .. } => {}
            Screen::Result { menu } => {
                let Some(k) = menu_key(i) else { return };
                match menu.handle(k) {
                    Some(0) => {
                        let mut d = self.duel.take().expect("duel");
                        d.rematch();
                        self.start_duel(d);
                        self.menu_wait = MENU_GRACE;
                    }
                    Some(1) => self.new_duel(),
                    Some(_) => self.quit = true,
                    None => {}
                }
            }
        }
    }

    /// Classes and dim amounts to show at each fighter's spot.
    pub fn preview(&self) -> ([Class; 2], [f32; 2]) {
        match &self.screen {
            Screen::Setup { stage, who, menu, classes, .. } => {
                let mut c = [Class::Warrior, Class::Warrior];
                for (i, slot) in c.iter_mut().enumerate() {
                    if i < *who {
                        *slot = classes[i];
                    } else if i == *who && *stage == SetupStage::Class {
                        *slot = if menu.sel < 3 { Class::ALL[menu.sel] } else { classes[i] };
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

    /// The fighter whose turn it is; while a move plays, the one making it; during
    /// setup, the player choosing.
    pub fn active(&self) -> usize {
        match &self.screen {
            Screen::Animate { action, .. } => action.res.actor,
            Screen::Setup { who, .. } => *who,
            _ => self.duel.as_ref().map_or(0, |d| d.turn),
        }
    }

    /// True while a name is being typed: letter hotkeys must not fire.
    pub fn typing(&self) -> bool {
        self.popup.is_none() && matches!(self.screen, Screen::Setup { stage: SetupStage::Name, .. })
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

    /// The move menu's header and the player whose colour it takes.
    pub fn header(&self) -> Option<(String, usize)> {
        let d = self.duel.as_ref()?;
        if !matches!(self.screen, Screen::Choose { .. }) {
            return None;
        }
        if self.bot_turn() {
            return Some(("Bot is thinking…".to_string(), 1));
        }
        Some((format!("{}'s move · Turn {}/{}", d.f[d.turn].name, d.turn_number(), TURN_LIMIT), d.turn))
    }

    /// Result banner text and the player whose colour it takes (`None` for a draw).
    pub fn banner(&self) -> Option<(String, Option<usize>)> {
        if !matches!(self.screen, Screen::Result { .. }) {
            return None;
        }
        let d = self.duel.as_ref()?;
        match d.end? {
            End::Won { winner } => Some((format!("{} wins!", d.f[winner].name), Some(winner))),
            End::Fled { who } => Some((format!("{} fled the fight!", d.f[who].name), Some(who))),
            End::TimeUp { winner: Some(w) } => Some((format!("Time! {} wins on health", d.f[w].name), Some(w))),
            End::TimeUp { winner: None } => Some(("Draw!".to_string(), None)),
        }
    }
}

fn setup_screen(stage: SetupStage, names: [String; 2]) -> Screen {
    Screen::Setup {
        stage,
        who: 0,
        field: NameField::default(),
        menu: mode_menu(Mode::Pvp),
        names,
        classes: [Class::Warrior, Class::Warrior],
    }
}

fn mode_menu(mode: Mode) -> Menu {
    let mut m = Menu::new(
        vec!["Player vs Player".into(), "Player vs Bot".into()],
        vec!["two players, one keyboard".into(), "you against the computer".into()],
        vec![true, true],
    );
    m.sel = if mode == Mode::Bot { 1 } else { 0 };
    m
}

fn class_menu_at(selected: Class) -> Menu {
    let mut m = Menu::new(
        Class::ALL.iter().map(|c| c.name().to_string()).collect(),
        Class::ALL.iter().map(|c| c.blurb().to_string()).collect(),
        vec![true; 3],
    );
    m.sel = Class::ALL.iter().position(|&c| c == selected).unwrap_or(0);
    m
}

fn bot_class_menu() -> Menu {
    let mut m = class_menu_at(Class::Warrior);
    m.items.push("Random".into());
    m.notes.push("let fate decide".into());
    m.enabled.push(true);
    m
}

fn speed_label(fast: bool) -> String {
    format!("Animations: {}", if fast { "Fast" } else { "Normal" })
}

fn pause_popup(in_setup: bool, fast: bool) -> Popup {
    let menu = if in_setup {
        Menu::plain(&["Resume", "Quit"])
    } else {
        let mut m = Menu::plain(&["Resume", "How to play", "", "New duel", "Quit"]);
        m.items[2] = speed_label(fast);
        m
    };
    Popup::Pause { menu }
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

    /// Let any move animation (and knockout collapse) finish.
    fn settle(f: &mut Flow) {
        for _ in 0..100 {
            if !matches!(f.screen, Screen::Animate { .. }) {
                break;
            }
            f.update(0.1);
        }
        assert!(!matches!(f.screen, Screen::Animate { .. }), "animation never finished");
        // Let the new menu's input grace pass.
        f.update(0.3);
    }

    /// Feed each input as its own frame.
    fn frames(f: &mut Flow, inputs: &[Input]) {
        for i in inputs {
            f.step(std::slice::from_ref(i));
        }
    }

    /// A player-vs-player setup, past the mode menu.
    fn pvp() -> Flow {
        let mut f = Flow::new();
        frames(&mut f, &[Digit(1)]);
        f
    }

    fn stage(f: &Flow) -> String {
        match &f.popup {
            Some(Popup::Pause { .. }) => return "pause".into(),
            Some(Popup::HowTo) => return "howto".into(),
            Some(Popup::ConfirmFlee { .. }) => return "confirm".into(),
            None => {}
        }
        match &f.screen {
            Screen::Setup { stage: SetupStage::Mode, .. } => "setup mode".into(),
            Screen::Setup { stage: SetupStage::Name, who, .. } => format!("setup {who} name"),
            Screen::Setup { stage: SetupStage::Class, who, .. } => format!("setup {who} class"),
            Screen::Choose { .. } => format!("choose {}", f.active()),
            Screen::Animate { .. } => "animate".into(),
            Screen::Result { .. } => "result".into(),
        }
    }

    #[test]
    fn setup_full_flow() {
        let mut f = pvp();
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
        let mut f = pvp();
        assert!(f.typing());
        assert_eq!(f.names(), ["Player 1".to_string(), "Player 2".to_string()]);
        frames(&mut f, &typed("Ann"));
        frames(&mut f, &[Enter]);
        assert!(!f.typing());
        assert_eq!(f.names()[0], "Ann");
    }

    #[test]
    fn empty_name_is_not_accepted() {
        let mut f = pvp();
        frames(&mut f, &[Char(' '), Enter]);
        assert_eq!(stage(&f), "setup 0 name");
    }

    #[test]
    fn enter_accepting_name_does_not_confirm_class() {
        let mut f = pvp();
        frames(&mut f, &typed("Ann"));
        f.step(&[Enter, Enter]);
        assert_eq!(stage(&f), "setup 0 class");
    }

    #[test]
    fn digit_picking_class_does_not_land_in_next_name() {
        let mut f = pvp();
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
        let mut f = pvp();
        f.step(&[Digit(7), Char('7')]);
        match &f.screen {
            Screen::Setup { field, stage: SetupStage::Name, .. } => assert_eq!(field.text, "7"),
            _ => panic!("still naming"),
        }
    }

    #[test]
    fn class_preview_follows_highlight() {
        let mut f = pvp();
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
        settle(&mut f);
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
                assert_eq!(menu.reasons[1], "shield already full");
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn knockout_and_rematch() {
        let mut f = Flow::with_duel(["A", "B"], [Class::Warrior, Class::Assassin]);
        f.duel.as_mut().unwrap().f[1].health = 10;
        frames(&mut f, &[Digit(1)]);
        settle(&mut f);
        assert_eq!(stage(&f), "result");
        assert_eq!(f.banner(), Some(("A wins!".to_string(), Some(0))));
        frames(&mut f, &[Digit(1)]);
        assert_eq!(stage(&f), "choose 1", "B starts the rematch");
        assert_eq!(f.shown.as_ref().unwrap()[1].health, 100);
    }

    #[test]
    fn flee_then_new_duel() {
        let mut f = Flow::with_duel(["A", "B"], [Class::Sorcerer, Class::Assassin]);
        frames(&mut f, &[Digit(5), Digit(2)]);
        settle(&mut f);
        assert_eq!(f.banner(), Some(("A fled the fight!".to_string(), Some(0))));
        frames(&mut f, &[Down, Enter]);
        assert_eq!(stage(&f), "setup mode");
        assert!(f.duel.is_none());
    }

    #[test]
    fn esc_never_quits() {
        let mut f = Flow::new();
        for _ in 0..5 {
            f.step(&[Esc]);
            assert!(!f.quit);
        }
        let mut g = Flow::with_duel(["A", "B"], [Class::Sorcerer, Class::Assassin]);
        for _ in 0..5 {
            g.step(&[Esc]);
            assert!(!g.quit);
        }
    }

    #[test]
    fn result_quit_item() {
        let mut f = Flow::with_duel(["A", "B"], [Class::Sorcerer, Class::Assassin]);
        frames(&mut f, &[Digit(5), Digit(2)]);
        settle(&mut f);
        frames(&mut f, &[Digit(3)]);
        assert!(f.quit);
    }

    #[test]
    fn flee_records_end() {
        let mut f = Flow::with_duel(["A", "B"], [Class::Sorcerer, Class::Assassin]);
        frames(&mut f, &[Digit(5), Digit(2)]);
        assert_eq!(f.duel.as_ref().unwrap().end, Some(End::Fled { who: 0 }));
    }

    #[test]
    fn enter_confirming_move_does_not_skip_it() {
        let mut f = Flow::with_duel(["A", "B"], [Class::Warrior, Class::Sorcerer]);
        f.step(&[Enter, Enter]);
        assert_eq!(stage(&f), "animate");
        assert_eq!(f.shown.as_ref().unwrap()[1].health, 100, "numbers change at impact, not on pick");
        match &f.screen {
            Screen::Animate { action, .. } => assert_eq!(action.t, 0.0),
            _ => unreachable!(),
        }
    }

    #[test]
    fn keys_during_animation_change_nothing() {
        let mut f = Flow::with_duel(["A", "B"], [Class::Warrior, Class::Sorcerer]);
        frames(&mut f, &[Digit(1)]);
        frames(&mut f, &[Digit(2), Up, Down, Digit(1), Char('x'), Backspace]);
        assert_eq!(stage(&f), "animate");
        assert_eq!(f.duel.as_ref().unwrap().turn, 1);
        settle(&mut f);
        assert_eq!(stage(&f), "choose 1");
        assert_eq!(f.duel.as_ref().unwrap().f[1].health, 90, "only the one attack landed");
    }

    #[test]
    fn shown_numbers_change_at_impact() {
        let mut f = Flow::with_duel(["A", "B"], [Class::Warrior, Class::Sorcerer]);
        frames(&mut f, &[Digit(1)]);
        f.update(0.5);
        assert_eq!(f.shown.as_ref().unwrap()[1].health, 100);
        assert!(!f.take_impact());
        f.update(0.1);
        assert!(f.take_impact());
        assert_eq!(f.shown.as_ref().unwrap()[1].health, 90);
        f.update(0.1);
        assert!(!f.take_impact());
    }

    #[test]
    fn skip_applies_impact_once() {
        let mut f = Flow::with_duel(["A", "B"], [Class::Warrior, Class::Sorcerer]);
        frames(&mut f, &[Digit(1)]);
        f.step(&[Space]);
        assert!(f.take_impact());
        assert_eq!(f.shown.as_ref().unwrap()[1].health, 90);
        f.step(&[Space]);
        f.update(0.016);
        assert_eq!(stage(&f), "choose 1");
        assert_eq!(f.shown.as_ref().unwrap()[1].health, 90);
    }

    #[test]
    fn knockout_collapses_before_result() {
        let mut f = Flow::with_duel(["A", "B"], [Class::Warrior, Class::Assassin]);
        f.duel.as_mut().unwrap().f[1].health = 10;
        frames(&mut f, &[Digit(1), Space]);
        f.update(0.016);
        assert_eq!(stage(&f), "animate", "loser collapses first");
        f.update(0.5);
        let (poses, _) = f.body_state(0.0, 3.8);
        assert!(poses[1].fall > 0.2);
        f.step(&[Space]);
        f.update(0.016);
        assert_eq!(stage(&f), "result");
        assert_eq!(f.body_state(0.0, 3.8).0[1].fall, 1.0, "stays down on the result screen");
    }

    #[test]
    fn flee_fades_the_fleeing_fighter() {
        let mut f = Flow::with_duel(["A", "B"], [Class::Warrior, Class::Assassin]);
        frames(&mut f, &[Digit(5), Digit(2)]);
        settle(&mut f);
        assert!(f.body_state(0.0, 3.8).1[0] >= 0.99);
        assert_eq!(f.body_state(0.0, 3.8).1[1], 0.0);
    }

    #[test]
    fn attacker_lunges_mid_move() {
        let mut f = Flow::with_duel(["A", "B"], [Class::Warrior, Class::Assassin]);
        frames(&mut f, &[Digit(1)]);
        f.update(0.55);
        assert!(f.body_state(0.0, 3.8).0[0].root.z > 2.0);
    }

    #[test]
    fn active_is_the_actor_while_animating() {
        let mut f = Flow::with_duel(["A", "B"], [Class::Warrior, Class::Sorcerer]);
        frames(&mut f, &[Digit(1)]);
        assert_eq!(f.active(), 0);
        settle(&mut f);
        assert_eq!(f.active(), 1);
    }

    #[test]
    fn result_lines_appear_at_impact() {
        let mut f = Flow::with_duel(["A", "B"], [Class::Sorcerer, Class::Warrior]);
        frames(&mut f, &[Digit(1)]);
        assert_eq!(f.caption.len(), 1, "only the action line before impact");
        f.step(&[Space]);
        assert_eq!(f.caption[1], "B's shield absorbs 12 damage (13/25 remaining).");
    }

    #[test]
    fn mashing_enter_after_a_skip_does_not_play_the_next_move() {
        let mut f = Flow::with_duel(["A", "B"], [Class::Warrior, Class::Sorcerer]);
        frames(&mut f, &[Digit(1)]);
        for _ in 0..3 {
            f.step(&[Enter]);
            f.update(1.0 / 60.0);
        }
        assert_eq!(stage(&f), "choose 1");
        assert_eq!(f.duel.as_ref().unwrap().f[0].health, 100, "B did not act");
        f.update(0.3);
        frames(&mut f, &[Enter]);
        assert_eq!(stage(&f), "animate", "the menu works once the grace passes");
    }

    #[test]
    fn mashing_enter_after_a_knockout_shows_the_result() {
        let mut f = Flow::with_duel(["A", "B"], [Class::Warrior, Class::Assassin]);
        f.duel.as_mut().unwrap().f[1].health = 10;
        frames(&mut f, &[Digit(1)]);
        for _ in 0..4 {
            f.step(&[Enter]);
            f.update(1.0 / 60.0);
        }
        assert_eq!(stage(&f), "result");
    }

    #[test]
    fn class_pick_mash_does_not_play_first_move() {
        let mut f = pvp();
        frames(&mut f, &typed("A"));
        frames(&mut f, &[Enter, Enter]);
        frames(&mut f, &typed("B"));
        frames(&mut f, &[Enter]);
        f.step(&[Enter]);
        f.update(1.0 / 60.0);
        f.step(&[Enter]);
        assert_eq!(stage(&f), "choose 0");
    }

    #[test]
    fn knockout_with_hit_stop_reaches_result() {
        let mut f = Flow::with_duel(["A", "B"], [Class::Warrior, Class::Assassin]);
        f.duel.as_mut().unwrap().f[1].health = 10;
        frames(&mut f, &[Digit(1)]);
        let mut t = 0.0;
        while stage(&f) != "result" && t < 3.0 {
            f.update(1.0 / 60.0);
            t += 1.0 / 60.0;
        }
        assert_eq!(stage(&f), "result", "after {t} s");
    }

    #[test]
    fn mode_then_names_pvp() {
        let mut f = Flow::new();
        assert_eq!(stage(&f), "setup mode");
        frames(&mut f, &[Digit(1)]);
        assert_eq!((stage(&f), f.mode), ("setup 0 name".to_string(), Mode::Pvp));
    }

    #[test]
    fn bot_mode_setup() {
        let mut f = Flow::new();
        frames(&mut f, &[Digit(2)]);
        assert_eq!(f.mode, Mode::Bot);
        frames(&mut f, &typed("Ann"));
        frames(&mut f, &[Enter, Digit(1)]);
        assert_eq!(stage(&f), "setup 1 class");
        match &f.screen {
            Screen::Setup { menu, names, .. } => {
                assert_eq!(menu.items.len(), 4);
                assert_eq!(menu.items[3], "Random");
                assert_eq!(names[1], BOT_NAME);
            }
            _ => unreachable!(),
        }
        frames(&mut f, &[Digit(3)]);
        let d = f.duel.as_ref().unwrap();
        assert_eq!((d.f[1].name.as_str(), d.f[1].class), ("Bot", Class::Sorcerer));
    }

    #[test]
    fn bot_random_class_picks_a_real_class() {
        let mut f = Flow::new();
        frames(&mut f, &[Digit(2)]);
        frames(&mut f, &typed("Ann"));
        frames(&mut f, &[Enter, Digit(1), Digit(4)]);
        assert!(f.duel.is_some());
    }

    #[test]
    fn esc_backs_out_of_setup() {
        let mut f = pvp();
        frames(&mut f, &typed("Ann"));
        frames(&mut f, &[Enter, Digit(2)]);
        assert_eq!(stage(&f), "setup 1 name");
        frames(&mut f, &[Esc]);
        assert_eq!(stage(&f), "setup 0 class");
        frames(&mut f, &[Esc]);
        assert_eq!(stage(&f), "setup 0 name");
        match &f.screen {
            Screen::Setup { field, .. } => assert_eq!(field.text, "Ann", "name kept"),
            _ => unreachable!(),
        }
        frames(&mut f, &[Esc]);
        assert_eq!(stage(&f), "setup mode");
        frames(&mut f, &[Esc]);
        assert_eq!(stage(&f), "pause");
        frames(&mut f, &[Esc]);
        assert_eq!(stage(&f), "setup mode");
        assert!(!f.quit);
    }

    #[test]
    fn esc_opens_pause_and_closes() {
        let mut f = Flow::with_duel(["A", "B"], [Class::Warrior, Class::Sorcerer]);
        frames(&mut f, &[Esc]);
        assert_eq!(stage(&f), "pause");
        frames(&mut f, &[Esc]);
        assert_eq!(stage(&f), "choose 0");
        frames(&mut f, &[Esc, Digit(1)]);
        assert_eq!(stage(&f), "choose 0", "Resume");
    }

    #[test]
    fn pause_blocks_input_and_animation() {
        let mut f = Flow::with_duel(["A", "B"], [Class::Warrior, Class::Sorcerer]);
        frames(&mut f, &[Digit(1)]);
        f.update(0.2);
        let t0 = match &f.screen {
            Screen::Animate { action, .. } => action.t,
            _ => unreachable!(),
        };
        frames(&mut f, &[Esc]);
        f.update(2.0);
        frames(&mut f, &[Space]);
        match &f.screen {
            Screen::Animate { action, .. } => assert_eq!(action.t, t0, "frozen under the pause"),
            _ => panic!("animation must still be running"),
        }
        assert_eq!(f.duel.as_ref().unwrap().f[1].health, 90, "only the one move");
        frames(&mut f, &[Esc]);
        settle(&mut f);
        assert_eq!(stage(&f), "choose 1");
    }

    #[test]
    fn pause_toggles_speed() {
        let mut f = Flow::with_duel(["A", "B"], [Class::Warrior, Class::Sorcerer]);
        frames(&mut f, &[Esc, Digit(3)]);
        assert!(f.fast);
        assert_eq!(stage(&f), "pause", "toggle stays in the pause menu");
        match &f.popup {
            Some(Popup::Pause { menu }) => assert_eq!((menu.items[2].as_str(), menu.sel), ("Animations: Fast", 2)),
            _ => unreachable!(),
        }
        frames(&mut f, &[Digit(3)]);
        assert!(!f.fast);
    }

    #[test]
    fn fast_halves_animation() {
        let run = |fast: bool| {
            let mut f = Flow::with_duel(["A", "B"], [Class::Warrior, Class::Sorcerer]);
            f.fast = fast;
            frames(&mut f, &[Digit(4)]);
            let mut n = 0;
            while matches!(f.screen, Screen::Animate { .. }) {
                f.update(1.0 / 60.0);
                n += 1;
            }
            n
        };
        let (slow, fast) = (run(false), run(true));
        assert!((fast as f32 - slow as f32 / 2.0).abs() <= 2.0, "{fast} vs {slow}");
    }

    #[test]
    fn howto_any_key_closes() {
        let mut f = Flow::with_duel(["A", "B"], [Class::Warrior, Class::Sorcerer]);
        frames(&mut f, &[Esc, Digit(2)]);
        assert_eq!(stage(&f), "howto");
        frames(&mut f, &[Char('x')]);
        assert_eq!(stage(&f), "pause");
    }

    #[test]
    fn flee_needs_confirm() {
        let mut f = Flow::with_duel(["A", "B"], [Class::Warrior, Class::Sorcerer]);
        frames(&mut f, &[Digit(5)]);
        assert_eq!(stage(&f), "confirm");
        frames(&mut f, &[Enter]);
        assert_eq!(stage(&f), "choose 0", "Stay is the default");
        assert!(f.duel.as_ref().unwrap().end.is_none());
        frames(&mut f, &[Digit(5), Digit(2)]);
        assert_eq!(f.duel.as_ref().unwrap().end, Some(End::Fled { who: 0 }));
    }

    #[test]
    fn quit_only_from_menu_items() {
        let mut f = Flow::with_duel(["A", "B"], [Class::Warrior, Class::Sorcerer]);
        frames(&mut f, &[Esc, Digit(5)]);
        assert!(f.quit);
    }

    #[test]
    fn bot_plays_after_think() {
        let mut f = Flow::with_duel(["A", "Bot"], [Class::Warrior, Class::Sorcerer]);
        f.mode = Mode::Bot;
        frames(&mut f, &[Digit(4)]);
        settle(&mut f);
        assert_eq!(stage(&f), "choose 1");
        f.update(0.3);
        assert_eq!(stage(&f), "choose 1", "still thinking");
        f.update(0.5);
        assert_eq!(stage(&f), "animate");
        assert_eq!(f.last.as_ref().unwrap().actor, 1);
    }

    #[test]
    fn human_keys_ignored_on_bot_turn() {
        let mut f = Flow::with_duel(["A", "Bot"], [Class::Warrior, Class::Sorcerer]);
        f.mode = Mode::Bot;
        frames(&mut f, &[Digit(4)]);
        settle(&mut f);
        frames(&mut f, &[Digit(1), Digit(4), Enter]);
        assert_eq!(stage(&f), "choose 1");
    }

    #[test]
    fn bot_first_after_rematch_acts() {
        let mut f = Flow::with_duel(["A", "Bot"], [Class::Warrior, Class::Assassin]);
        f.mode = Mode::Bot;
        f.duel.as_mut().unwrap().f[1].health = 10;
        frames(&mut f, &[Digit(1)]);
        settle(&mut f);
        frames(&mut f, &[Digit(1)]);
        assert_eq!(stage(&f), "choose 1", "bot starts the rematch");
        f.update(1.0);
        assert_eq!(stage(&f), "animate");
    }

    #[test]
    fn score_counts_wins_and_resets_on_new_duel() {
        let mut f = Flow::with_duel(["A", "B"], [Class::Warrior, Class::Assassin]);
        f.duel.as_mut().unwrap().f[1].health = 10;
        frames(&mut f, &[Digit(1)]);
        settle(&mut f);
        assert_eq!(f.score, [1, 0]);
        frames(&mut f, &[Digit(1)]);
        f.update(0.3);
        frames(&mut f, &[Digit(5), Digit(2)]);
        settle(&mut f);
        assert_eq!(f.score, [1, 0], "a flee scores nothing");
        frames(&mut f, &[Digit(2)]);
        assert_eq!(f.score, [0, 0]);
    }

    #[test]
    fn log_keeps_last_four() {
        let mut f = Flow::with_duel(["A", "B"], [Class::Warrior, Class::Sorcerer]);
        for _ in 0..6 {
            frames(&mut f, &[Digit(4)]);
            settle(&mut f);
        }
        assert_eq!(f.log.len(), 4);
        assert!(f.log.iter().all(|l| l.contains("Rest")));
    }

    #[test]
    fn new_duel_prefills_names() {
        let mut f = Flow::with_duel(["Ann", "Bo"], [Class::Warrior, Class::Sorcerer]);
        frames(&mut f, &[Digit(5), Digit(2)]);
        settle(&mut f);
        frames(&mut f, &[Digit(2), Digit(1)]);
        match &f.screen {
            Screen::Setup { field, .. } => assert_eq!(field.text, "Ann"),
            _ => unreachable!(),
        }
    }

    #[test]
    fn time_up_banner_and_draw() {
        let mut f = Flow::with_duel(["A", "B"], [Class::Warrior, Class::Assassin]);
        f.duel.as_mut().unwrap().moves = [TURN_LIMIT - 1, TURN_LIMIT - 1];
        frames(&mut f, &[Digit(4)]);
        settle(&mut f);
        frames(&mut f, &[Digit(4)]);
        settle(&mut f);
        assert_eq!(stage(&f), "result");
        assert_eq!(f.banner(), Some(("Draw!".to_string(), None)));
        assert_eq!(f.score, [0, 0]);
    }

    #[test]
    fn header_text() {
        let mut f = Flow::with_duel(["Ann", "Bot"], [Class::Warrior, Class::Sorcerer]);
        assert_eq!(f.header(), Some(("Ann's move · Turn 1/40".to_string(), 0)));
        f.mode = Mode::Bot;
        frames(&mut f, &[Digit(4)]);
        settle(&mut f);
        assert_eq!(f.header(), Some(("Bot is thinking…".to_string(), 1)));
    }

    #[test]
    fn mn_targets_the_player_choosing_in_setup() {
        let mut f = pvp();
        frames(&mut f, &typed("A"));
        frames(&mut f, &[Enter, Digit(1)]);
        assert_eq!(f.active(), 1);
    }

    #[test]
    fn dimmed_press_flashes() {
        let mut f = Flow::with_duel(["A", "B"], [Class::Warrior, Class::Sorcerer]);
        frames(&mut f, &[Digit(2)]);
        match &f.screen {
            Screen::Choose { menu } => {
                assert!(menu.flash > 0.0 && menu.flash_row == 1);
                assert_eq!(menu.reasons[1], "shield already full");
                assert_eq!(menu.notes[0], "10 ST · 10 dmg");
            }
            _ => unreachable!(),
        }
        f.update(0.5);
        match &f.screen {
            Screen::Choose { menu } => assert_eq!(menu.flash, 0.0),
            _ => unreachable!(),
        }
    }
}
