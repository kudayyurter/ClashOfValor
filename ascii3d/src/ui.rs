//! Menus, the name field, and drawing of panels, menus, captions and banners.
//!
//! Follows the project's menu rules: option lists centred on the live window width,
//! the selected row in reverse video (no `>` prompt), arrows move, Enter confirms,
//! number keys jump to an option and confirm it.

use crate::rules::{Class, Fighter};
use macroquad::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Key {
    Up,
    Down,
    Enter,
    Digit(u8),
}

#[derive(Clone, Debug)]
pub struct Menu {
    pub items: Vec<String>,
    /// Dim reason or description per item; "" for none.
    pub notes: Vec<String>,
    pub enabled: Vec<bool>,
    pub sel: usize,
    /// Why a dimmed row can't be picked; "" for enabled rows.
    pub reasons: Vec<String>,
    /// Seconds left on a "can't pick that" flash, and which row.
    pub flash: f32,
    pub flash_row: usize,
}

impl Menu {
    pub fn new(items: Vec<String>, notes: Vec<String>, enabled: Vec<bool>) -> Menu {
        let sel = enabled.iter().position(|&e| e).unwrap_or(0);
        let reasons = vec![String::new(); items.len()];
        Menu { items, notes, enabled, reasons, sel, flash: 0.0, flash_row: 0 }
    }

    /// Flash a row (a dimmed row the player tried to pick).
    pub fn flash_row(&mut self, i: usize) {
        if i < self.items.len() {
            self.flash = 0.4;
            self.flash_row = i;
        }
    }

    /// Let a flash fade.
    pub fn tick(&mut self, dt: f32) {
        self.flash = (self.flash - dt).max(0.0);
    }

    /// A menu whose items are all enabled and have no notes.
    pub fn plain(items: &[&str]) -> Menu {
        let n = items.len();
        Menu::new(items.iter().map(|s| s.to_string()).collect(), vec![String::new(); n], vec![true; n])
    }

    /// Returns the confirmed 0-based index.
    pub fn handle(&mut self, k: Key) -> Option<usize> {
        let n = self.items.len();
        if n == 0 {
            return None;
        }
        match k {
            Key::Up | Key::Down => {
                let step = if k == Key::Down { 1 } else { n - 1 };
                let mut i = self.sel;
                for _ in 0..n {
                    i = (i + step) % n;
                    if self.enabled[i] {
                        self.sel = i;
                        break;
                    }
                }
                None
            }
            Key::Enter => self.enabled[self.sel].then_some(self.sel),
            Key::Digit(d) => {
                let i = (d as usize).checked_sub(1)?;
                if i < n && self.enabled[i] {
                    self.sel = i;
                    Some(i)
                } else {
                    None
                }
            }
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct NameField {
    pub text: String,
}

pub const NAME_MAX: usize = 16;

impl NameField {
    /// Printable ASCII only (the font may lack other glyphs), no leading space.
    pub fn push(&mut self, c: char) {
        if (' '..='~').contains(&c) && self.text.len() < NAME_MAX && !(c == ' ' && self.text.is_empty()) {
            self.text.push(c);
        }
    }

    pub fn backspace(&mut self) {
        self.text.pop();
    }

    pub fn accept(&self) -> Option<String> {
        let t = self.text.trim();
        (!t.is_empty()).then(|| t.to_string())
    }
}

pub const FONT: u16 = 16;
pub const ROW: f32 = 22.0;
pub const DIM: Color = Color::new(0.55, 0.57, 0.65, 1.0);
pub const TEXT: Color = Color::new(0.9, 0.9, 0.95, 1.0);
const PANEL_BG: Color = Color::new(0.0, 0.0, 0.0, 0.72);

pub fn rgb(v: Vec3) -> Color {
    Color::new(v.x, v.y, v.z, 1.0)
}

fn tp(font: &Font, size: u16, color: Color) -> TextParams<'_> {
    TextParams { font: Some(font), font_size: size, color, ..Default::default() }
}

pub fn text_w(font: &Font, s: &str, size: u16) -> f32 {
    measure_text(s, Some(font), size, 1.0).width
}

/// Draw `s` centred on `cx` with its baseline at `y`.
pub fn text_centered(font: &Font, s: &str, cx: f32, y: f32, size: u16, color: Color) {
    draw_text_ex(s, cx - text_w(font, s, size) * 0.5, y, tp(font, size, color));
}

/// The cost-and-effect column for a move, from the fighter's point of view.
pub fn move_cost(f: &Fighter, m: crate::rules::Move) -> String {
    use crate::rules::Move::*;
    match m {
        Attack => format!("10 ST · {} dmg", f.damage),
        RaiseShield => "15 ST · +25 shield".to_string(),
        CritAttack => format!("20 ST · {} dmg", (f.damage as f32 * f.crit) as i32),
        FireOrb => format!("10 MP 10 ST · {} dmg", f.damage + 2),
        HealthPotion => format!("+50 HP · {} left", f.health_potions),
        ManaPotion => format!("+50 MP · {} left", f.mana_potions),
        Rest if f.class == Class::Sorcerer => "+10 HP +25 ST +20 MP".to_string(),
        Rest => "+10 HP +25 ST".to_string(),
        Flee => "ends the duel".to_string(),
    }
}

/// Health bar colour: green above half, gold to a quarter, red below; pulses when
/// nearly dead.
pub fn hp_color(frac: f32, time: f32) -> Vec3 {
    let base = if frac > 0.5 {
        HEALTH
    } else if frac > 0.25 {
        Vec3::new(0.95, 0.75, 0.25)
    } else {
        Vec3::new(0.95, 0.3, 0.25)
    };
    if frac < 0.15 {
        base * (0.75 + 0.25 * (time * std::f32::consts::TAU * 2.0).cos())
    } else {
        base
    }
}

/// Stamina has no cap: its bar fills against 100.
pub fn stamina_fill(shown: f32) -> f32 {
    (shown / 100.0).clamp(0.0, 1.0)
}

/// Fraction of a capped bar.
fn fill(shown: f32, max: i32) -> f32 {
    if max > 0 { (shown / max as f32).clamp(0.0, 1.0) } else { 0.0 }
}

/// `s` cut to at most `max_chars` characters, ending in `…` when cut.
pub fn fit(s: &str, max_chars: usize) -> String {
    let n = s.chars().count();
    if n <= max_chars {
        return s.to_string();
    }
    if max_chars < 2 {
        return String::new();
    }
    let mut out: String = s.chars().take(max_chars - 1).collect();
    out.push('…');
    out
}

/// Word-wrap to lines of at most `max_chars`; over-long words are split. An empty
/// string gives no lines.
pub fn wrap(s: &str, max_chars: usize) -> Vec<String> {
    let max = max_chars.max(1);
    let mut lines = Vec::new();
    let mut cur = String::new();
    for word in s.split_whitespace() {
        let mut word: Vec<char> = word.chars().collect();
        // Hard-split words that can never fit on one line.
        while word.len() > max {
            if !cur.is_empty() {
                lines.push(std::mem::take(&mut cur));
            }
            lines.push(word.drain(..max).collect());
        }
        let w: String = word.into_iter().collect();
        if w.is_empty() {
            continue;
        }
        let need = if cur.is_empty() { w.chars().count() } else { cur.chars().count() + 1 + w.chars().count() };
        if need > max {
            lines.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur.push_str(&w);
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}

/// Characters of `size` px text that fit in `width` px.
pub fn chars_in(font: &Font, width: f32, size: u16) -> usize {
    (width / text_w(font, "M", size).max(1.0)).floor().max(0.0) as usize
}

/// A box-drawing frame (`┌─┐│└┘`) around the rectangle, in glyphs.
pub fn draw_frame(font: &Font, x: f32, y: f32, w: f32, h: f32, color: Color) {
    let size = 13;
    let cw = text_w(font, "─", size).max(1.0);
    let n = ((w / cw).round() as usize).max(2);
    let top = format!("┌{}┐", "─".repeat(n - 2));
    let bottom = format!("└{}┘", "─".repeat(n - 2));
    draw_text_ex(&top, x, y + 9.0, tp(font, size, color));
    draw_text_ex(&bottom, x, y + h + 3.0, tp(font, size, color));
    let right = x + (n - 1) as f32 * cw;
    let mut yy = y + 9.0 + 12.0;
    while yy < y + h + 3.0 - 4.0 {
        draw_text_ex("│", x, yy, tp(font, size, color));
        draw_text_ex("│", right, yy, tp(font, size, color));
        yy += 12.0;
    }
}

pub fn menu_height(m: &Menu) -> f32 {
    m.items.len() as f32 * ROW
}

/// The option list as a block centred on `cx`. The selected row is drawn in reverse
/// video; disabled rows are dimmed and show their note.
pub fn draw_menu(font: &Font, m: &Menu, cx: f32, top: f32) {
    // Enabled rows show their note (cost, description); dimmed rows show why not.
    let rows: Vec<(String, String, bool)> = m
        .items
        .iter()
        .enumerate()
        .map(|(i, it)| {
            let reason = m.reasons.get(i).filter(|r| !r.is_empty() && !m.enabled[i]);
            let note = reason.or(m.notes.get(i)).cloned().unwrap_or_default();
            (format!(" {} {it} ", i + 1), note, reason.is_some())
        })
        .collect();
    let label_w = rows.iter().map(|(l, _, _)| text_w(font, l, FONT)).fold(0.0, f32::max);
    let note_w = rows.iter().map(|(_, n, _)| if n.is_empty() { 0.0 } else { text_w(font, n, 13) + 14.0 }).fold(0.0, f32::max);
    let x = (cx - (label_w + note_w) * 0.5).max(6.0);
    draw_rectangle(x - 8.0, top - 6.0, label_w + note_w + 16.0, menu_height(m) + 12.0, PANEL_BG);
    draw_frame(font, x - 12.0, top - 10.0, label_w + note_w + 26.0, menu_height(m) + 16.0, Color::new(0.4, 0.42, 0.5, 1.0));
    for (i, (label, note, why)) in rows.iter().enumerate() {
        let y = top + i as f32 * ROW;
        let on = m.enabled[i];
        let flash = if m.flash > 0.0 && m.flash_row == i { m.flash / 0.4 } else { 0.0 };
        if flash > 0.0 {
            draw_rectangle(x, y, label_w + note_w, ROW - 2.0, Color::new(0.85, 0.3, 0.25, 0.35 * flash));
        }
        if i == m.sel && on {
            draw_rectangle(x, y, label_w, ROW - 2.0, TEXT);
            draw_text_ex(label, x, y + 15.0, tp(font, FONT, Color::new(0.03, 0.03, 0.07, 1.0)));
        } else {
            draw_text_ex(label, x, y + 15.0, tp(font, FONT, if on { TEXT } else { Color::new(0.4, 0.4, 0.46, 1.0) }));
        }
        if !note.is_empty() {
            let c = if *why { Color::new(0.85, 0.45 + 0.4 * flash, 0.4 + 0.4 * flash, 1.0) } else { DIM };
            draw_text_ex(note, x + label_w + 14.0, y + 15.0, tp(font, 13, c));
        }
    }
}

/// "Ann's move · Turn 3/40" in the player's colour, centred just above a menu.
pub fn draw_menu_header(font: &Font, text: &str, accent: Vec3, cx: f32, y: f32) {
    let w = text_w(font, text, 15);
    draw_rectangle(cx - w * 0.5 - 10.0, y - 15.0, w + 20.0, 21.0, Color::new(accent.x, accent.y, accent.z, 0.28));
    text_centered(font, text, cx, y, 15, TEXT);
}

/// The last few moves, newest at the bottom.
pub fn draw_log(font: &Font, lines: &[String], x: f32, y: f32, w: f32) {
    if lines.is_empty() {
        return;
    }
    let h = lines.len() as f32 * 15.0 + 10.0;
    draw_rectangle(x, y, w, h, Color::new(0.0, 0.0, 0.0, 0.55));
    let max = chars_in(font, w - 12.0, 12);
    let n = lines.len();
    for (i, l) in lines.iter().enumerate() {
        let a = 0.45 + 0.55 * (i + 1) as f32 / n as f32;
        draw_text_ex(fit(l, max), x + 6.0, y + 16.0 + i as f32 * 15.0, tp(font, 12, Color::new(0.85, 0.86, 0.92, a)));
    }
}

/// A centred, framed plate over a dimmed scene.
fn plate(font: &Font, w: f32, h: f32) -> (f32, f32) {
    let (sw, sh) = (screen_width(), screen_height());
    draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.45));
    let (x, y) = ((sw - w) * 0.5, (sh - h) * 0.5);
    draw_rectangle(x, y, w, h, Color::new(0.02, 0.02, 0.05, 0.95));
    draw_frame(font, x, y, w, h, Color::new(0.75, 0.62, 0.3, 1.0));
    (x, y)
}

pub const HOW_TO: &[&str] = &[
    "Take turns. Knock your opponent's health to 0 to win.",
    "Each fighter gets 40 turns; then higher health % wins.",
    "",
    "Warrior   Attack 10 ST: 10 dmg.  Raise shield 15 ST: +25 shield.",
    "          The shield soaks damage before health.",
    "Assassin  Attack 10 ST: 10 dmg.  Crit 20 ST: 15 dmg.",
    "Sorcerer  Fire orb 10 MP + 10 ST: 12 dmg.  Mana potion +50 MP.",
    "Everyone  Health potion +50 HP.  Rest +10 HP +25 ST (+20 MP).",
    "          Flee ends the duel with no winner.",
    "",
    "Arrows or number keys choose, Enter confirms, Space skips a move,",
    "Esc pauses. Right-drag turns the camera.",
];

/// Pause menu, how-to-play page, or flee confirm, over everything else.
pub fn draw_popup(font: &Font, popup: &crate::flow::Popup) {
    use crate::flow::Popup;
    let cx = screen_width() * 0.5;
    match popup {
        Popup::Pause { menu } => {
            let (_, y) = plate(font, 340.0, menu_height(menu) + 70.0);
            text_centered(font, "Paused", cx, y + 30.0, 18, Color::new(0.95, 0.8, 0.4, 1.0));
            draw_menu(font, menu, cx, y + 46.0);
        }
        Popup::HowTo => {
            let w = (screen_width() - 40.0).min(620.0);
            let (x, y) = plate(font, w, HOW_TO.len() as f32 * 17.0 + 70.0);
            text_centered(font, "How to play", cx, y + 28.0, 18, Color::new(0.95, 0.8, 0.4, 1.0));
            let max = chars_in(font, w - 30.0, 13);
            for (i, l) in HOW_TO.iter().enumerate() {
                draw_text_ex(fit(l, max), x + 16.0, y + 52.0 + i as f32 * 17.0, tp(font, 13, TEXT));
            }
            text_centered(font, "press any key", cx, y + 52.0 + HOW_TO.len() as f32 * 17.0 + 4.0, 12, DIM);
        }
        Popup::ConfirmFlee { menu } => {
            let (_, y) = plate(font, 340.0, menu_height(menu) + 70.0);
            text_centered(font, "Flee the duel? This ends it.", cx, y + 30.0, 15, TEXT);
            draw_menu(font, menu, cx, y + 46.0);
        }
    }
}

/// Three rows of block-glyph lettering (`█▀▄`) for titles and banners. Each letter is
/// 3 columns wide with a 1-column gap; unknown characters become spaces.
pub fn block_text(s: &str) -> [String; 3] {
    fn glyph(c: char) -> [&'static str; 3] {
        match c {
            'A' => ["▄▀▄", "█▀█", "▀ ▀"],
            'B' => ["█▀▄", "█▀▄", "▀▀ "],
            'C' => ["▄▀▀", "█  ", " ▀▀"],
            'D' => ["█▀▄", "█ █", "▀▀ "],
            'E' => ["█▀▀", "█▀ ", "▀▀▀"],
            'F' => ["█▀▀", "█▀ ", "▀  "],
            'G' => ["▄▀▀", "█ ▄", " ▀▀"],
            'H' => ["█ █", "█▀█", "▀ ▀"],
            'I' => ["▀█▀", " █ ", "▀▀▀"],
            'J' => ["  █", "  █", "▀▀ "],
            'K' => ["█ █", "█▀▄", "▀ ▀"],
            'L' => ["█  ", "█  ", "▀▀▀"],
            'M' => ["█▄█", "█ █", "▀ ▀"],
            'N' => ["█▀█", "█ █", "▀ ▀"],
            'O' => ["▄▀▄", "█ █", " ▀ "],
            'P' => ["█▀▄", "█▀ ", "▀  "],
            'Q' => ["▄▀▄", "█ █", " ▀▄"],
            'R' => ["█▀▄", "█▀▄", "▀ ▀"],
            'S' => ["▄▀▀", " ▀▄", "▀▀ "],
            'T' => ["▀█▀", " █ ", " ▀ "],
            'U' => ["█ █", "█ █", " ▀ "],
            'V' => ["█ █", "▀▄▀", " ▀ "],
            'W' => ["█ █", "█▄█", "▀ ▀"],
            'X' => ["▀▄▀", " █ ", "▀ ▀"],
            'Y' => ["█ █", " █ ", " ▀ "],
            'Z' => ["▀▀█", "▄▀ ", "▀▀▀"],
            '!' => [" █ ", " █ ", " ▄ "],
            _ => ["   ", "   ", "   "],
        }
    }
    let mut rows = [String::new(), String::new(), String::new()];
    for (i, c) in s.chars().enumerate() {
        let g = glyph(c.to_ascii_uppercase());
        for r in 0..3 {
            if i > 0 {
                rows[r].push(' ');
            }
            rows[r].push_str(g[r]);
        }
    }
    rows
}

/// Shown instead of the game when the window is too small to lay out.
pub fn draw_too_small(font: &Font) {
    let (sw, sh) = (screen_width(), screen_height());
    draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.02, 0.02, 0.05, 1.0));
    text_centered(font, "Make the window larger", sw * 0.5, sh * 0.5, 14, TEXT);
}


#[allow(clippy::too_many_arguments)]
fn bar(font: &Font, x: f32, y: f32, w: f32, label: &str, frac: f32, text: &str, color: Vec3) {
    let lw = 30.0;
    draw_text_ex(label, x, y + 10.0, tp(font, 13, DIM));
    let bw = w - lw - 70.0;
    draw_rectangle(x + lw, y + 1.0, bw, 10.0, Color::new(0.15, 0.15, 0.2, 1.0));
    draw_rectangle(x + lw, y + 1.0, bw * frac, 10.0, rgb(color));
    draw_text_ex(text, x + lw + bw + 8.0, y + 10.0, tp(font, 13, TEXT));
}

/// What one status panel shows. `bars` are the animated values for health, stamina and
/// the class bar (shield or mana).
pub struct PanelView<'a> {
    pub fighter: &'a Fighter,
    pub bars: [f32; 3],
    pub accent: Vec3,
    pub active: bool,
    pub metal: Option<&'a str>,
}

pub const HEALTH: Vec3 = Vec3::new(0.35, 0.85, 0.4);
pub const STAMINA: Vec3 = Vec3::new(1.0, 0.78, 0.3);
pub const SHIELD: Vec3 = Vec3::new(0.4, 0.6, 1.0);
pub const MANA: Vec3 = Vec3::new(0.3, 0.85, 1.0);

/// Status panels, each on its fighter's side of the screen.
pub fn draw_panels(font: &Font, views: [PanelView; 2]) {
    let sw = screen_width();
    let w = (sw * 0.42).min(380.0);
    let h = 104.0;
    for (i, v) in views.iter().enumerate() {
        let x = if i == 0 { 12.0 } else { sw - w - 12.0 };
        let y = 28.0;
        let accent = rgb(v.accent);
        draw_rectangle(x, y, w, h, PANEL_BG);
        // Header strip in the player's colour.
        draw_rectangle(x + 4.0, y + 5.0, w - 8.0, 20.0, Color::new(v.accent.x, v.accent.y, v.accent.z, if v.active { 0.32 } else { 0.16 }));
        let frame = if v.active { accent } else { Color::new(0.35, 0.37, 0.45, 1.0) };
        draw_frame(font, x, y, w, h, frame);
        let f = v.fighter;
        let pad = x + 12.0;
        let inner = w - 24.0;
        let head = fit(&format!("{}  ·  {}", f.name, f.class.name()), chars_in(font, inner, 15));
        draw_text_ex(&head, pad, y + 20.0, tp(font, 15, TEXT));
        let bw = inner;
        let hp = fill(v.bars[0], f.max_health);
        bar(font, pad, y + 32.0, bw, "HP", hp, &format!("{}/{}", f.health, f.max_health), hp_color(hp, get_time() as f32));
        // Stamina has no cap: the bar fills at 100 and the number shows the real value.
        bar(font, pad, y + 48.0, bw, "ST", stamina_fill(v.bars[1]), &f.stamina.to_string(), STAMINA);
        let (third, potions) = match f.class {
            Class::Warrior => {
                bar(font, pad, y + 64.0, bw, "SH", fill(v.bars[2], f.max_shield), &format!("{}/{}", f.shield, f.max_shield), SHIELD);
                (v.metal.map(|m| format!("{m} armour")), format!("Potions {}", f.health_potions))
            }
            Class::Sorcerer => {
                bar(font, pad, y + 64.0, bw, "MP", fill(v.bars[2], f.max_mana), &format!("{}/{}", f.mana, f.max_mana), MANA);
                (None, format!("Potions {}  Mana potions {}", f.health_potions, f.mana_potions))
            }
            Class::Assassin => {
                // No class bar: the crit multiplier fills that row instead.
                draw_text_ex(format!("CR  Crit x{:.1}", f.crit), pad, y + 74.0, tp(font, 13, DIM));
                (None, format!("Potions {}", f.health_potions))
            }
        };
        let line = match third {
            Some(t) => format!("{potions}  ·  {t}"),
            None => potions,
        };
        draw_text_ex(fit(&line, chars_in(font, inner, 13)), pad, y + 92.0, tp(font, 13, DIM));
    }
}

/// Narration lines, centred, ending just above `bottom`.
pub fn draw_caption(font: &Font, lines: &[String], bottom: f32) {
    let cx = screen_width() * 0.5;
    // Wrap each narration line to the window, keeping the first line bright.
    let max = chars_in(font, screen_width() * 0.9, 14);
    let wrapped: Vec<(bool, String)> =
        lines.iter().enumerate().flat_map(|(i, l)| wrap(l, max).into_iter().map(move |w| (i == 0, w))).collect();
    let n = wrapped.len() as f32;
    for (i, (first, l)) in wrapped.iter().enumerate() {
        let y = bottom - (n - i as f32) * 18.0 + 13.0;
        let w = text_w(font, l, 14);
        draw_rectangle(cx - w * 0.5 - 6.0, y - 13.0, w + 12.0, 18.0, PANEL_BG);
        text_centered(font, l, cx, y, 14, if *first { TEXT } else { DIM });
    }
}

/// Result banner: the text between block-glyph bands in the winner's colour, then the
/// score and stats lines underneath. Everything is fitted to the window.
pub fn draw_banner(font: &Font, text: &str, color: Vec3, lines: &[String]) {
    let (sw, sh) = (screen_width(), screen_height());
    let size = 38;
    let text = fit(text, chars_in(font, sw - 80.0, size));
    let w = text_w(font, &text, size).max(lines.iter().map(|l| text_w(font, l, 14)).fold(0.0, f32::max)) + 48.0;
    let w = w.min(sw - 24.0);
    let x = sw * 0.5 - w * 0.5;
    let y = sh * 0.3;
    let h = 92.0 + lines.len() as f32 * 19.0;
    draw_rectangle(x, y - 46.0, w, h, Color::new(0.02, 0.02, 0.05, 0.92));
    let c = rgb(color);
    let band = |ch: &str, yy: f32| {
        let n = chars_in(font, w, 14);
        text_centered(font, &ch.repeat(n), sw * 0.5, yy, 14, c);
    };
    band("▄", y - 32.0);
    text_centered(font, &text, sw * 0.5, y + 6.0, size, c);
    band("▀", y + 26.0);
    for (i, l) in lines.iter().enumerate() {
        text_centered(font, &fit(l, chars_in(font, w - 20.0, 14)), sw * 0.5, y + 48.0 + i as f32 * 19.0, 14, if i == 0 { TEXT } else { DIM });
    }
}

/// Rows of block-glyph text, centred on `cx`, starting at `top`.
pub fn draw_block(font: &Font, rows: &[String; 3], cx: f32, top: f32, size: u16, color: Color) {
    // Step rows by the glyph's own height so the half-blocks join into solid letters.
    let m = measure_text("█", Some(font), size, 1.0);
    let step = m.height.max(1.0);
    for (r, row) in rows.iter().enumerate() {
        text_centered(font, row, cx, top + m.offset_y + r as f32 * step, size, color);
    }
}

/// The game's title in block letters.
pub fn draw_title(font: &Font, cx: f32, top: f32) {
    let rows = block_text("CLASH OF VALOR");
    let size = if screen_width() < 900.0 { 14 } else { 20 };
    draw_block(font, &rows, cx + 2.0, top + 2.0, size, Color::new(0.25, 0.12, 0.02, 0.9));
    draw_block(font, &rows, cx, top, size, Color::new(1.0, 0.78, 0.32, 1.0));
}

/// "Player N, enter your name" with the text field, centred at `y`.
/// A backed, framed plate for setup prompts.
pub fn draw_prompt_plate(font: &Font, title: &str, color: Vec3, y: f32, h: f32) {
    let cx = screen_width() * 0.5;
    let w = (text_w(font, title, FONT) + 60.0).max(text_w(font, &"M".repeat(NAME_MAX + 1), FONT) + 50.0);
    draw_rectangle(cx - w * 0.5, y - 22.0, w, h, PANEL_BG);
    draw_frame(font, cx - w * 0.5, y - 26.0, w, h + 4.0, Color::new(color.x, color.y, color.z, 0.8));
    text_centered(font, title, cx, y, FONT, rgb(color));
}

pub fn draw_name_prompt(font: &Font, title: &str, field: &NameField, color: Vec3, y: f32, cursor: bool) {
    let cx = screen_width() * 0.5;
    draw_prompt_plate(font, title, color, y, 66.0);
    let w = text_w(font, &"M".repeat(NAME_MAX + 1), FONT) + 16.0;
    draw_rectangle(cx - w * 0.5, y + 10.0, w, ROW + 4.0, PANEL_BG);
    draw_rectangle_lines(cx - w * 0.5, y + 10.0, w, ROW + 4.0, 1.0, DIM);
    let shown = if cursor { format!("{}_", field.text) } else { field.text.clone() };
    draw_text_ex(&shown, cx - w * 0.5 + 8.0, y + 28.0, tp(font, FONT, TEXT));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn menu(enabled: &[bool]) -> Menu {
        let n = enabled.len();
        Menu::new((0..n).map(|i| format!("item {i}")).collect(), vec![String::new(); n], enabled.to_vec())
    }

    #[test]
    fn menu_starts_on_first_enabled() {
        assert_eq!(menu(&[false, true, true]).sel, 1);
        assert_eq!(menu(&[true, true]).sel, 0);
    }

    #[test]
    fn menu_arrows_skip_disabled_and_wrap() {
        let mut m = menu(&[true, false, true]);
        assert_eq!(m.handle(Key::Down), None);
        assert_eq!(m.sel, 2);
        m.handle(Key::Down);
        assert_eq!(m.sel, 0);
        m.handle(Key::Up);
        assert_eq!(m.sel, 2);
    }

    #[test]
    fn menu_digit_confirms_enabled() {
        let mut m = menu(&[true, false, true]);
        assert_eq!(m.handle(Key::Digit(3)), Some(2));
        assert_eq!(m.sel, 2);
    }

    #[test]
    fn menu_digit_ignores_disabled_and_out_of_range() {
        let mut m = menu(&[true, false, true]);
        assert_eq!(m.handle(Key::Digit(2)), None);
        assert_eq!(m.sel, 0);
        assert_eq!(m.handle(Key::Digit(9)), None);
        assert_eq!(m.handle(Key::Digit(0)), None);
        assert_eq!(m.sel, 0);
    }

    #[test]
    fn menu_enter_confirms_sel() {
        let mut m = menu(&[false, true, true]);
        m.handle(Key::Down);
        assert_eq!(m.handle(Key::Enter), Some(2));
    }

    #[test]
    fn name_field_rules() {
        let mut f = NameField::default();
        f.backspace();
        f.push(' ');
        f.push('é');
        f.push('\n');
        assert_eq!(f.text, "");
        for _ in 0..20 {
            f.push('a');
        }
        assert_eq!(f.text.len(), NAME_MAX);
        f.backspace();
        assert_eq!(f.text.len(), NAME_MAX - 1);
        assert_eq!(NameField { text: "   ".into() }.accept(), None);
        assert_eq!(NameField { text: " Bo ".into() }.accept(), Some("Bo".to_string()));
        let mut g = NameField::default();
        g.push('B');
        g.push(' ');
        g.push('o');
        assert_eq!(g.text, "B o");
    }

    #[test]
    fn stamina_bar_fills_against_100() {
        assert_eq!(stamina_fill(125.0), 1.0, "over 100 stays full");
        assert_eq!(stamina_fill(100.0), 1.0);
        assert_eq!(stamina_fill(50.0), 0.5);
        assert_eq!(stamina_fill(-5.0), 0.0);
    }

    #[test]
    fn fit_truncates_with_ellipsis() {
        assert_eq!(fit("abcdef", 10), "abcdef");
        let f = fit("abcdefghijk", 6);
        assert_eq!(f.chars().count(), 6);
        assert!(f.ends_with('…'));
        assert_eq!(fit("ab", 1), "");
        assert_eq!(fit("ab", 2), "ab");
    }

    #[test]
    fn wrap_breaks_on_words() {
        assert_eq!(wrap("aa bb cc", 5), vec!["aa bb", "cc"]);
        assert_eq!(wrap("abcdefgh", 3), vec!["abc", "def", "gh"]);
        assert!(wrap("", 5).is_empty());
        assert!(wrap("one two three four five", 9).iter().all(|l| l.chars().count() <= 9));
    }

    #[test]
    fn move_cost_strings() {
        use crate::rules::Move::*;
        let w = Fighter::new("W", Class::Warrior);
        let a = Fighter::new("A", Class::Assassin);
        let s = Fighter::new("S", Class::Sorcerer);
        assert_eq!(move_cost(&w, Attack), "10 ST · 10 dmg");
        assert_eq!(move_cost(&w, RaiseShield), "15 ST · +25 shield");
        assert_eq!(move_cost(&a, CritAttack), "20 ST · 15 dmg");
        assert_eq!(move_cost(&s, FireOrb), "10 MP 10 ST · 12 dmg");
        assert_eq!(move_cost(&w, HealthPotion), "+50 HP · 2 left");
        assert_eq!(move_cost(&s, ManaPotion), "+50 MP · 3 left");
        assert_eq!(move_cost(&w, Rest), "+10 HP +25 ST");
        assert_eq!(move_cost(&s, Rest), "+10 HP +25 ST +20 MP");
        assert_eq!(move_cost(&a, Flee), "ends the duel");
    }

    #[test]
    fn hp_color_thresholds() {
        let g = hp_color(0.8, 0.0);
        let y = hp_color(0.4, 0.0);
        let r = hp_color(0.2, 0.0);
        assert!(g.y > g.x && g.y > g.z, "green");
        assert!(y.x > 0.8 && y.y > 0.5 && y.z < 0.5, "gold");
        assert!(r.x > r.y && r.x > r.z, "red");
        assert_ne!(hp_color(0.1, 0.0), hp_color(0.1, 0.25), "pulses when nearly dead");
        assert_eq!(hp_color(0.2, 0.0), hp_color(0.2, 0.25), "no pulse above 15%");
    }

    #[test]
    fn menu_flash_decays() {
        let mut m = menu(&[true, false]);
        m.flash_row(1);
        assert!(m.flash > 0.0 && m.flash_row == 1);
        m.tick(0.5);
        assert_eq!(m.flash, 0.0);
    }

    #[test]
    fn block_text_rows_equal_width() {
        let rows = block_text("CLASH OF VALOR");
        let w = 4 * "CLASH OF VALOR".chars().count() - 1;
        for r in &rows {
            assert_eq!(r.chars().count(), w, "{r:?}");
        }
    }

    #[test]
    fn block_text_known_letter() {
        assert_eq!(block_text("A"), ["▄▀▄".to_string(), "█▀█".to_string(), "▀ ▀".to_string()]);
        assert_eq!(block_text("a"), block_text("A"), "lowercase maps to capitals");
        assert_eq!(block_text("?")[0], "   ");
    }
}
