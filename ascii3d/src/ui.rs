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
}

impl Menu {
    pub fn new(items: Vec<String>, notes: Vec<String>, enabled: Vec<bool>) -> Menu {
        let sel = enabled.iter().position(|&e| e).unwrap_or(0);
        Menu { items, notes, enabled, sel }
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

pub fn menu_height(m: &Menu) -> f32 {
    m.items.len() as f32 * ROW
}

/// The option list as a block centred on `cx`. The selected row is drawn in reverse
/// video; disabled rows are dimmed and show their note.
pub fn draw_menu(font: &Font, m: &Menu, cx: f32, top: f32) {
    let rows: Vec<(String, String)> = m
        .items
        .iter()
        .enumerate()
        .map(|(i, it)| (format!(" {} {it} ", i + 1), m.notes.get(i).cloned().unwrap_or_default()))
        .collect();
    let label_w = rows.iter().map(|(l, _)| text_w(font, l, FONT)).fold(0.0, f32::max);
    let note_w = rows.iter().map(|(_, n)| if n.is_empty() { 0.0 } else { text_w(font, n, 13) + 14.0 }).fold(0.0, f32::max);
    let x = (cx - (label_w + note_w) * 0.5).max(6.0);
    draw_rectangle(x - 8.0, top - 4.0, label_w + note_w + 16.0, menu_height(m) + 8.0, PANEL_BG);
    for (i, (label, note)) in rows.iter().enumerate() {
        let y = top + i as f32 * ROW;
        let on = m.enabled[i];
        if i == m.sel && on {
            draw_rectangle(x, y, label_w, ROW - 2.0, TEXT);
            draw_text_ex(label, x, y + 15.0, tp(font, FONT, Color::new(0.03, 0.03, 0.07, 1.0)));
        } else {
            draw_text_ex(label, x, y + 15.0, tp(font, FONT, if on { TEXT } else { Color::new(0.4, 0.4, 0.46, 1.0) }));
        }
        if !note.is_empty() {
            draw_text_ex(note, x + label_w + 14.0, y + 15.0, tp(font, 13, DIM));
        }
    }
}

fn bar(font: &Font, x: f32, y: f32, w: f32, label: &str, shown: f32, value: i32, max: i32, color: Vec3) {
    let lw = 30.0;
    draw_text_ex(label, x, y + 10.0, tp(font, 13, DIM));
    let bw = w - lw - 70.0;
    draw_rectangle(x + lw, y + 1.0, bw, 10.0, Color::new(0.15, 0.15, 0.2, 1.0));
    let f = if max > 0 { (shown / max as f32).clamp(0.0, 1.0) } else { 0.0 };
    draw_rectangle(x + lw, y + 1.0, bw * f, 10.0, rgb(color));
    draw_text_ex(&format!("{value}/{max}"), x + lw + bw + 8.0, y + 10.0, tp(font, 13, TEXT));
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
    let h = 96.0;
    for (i, v) in views.iter().enumerate() {
        let x = if i == 0 { 12.0 } else { sw - w - 12.0 };
        let y = 30.0;
        draw_rectangle(x, y, w, h, PANEL_BG);
        if v.active {
            draw_rectangle_lines(x, y, w, h, 2.0, rgb(v.accent));
        }
        let f = v.fighter;
        let pad = x + 10.0;
        draw_text_ex(&f.name, pad, y + 18.0, tp(font, FONT, rgb(v.accent)));
        let cls = match v.metal {
            Some(m) => format!("{}  ·  {m} armour", f.class.name()),
            None => f.class.name().to_string(),
        };
        draw_text_ex(&cls, pad + text_w(font, &f.name, FONT) + 10.0, y + 18.0, tp(font, 13, DIM));
        let bw = w - 20.0;
        bar(font, pad, y + 26.0, bw, "HP", v.bars[0], f.health, f.max_health, HEALTH);
        // Stamina has no cap: the bar fills at 100 and the number shows the real value.
        bar(font, pad, y + 42.0, bw, "ST", v.bars[1].min(100.0), f.stamina, f.stamina.max(100), STAMINA);
        let potions = match f.class {
            Class::Warrior => {
                bar(font, pad, y + 58.0, bw, "SH", v.bars[2], f.shield, f.max_shield, SHIELD);
                format!("Potions {}", f.health_potions)
            }
            Class::Sorcerer => {
                bar(font, pad, y + 58.0, bw, "MP", v.bars[2], f.mana, f.max_mana, MANA);
                format!("Potions {}   Mana potions {}", f.health_potions, f.mana_potions)
            }
            Class::Assassin => {
                draw_text_ex(&format!("Crit x{:.1}", f.crit), pad, y + 68.0, tp(font, 13, DIM));
                format!("Potions {}", f.health_potions)
            }
        };
        draw_text_ex(&potions, pad, y + 88.0, tp(font, 13, DIM));
    }
}

/// Narration lines, centred, ending just above `bottom`.
pub fn draw_caption(font: &Font, lines: &[String], bottom: f32) {
    let cx = screen_width() * 0.5;
    let n = lines.len() as f32;
    for (i, l) in lines.iter().enumerate() {
        let y = bottom - (n - i as f32) * 18.0 + 13.0;
        let w = text_w(font, l, 14);
        draw_rectangle(cx - w * 0.5 - 6.0, y - 13.0, w + 12.0, 18.0, PANEL_BG);
        text_centered(font, l, cx, y, 14, if i == 0 { TEXT } else { DIM });
    }
}

pub fn draw_banner(font: &Font, text: &str, color: Vec3) {
    let (sw, sh) = (screen_width(), screen_height());
    let size = 40;
    let w = text_w(font, text, size);
    let y = sh * 0.36;
    draw_rectangle(sw * 0.5 - w * 0.5 - 24.0, y - 40.0, w + 48.0, 58.0, PANEL_BG);
    text_centered(font, text, sw * 0.5, y, size, rgb(color));
}

/// "Player N, enter your name" with the text field, centred at `y`.
pub fn draw_name_prompt(font: &Font, title: &str, field: &NameField, color: Vec3, y: f32, cursor: bool) {
    let cx = screen_width() * 0.5;
    text_centered(font, title, cx, y, FONT, rgb(color));
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
}
