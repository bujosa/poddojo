//! The built-in terminal: a real interactive shell (bash on a pseudo-terminal) with the lab
//! environment, drawn cell by cell. `cd`, aliases and variables persist; colours, Ctrl-C, pasting,
//! selecting and copying, `vim`, `watch` and `kubectl exec -it` work like in any terminal.

use std::path::Path;
use std::time::Duration;

use egui::{Color32, CornerRadius, FontId, Margin, Pos2, Rect, RichText, Sense, Stroke, vec2};
use poddojo_runner::LabEnv;
use poddojo_runner::shell::{Session, vt100};

use crate::icons::{self, Icon};

/// The terminal keeps its own dark palette in both themes, like a real terminal.
const BG: Color32 = Color32::from_rgb(7, 10, 22);
const BAR: Color32 = Color32::from_rgb(14, 19, 40);
const BORDER: Color32 = Color32::from_rgb(36, 44, 82);
const TEXT: Color32 = Color32::from_rgb(203, 213, 235);
const DIM: Color32 = Color32::from_rgb(110, 122, 160);
const OK: Color32 = Color32::from_rgb(52, 211, 153);
const CURSOR: Color32 = Color32::from_rgb(45, 212, 191);
const SELECTION: Color32 = Color32::from_rgb(49, 64, 128);
const FONT_SIZE: f32 = 13.0;
const PAD: f32 = 10.0;

/// The 16 ANSI colours, tuned to PodDojo's palette.
const ANSI: [Color32; 16] = [
    Color32::from_rgb(30, 35, 56),
    Color32::from_rgb(248, 113, 113),
    Color32::from_rgb(52, 211, 153),
    Color32::from_rgb(251, 191, 36),
    Color32::from_rgb(96, 165, 250),
    Color32::from_rgb(192, 132, 252),
    Color32::from_rgb(45, 212, 191),
    Color32::from_rgb(203, 213, 235),
    Color32::from_rgb(98, 108, 145),
    Color32::from_rgb(252, 165, 165),
    Color32::from_rgb(110, 231, 183),
    Color32::from_rgb(253, 224, 71),
    Color32::from_rgb(147, 197, 253),
    Color32::from_rgb(216, 180, 254),
    Color32::from_rgb(94, 234, 212),
    Color32::from_rgb(255, 255, 255),
];

pub enum Action {
    /// Open the learner's own terminal app with the same environment.
    OpenExternal,
}

/// A cell on the visible screen: (row, column).
type Cell = (u16, u16);

#[derive(Default)]
pub struct Terminal {
    /// The bottom panel is shown (outside the lab and exam workspaces, which always show it).
    pub open: bool,
    /// Take the keyboard on the next frame.
    pub focus: bool,
    session: Option<Session>,
    error: Option<String>,
    /// Typed before the shell was up (Run buttons, `PODDOJO_RUN`): sent once it starts.
    pending: Vec<u8>,
    /// Selection anchor and head; normalised when used.
    selection: Option<(Cell, Cell)>,
    scroll_acc: f32,
    size: (u16, u16),
    /// The learner asked for a new shell after the last one exited.
    restart: bool,
}

impl Terminal {
    /// Type `cmd` and Enter into the shell, as if the learner did.
    pub fn run(&mut self, cmd: &str) {
        let mut bytes = cmd.trim_end().replace("\r\n", "\n").replace('\n', "\r").into_bytes();
        bytes.push(b'\r');
        self.send(bytes);
    }

    /// A full-screen program (vim, less, `watch`) has the terminal: typed commands would land in it.
    pub fn busy(&self) -> bool {
        self.session.as_ref().is_some_and(|s| s.alive() && s.screen().screen().alternate_screen())
    }

    /// Nothing printed for `quiet` (screenshots wait for commands to finish).
    pub fn settled(&self, quiet: Duration) -> bool {
        match &self.session {
            Some(s) => self.pending.is_empty() && s.last_output().elapsed() >= quiet,
            // Commands are waiting for the shell to start.
            None => self.pending.is_empty(),
        }
    }

    fn send(&mut self, bytes: Vec<u8>) {
        match &self.session {
            Some(s) if s.alive() && s.ready() && self.pending.is_empty() => {
                if s.screen().screen().scrollback() > 0 {
                    s.screen().screen_mut().set_scrollback(0);
                }
                s.write(bytes);
            }
            _ => self.pending.extend(bytes),
        }
    }

    /// Draw the terminal filling the available space. `chips` are one-click commands.
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        env: Option<&LabEnv>,
        machine: &str,
        home: &Path,
        chips: &[&str],
    ) -> Option<Action> {
        let mut action = None;
        egui::Frame::new().fill(BG).stroke(Stroke::new(1.0, BORDER)).corner_radius(CornerRadius::same(12)).show(
            ui,
            |ui| {
                ui.set_min_size(ui.available_size());
                ui.spacing_mut().item_spacing.y = 0.0;
                action = self.title_bar(ui, env, machine);
                let chips_h = if chips.is_empty() { 0.0 } else { 40.0 };
                let grid =
                    Rect::from_min_max(ui.cursor().min, egui::pos2(ui.max_rect().max.x, ui.max_rect().max.y - chips_h));
                ui.allocate_rect(grid, Sense::hover());
                self.screen_ui(ui, grid, env, machine, home);
                if !chips.is_empty() {
                    self.chips(ui, chips);
                }
            },
        );
        action
    }

    fn title_bar(&mut self, ui: &mut egui::Ui, env: Option<&LabEnv>, machine: &str) -> Option<Action> {
        let mut action = None;
        egui::Frame::new()
            .fill(BAR)
            .corner_radius(CornerRadius { nw: 12, ne: 12, sw: 0, se: 0 })
            .inner_margin(Margin::symmetric(12, 7))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    for c in [Color32::from_rgb(248, 113, 113), Color32::from_rgb(251, 191, 36), OK] {
                        let (r, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
                        ui.painter().circle_filled(r.center(), 4.5, c.gamma_multiply(0.85));
                    }
                    ui.add_space(6.0);
                    icons::show(ui, Icon::Terminal, 14.0, DIM);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        if tiny(ui, "External")
                            .on_hover_text("Open your own terminal app with the same environment (⌘T)")
                            .clicked()
                        {
                            action = Some(Action::OpenExternal);
                        }
                        if tiny(ui, "Restart").on_hover_text("Start a fresh shell in the lab folder").clicked() {
                            self.session = None;
                            self.error = None;
                            self.focus = true;
                        }
                        if tiny(ui, "Clear").on_hover_text("Clear the screen and the scrollback").clicked() {
                            if let Some(s) = &self.session {
                                s.screen().screen_mut().clear_scrollback();
                            }
                            self.send(b"\x0c".to_vec());
                            self.focus = true;
                        }
                        if tiny(ui, "Ctrl-C").on_hover_text("Interrupt the running command").clicked() {
                            self.send(b"\x03".to_vec());
                            self.focus = true;
                        }
                        if let Some(e) = env
                            && tiny(ui, "Copy env")
                                .on_hover_text("Shell exports to paste in your own terminal")
                                .clicked()
                        {
                            ui.ctx().copy_text(e.export_script());
                        }
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                            let title = self
                                .session
                                .as_ref()
                                .map(|s| s.screen().callbacks().title.clone())
                                .filter(|t| !t.is_empty());
                            let label = match title {
                                Some(t) => format!("{machine} · {t}"),
                                None => format!("{machine} · kind-poddojo"),
                            };
                            ui.label(RichText::new(label).size(12.5).color(DIM));
                        });
                    });
                });
            });
        action
    }

    fn chips(&mut self, ui: &mut egui::Ui, chips: &[&str]) {
        egui::Frame::new().inner_margin(Margin { left: 12, right: 12, top: 6, bottom: 8 }).show(ui, |ui| {
            egui::ScrollArea::horizontal().id_salt("terminal-chips").show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = vec2(6.0, 4.0);
                    for c in chips {
                        let resp = ui.add(
                            egui::Button::new(RichText::new(*c).monospace().size(11.5).color(TEXT))
                                .fill(BAR)
                                .stroke(Stroke::new(1.0, BORDER))
                                .corner_radius(CornerRadius::same(8)),
                        );
                        if resp.on_hover_text("Type it in the terminal and run it").clicked() {
                            if self.busy() {
                                self.focus = true;
                            } else {
                                self.run(c);
                                self.focus = true;
                            }
                        }
                    }
                });
            });
        });
    }

    /// (Re)start the shell when there is none, it exited and the learner asked for a new one, or
    /// the machine or lab folder changed.
    fn ensure_session(&mut self, ctx: &egui::Context, env: Option<&LabEnv>, machine: &str, home: &Path) {
        let Some(env) = env else {
            self.session = None;
            return;
        };
        let stale = self.session.as_ref().is_some_and(|s| &s.env != env || (!s.alive() && self.restart));
        if stale {
            self.session = None;
            self.selection = None;
        }
        self.restart = false;
        // After a failure, wait for Restart instead of retrying every frame.
        if self.session.is_some() || self.error.is_some() {
            return;
        }
        let waker = ctx.clone();
        match Session::start(env, machine, home, self.size, move || waker.request_repaint()) {
            Ok(s) => {
                self.session = Some(s);
                self.error = None;
            }
            Err(e) => self.error = Some(e),
        }
    }

    fn screen_ui(&mut self, ui: &mut egui::Ui, rect: Rect, env: Option<&LabEnv>, machine: &str, home: &Path) {
        let font = FontId::monospace(FONT_SIZE);
        let (cw, ch) = ui.fonts_mut(|f| (f.glyph_width(&font, 'M'), f.row_height(&font)));
        let inner = rect.shrink2(vec2(PAD, PAD * 0.6));
        let cols = ((inner.width() / cw).floor() as u16).max(10);
        let rows = ((inner.height() / ch).floor() as u16).max(2);
        let origin = inner.min;
        self.size = (rows, cols);
        if env.is_none() {
            ui.painter().text(
                inner.min,
                egui::Align2::LEFT_TOP,
                "No machine with Docker yet: pick one on the Setup page.",
                font,
                DIM,
            );
            return;
        }
        self.ensure_session(ui.ctx(), env, machine, home);
        if let Some(e) = &self.error {
            ui.painter().text(inner.min, egui::Align2::LEFT_TOP, e, font, ANSI[1]);
            return;
        }
        let Some(session) = &self.session else { return };
        // Typed-ahead commands go in once the shell is reading (no garbled echo before the prompt).
        if session.ready() && !self.pending.is_empty() {
            session.write(std::mem::take(&mut self.pending));
        }
        let (cur_rows, cur_cols) = {
            let p = session.screen();
            let (r, c) = p.screen().size();
            (r.get(), c.get())
        };
        if (cur_rows, cur_cols) != (rows, cols) {
            session.resize(rows, cols);
            self.selection = None;
        }

        let id = ui.id().with("terminal-screen");
        let resp = ui.interact(rect, id, Sense::click_and_drag()).on_hover_cursor(egui::CursorIcon::Text);
        if std::mem::take(&mut self.focus) || resp.clicked() || resp.drag_started() {
            resp.request_focus();
        }
        let focused = resp.has_focus();
        let cell_at = |p: Pos2| -> Cell {
            let r = ((p.y - origin.y) / ch).floor().clamp(0.0, f32::from(rows - 1)) as u16;
            let c = ((p.x - origin.x) / cw).floor().clamp(0.0, f32::from(cols - 1)) as u16;
            (r, c)
        };

        // Selecting: drag, double-click a word.
        if resp.drag_started()
            && let Some(p) = resp.interact_pointer_pos()
        {
            let c = cell_at(p);
            self.selection = Some((c, c));
        } else if resp.dragged()
            && let (Some(p), Some((anchor, _))) = (resp.interact_pointer_pos(), self.selection)
        {
            self.selection = Some((anchor, cell_at(p)));
        } else if resp.double_clicked()
            && let Some(p) = resp.interact_pointer_pos()
        {
            self.selection = word_at(&session.screen(), cell_at(p));
        } else if resp.clicked() {
            self.selection = None;
        }

        // Scrolling: back through history, or arrow keys for full-screen programs.
        if resp.hovered() {
            let dy = ui.input(|i| i.smooth_scroll_delta.y);
            if dy != 0.0 {
                self.scroll_acc += dy / ch;
                let lines = self.scroll_acc.trunc() as i32;
                if lines != 0 {
                    self.scroll_acc -= lines as f32;
                    let mut p = session.screen();
                    if p.screen().alternate_screen() {
                        let key: &[u8] = match (lines > 0, p.screen().application_cursor()) {
                            (true, true) => b"\x1bOA",
                            (true, false) => b"\x1b[A",
                            (false, true) => b"\x1bOB",
                            (false, false) => b"\x1b[B",
                        };
                        session.write(key.repeat(lines.unsigned_abs() as usize));
                    } else {
                        let back = (p.screen().scrollback() as i64 + i64::from(lines)).max(0) as usize;
                        p.screen_mut().set_scrollback(back);
                        self.selection = None;
                    }
                }
            }
        }

        if focused {
            ui.memory_mut(|m| {
                m.set_focus_lock_filter(
                    id,
                    egui::EventFilter { tab: true, horizontal_arrows: true, vertical_arrows: true, escape: true },
                );
            });
            let out = self.keyboard(ui, session);
            if !out.is_empty() {
                self.selection = None;
                let mut p = session.screen();
                if p.screen().scrollback() > 0 {
                    p.screen_mut().set_scrollback(0);
                }
                drop(p);
                session.write(out);
            }
        }

        resp.context_menu(|ui| {
            if ui.add_enabled(self.selection.is_some(), egui::Button::new("Copy")).clicked() {
                if let Some(text) = selected_text(&session.screen(), self.selection) {
                    ui.ctx().copy_text(text);
                }
                ui.close();
            }
            if ui.button("Paste").clicked() {
                resp.request_focus();
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::RequestPaste);
                ui.close();
            }
            if ui.button("Select all").clicked() {
                self.selection = Some(((0, 0), (rows - 1, cols - 1)));
                ui.close();
            }
        });

        paint(ui, &session.screen(), origin, (cw, ch), (rows, cols), self.selection, focused);
        if !session.alive() {
            let r = Rect::from_min_size(egui::pos2(rect.min.x, rect.max.y - 30.0), vec2(rect.width(), 30.0));
            ui.painter().rect_filled(r, 0.0, BAR);
            ui.painter().text(
                r.center(),
                egui::Align2::CENTER_CENTER,
                "The shell exited. Press Enter for a new one.",
                FontId::proportional(12.5),
                TEXT,
            );
            if focused && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                self.restart = true;
                ui.ctx().request_repaint();
            }
        }
    }

    /// Turn this frame's keyboard events into bytes for the shell (and copy/paste).
    fn keyboard(&self, ui: &egui::Ui, session: &Session) -> Vec<u8> {
        let (app_cursor, bracketed) = {
            let p = session.screen();
            (p.screen().application_cursor(), p.screen().bracketed_paste())
        };
        let events = ui.input(|i| i.events.clone());
        let mut out = Vec::new();
        for e in events {
            match e {
                egui::Event::Text(t) => out.extend_from_slice(t.as_bytes()),
                egui::Event::Paste(t) => {
                    let t = t.replace("\r\n", "\n").replace('\n', "\r").replace('\x1b', "");
                    if bracketed {
                        out.extend_from_slice(b"\x1b[200~");
                        out.extend_from_slice(t.as_bytes());
                        out.extend_from_slice(b"\x1b[201~");
                    } else {
                        out.extend_from_slice(t.as_bytes());
                    }
                }
                // ⌘C on macOS; Ctrl-C elsewhere arrives as Copy too: copy a selection, else interrupt.
                egui::Event::Copy | egui::Event::Cut => {
                    if let Some(text) = selected_text(&session.screen(), self.selection) {
                        ui.ctx().copy_text(text);
                    } else if !cfg!(target_os = "macos") {
                        out.push(if matches!(e, egui::Event::Copy) { 0x03 } else { 0x18 });
                    }
                }
                egui::Event::Key { key, pressed: true, modifiers, .. } => {
                    if let Some(bytes) = key_bytes(key, modifiers, app_cursor) {
                        out.extend_from_slice(&bytes);
                    }
                }
                _ => {}
            }
        }
        out
    }
}

/// The bytes a terminal sends for a key that does not produce text.
fn key_bytes(key: egui::Key, m: egui::Modifiers, app_cursor: bool) -> Option<Vec<u8>> {
    use egui::Key as K;
    // ⌘-shortcuts belong to the app.
    if m.mac_cmd {
        return None;
    }
    let csi = |s: &str| format!("\x1b[{s}").into_bytes();
    let arrow = |c: char| {
        if m.ctrl {
            csi(&format!("1;5{c}"))
        } else if m.shift {
            csi(&format!("1;2{c}"))
        } else if app_cursor {
            format!("\x1bO{c}").into_bytes()
        } else {
            csi(&c.to_string())
        }
    };
    if m.ctrl && !m.alt {
        let name = key.name();
        if name.len() == 1
            && let Some(c) = name.chars().next().filter(char::is_ascii_alphabetic)
        {
            return Some(vec![(c.to_ascii_lowercase() as u8) & 0x1f]);
        }
        match key {
            K::OpenBracket => return Some(vec![0x1b]),
            K::Backslash => return Some(vec![0x1c]),
            K::CloseBracket => return Some(vec![0x1d]),
            K::Space => return Some(vec![0]),
            _ => {}
        }
    }
    Some(match key {
        K::Enter => b"\r".to_vec(),
        K::Tab if m.shift => csi("Z"),
        K::Tab => b"\t".to_vec(),
        K::Backspace if m.alt => b"\x1b\x7f".to_vec(),
        K::Backspace => b"\x7f".to_vec(),
        K::Escape => b"\x1b".to_vec(),
        // ⌥← / ⌥→ jump words, like macOS terminals set up for readline.
        K::ArrowLeft if m.alt => b"\x1bb".to_vec(),
        K::ArrowRight if m.alt => b"\x1bf".to_vec(),
        K::ArrowUp => arrow('A'),
        K::ArrowDown => arrow('B'),
        K::ArrowRight => arrow('C'),
        K::ArrowLeft => arrow('D'),
        K::Home if app_cursor => b"\x1bOH".to_vec(),
        K::End if app_cursor => b"\x1bOF".to_vec(),
        K::Home => csi("H"),
        K::End => csi("F"),
        K::Insert => csi("2~"),
        K::Delete => csi("3~"),
        K::PageUp => csi("5~"),
        K::PageDown => csi("6~"),
        K::F1 => b"\x1bOP".to_vec(),
        K::F2 => b"\x1bOQ".to_vec(),
        K::F3 => b"\x1bOR".to_vec(),
        K::F4 => b"\x1bOS".to_vec(),
        K::F5 => csi("15~"),
        K::F6 => csi("17~"),
        K::F7 => csi("18~"),
        K::F8 => csi("19~"),
        K::F9 => csi("20~"),
        K::F10 => csi("21~"),
        K::F11 => csi("23~"),
        K::F12 => csi("24~"),
        _ => return None,
    })
}

fn ordered(sel: (Cell, Cell)) -> (Cell, Cell) {
    if sel.0 <= sel.1 { sel } else { (sel.1, sel.0) }
}

fn selected_text(p: &poddojo_runner::shell::Screen, sel: Option<(Cell, Cell)>) -> Option<String> {
    let (a, b) = ordered(sel?);
    let text = p.screen().contents_between(a.0, a.1, b.0, b.1 + 1);
    let text: Vec<&str> = text.lines().map(str::trim_end).collect();
    let text = text.join("\n");
    (!text.is_empty()).then_some(text)
}

fn word_at(p: &poddojo_runner::shell::Screen, (row, col): Cell) -> Option<(Cell, Cell)> {
    let s = p.screen();
    let is_word = |c: u16| {
        s.cell(row, c).is_some_and(|cell| {
            let t = cell.contents();
            !t.is_empty() && !t.chars().all(char::is_whitespace) && !"\"'`()[]{}<>|;,".contains(t)
        })
    };
    if !is_word(col) {
        return None;
    }
    let (_, cols) = s.size();
    let (mut a, mut b) = (col, col);
    while a > 0 && is_word(a - 1) {
        a -= 1;
    }
    while b + 1 < cols.get() && is_word(b + 1) {
        b += 1;
    }
    Some(((row, a), (row, b)))
}

fn color(c: vt100::Color, default: Color32) -> Color32 {
    match c {
        vt100::Color::Default => default,
        vt100::Color::Idx(i) => indexed(i),
        vt100::Color::Rgb(r, g, b) => Color32::from_rgb(r, g, b),
    }
}

/// xterm's 256 colours: 16 ANSI, a 6×6×6 cube, 24 greys.
fn indexed(i: u8) -> Color32 {
    match i {
        0..16 => ANSI.get(usize::from(i)).copied().unwrap_or(TEXT),
        16..232 => {
            let i = i - 16;
            let level = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
            Color32::from_rgb(level(i / 36), level((i / 6) % 6), level(i % 6))
        }
        _ => {
            let g = 8 + (i - 232) * 10;
            Color32::from_rgb(g, g, g)
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
struct Style {
    fg: Color32,
    bg: Color32,
    bold: bool,
    italic: bool,
    underline: bool,
}

fn style(cell: &vt100::Cell, selected: bool) -> Style {
    let mut fg = color(cell.fgcolor(), TEXT);
    // Bold ANSI colours are the bright ones, as in most terminals.
    if cell.bold()
        && let vt100::Color::Idx(i @ 0..8) = cell.fgcolor()
    {
        fg = indexed(i + 8);
    }
    let mut bg = color(cell.bgcolor(), BG);
    if cell.inverse() {
        std::mem::swap(&mut fg, &mut bg);
    }
    if cell.dim() {
        fg = fg.gamma_multiply(0.6);
    }
    if selected {
        bg = SELECTION;
    }
    Style { fg, bg, bold: cell.bold(), italic: cell.italic(), underline: cell.underline() }
}

fn paint(
    ui: &egui::Ui,
    p: &poddojo_runner::shell::Screen,
    origin: Pos2,
    (cw, ch): (f32, f32),
    (rows, cols): (u16, u16),
    selection: Option<(Cell, Cell)>,
    focused: bool,
) {
    let s = p.screen();
    let painter = ui.painter();
    let font = FontId::monospace(FONT_SIZE);
    let sel = selection.map(ordered);
    let in_sel = |r: u16, c: u16| sel.is_some_and(|(a, b)| (r, c) >= a && (r, c) <= b);
    let cell_rect = |r: u16, c: u16, w: u16| {
        Rect::from_min_size(origin + vec2(f32::from(c) * cw, f32::from(r) * ch), vec2(f32::from(w) * cw, ch))
    };
    let draw = |text: &str, st: Style, r: u16, c: u16| {
        let mut job = egui::text::LayoutJob::default();
        job.append(
            text,
            0.0,
            egui::TextFormat {
                font_id: font.clone(),
                color: st.fg,
                italics: st.italic,
                underline: if st.underline { Stroke::new(1.0, st.fg) } else { Stroke::NONE },
                ..Default::default()
            },
        );
        let galley = painter.layout_job(job);
        let pos = cell_rect(r, c, 1).min;
        if st.bold {
            painter.galley(pos + vec2(0.6, 0.0), galley.clone(), st.fg);
        }
        painter.galley(pos, galley, st.fg);
    };
    for r in 0..rows {
        let mut c = 0;
        while c < cols {
            let Some(cell) = s.cell(r, c) else { break };
            let st = style(cell, in_sel(r, c));
            // Wide and non-ASCII characters get their own run so columns never drift.
            let simple = |cell: &vt100::Cell| !cell.is_wide() && cell.contents().is_ascii();
            let start = c;
            let mut text = String::new();
            if simple(cell) {
                while c < cols {
                    let Some(next) = s.cell(r, c) else { break };
                    if !simple(next) || style(next, in_sel(r, c)) != st {
                        break;
                    }
                    let t = next.contents();
                    text.push_str(if t.is_empty() { " " } else { t });
                    c += 1;
                }
            } else {
                text.push_str(cell.contents());
                c += if cell.is_wide() { 2 } else { 1 };
            }
            let width = c.saturating_sub(start).max(1);
            if st.bg != BG {
                painter.rect_filled(cell_rect(r, start, width), 0.0, st.bg);
            }
            if !text.trim().is_empty() {
                draw(&text, st, r, start);
            }
        }
    }
    // The cursor: a block while typing here, an outline otherwise. Hidden while scrolled back.
    if s.scrollback() == 0 && !s.hide_cursor() {
        let (r, c) = s.cursor_position();
        if r < rows && c < cols {
            let rect = cell_rect(r, c, 1);
            if focused {
                painter.rect_filled(rect, 1.0, CURSOR);
                if let Some(cell) = s.cell(r, c).filter(|cell| cell.has_contents()) {
                    let st = Style { fg: BG, ..style(cell, false) };
                    draw(cell.contents(), st, r, c);
                }
            } else {
                painter.rect_stroke(rect, 1.0, Stroke::new(1.0, CURSOR.gamma_multiply(0.7)), egui::StrokeKind::Inside);
            }
        }
    }
    if s.scrollback() > 0 {
        let label = format!("↑ {} lines back · type to return", s.scrollback());
        let pos = origin + vec2(f32::from(cols) * cw, 0.0);
        let galley = painter.layout_no_wrap(label, FontId::proportional(11.5), Color32::WHITE);
        let r = Rect::from_min_size(pos - vec2(galley.size().x + 12.0, 0.0), galley.size() + vec2(12.0, 6.0));
        painter.rect_filled(r, 6.0, SELECTION);
        painter.galley(r.min + vec2(6.0, 3.0), galley, Color32::WHITE);
    }
}

fn tiny(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(text).size(11.5).color(TEXT))
            .fill(Color32::from_rgb(24, 30, 58))
            .stroke(Stroke::new(1.0, BORDER))
            .corner_radius(CornerRadius::same(7)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_keys() {
        let ctrl = egui::Modifiers { ctrl: true, ..Default::default() };
        assert_eq!(key_bytes(egui::Key::C, ctrl, false), Some(vec![3]));
        assert_eq!(key_bytes(egui::Key::D, ctrl, false), Some(vec![4]));
        assert_eq!(key_bytes(egui::Key::Enter, egui::Modifiers::NONE, false), Some(b"\r".to_vec()));
        assert_eq!(key_bytes(egui::Key::ArrowUp, egui::Modifiers::NONE, true), Some(b"\x1bOA".to_vec()));
        assert_eq!(key_bytes(egui::Key::ArrowUp, egui::Modifiers::NONE, false), Some(b"\x1b[A".to_vec()));
        // Plain letters arrive as text, not keys.
        assert_eq!(key_bytes(egui::Key::A, egui::Modifiers::NONE, false), None);
    }

    #[test]
    fn palette() {
        assert_eq!(indexed(196), Color32::from_rgb(255, 0, 0));
        assert_eq!(indexed(232), Color32::from_rgb(8, 8, 8));
        assert_eq!(indexed(1), ANSI[1]);
    }
}
