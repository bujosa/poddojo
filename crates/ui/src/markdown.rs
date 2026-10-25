//! A small Markdown renderer for lessons: headings, paragraphs with inline styles, lists, quotes,
//! tables and code blocks with Copy and Run buttons. Parsed with pulldown-cmark into blocks, then
//! drawn with egui.

use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, CornerRadius, FontFamily, FontId, Margin, RichText, Stroke};
use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use crate::theme::Tokens;

#[derive(Debug, Clone, Default, PartialEq)]
struct Span {
    text: String,
    bold: bool,
    italic: bool,
    code: bool,
    link: bool,
    strike: bool,
}

#[derive(Debug, Clone, PartialEq)]
enum Block {
    Heading(u8, Vec<Span>),
    Para { spans: Vec<Span>, indent: usize, bullet: Option<String>, quote: bool },
    Code { lang: String, text: String },
    Table { head: Vec<Vec<Span>>, rows: Vec<Vec<Vec<Span>>> },
    Rule,
}

type Cells = Vec<Vec<Span>>;

/// What the reader asked for by clicking inside the Markdown.
pub enum Action {
    /// Run this shell snippet in the console.
    Run(String),
}

fn parse(md: &str) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut spans: Vec<Span> = Vec::new();
    let (mut bold, mut italic, mut link, mut strike) = (0usize, 0usize, 0usize, 0usize);
    let mut heading: Option<u8> = None;
    let mut lists: Vec<Option<u64>> = Vec::new();
    let mut pending_bullet: Option<String> = None;
    let mut quote = 0usize;
    let mut code: Option<(String, String)> = None;
    let mut table: Option<(Cells, Vec<Cells>, bool)> = None;
    let mut row: Vec<Vec<Span>> = Vec::new();

    let flush = |spans: &mut Vec<Span>,
                 blocks: &mut Vec<Block>,
                 lists: &Vec<Option<u64>>,
                 bullet: &mut Option<String>,
                 quote: usize| {
        if spans.iter().all(|s| s.text.trim().is_empty()) {
            spans.clear();
            return;
        }
        if let Some(first) = spans.first_mut() {
            first.text = first.text.trim_start().to_string();
        }
        blocks.push(Block::Para {
            spans: std::mem::take(spans),
            indent: lists.len(),
            bullet: bullet.take(),
            quote: quote > 0,
        });
    };

    let opts = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH;
    for ev in Parser::new_ext(md, opts) {
        match ev {
            Event::Start(tag) => match tag {
                Tag::Heading { level, .. } => {
                    flush(&mut spans, &mut blocks, &lists, &mut pending_bullet, quote);
                    heading = Some(match level {
                        HeadingLevel::H1 => 1,
                        HeadingLevel::H2 => 2,
                        HeadingLevel::H3 => 3,
                        _ => 4,
                    });
                }
                Tag::Paragraph => {}
                Tag::List(start) => {
                    flush(&mut spans, &mut blocks, &lists, &mut pending_bullet, quote);
                    lists.push(start);
                }
                Tag::Item => {
                    flush(&mut spans, &mut blocks, &lists, &mut pending_bullet, quote);
                    pending_bullet = Some(match lists.last_mut() {
                        Some(Some(n)) => {
                            let b = format!("{n}.");
                            *n += 1;
                            b
                        }
                        _ => "•".to_string(),
                    });
                }
                Tag::BlockQuote(_) => {
                    flush(&mut spans, &mut blocks, &lists, &mut pending_bullet, quote);
                    quote += 1;
                }
                Tag::CodeBlock(kind) => {
                    flush(&mut spans, &mut blocks, &lists, &mut pending_bullet, quote);
                    let lang = match kind {
                        CodeBlockKind::Fenced(l) => l.split_whitespace().next().unwrap_or("").to_string(),
                        CodeBlockKind::Indented => String::new(),
                    };
                    code = Some((lang, String::new()));
                }
                Tag::Table(_) => {
                    flush(&mut spans, &mut blocks, &lists, &mut pending_bullet, quote);
                    table = Some((Vec::new(), Vec::new(), true));
                }
                Tag::TableHead | Tag::TableRow => row.clear(),
                Tag::TableCell => spans.clear(),
                Tag::Strong => bold += 1,
                Tag::Emphasis => italic += 1,
                Tag::Strikethrough => strike += 1,
                Tag::Link { .. } => link += 1,
                _ => {}
            },
            Event::End(tag) => match tag {
                TagEnd::Heading(_) => {
                    let level = heading.take().unwrap_or(2);
                    blocks.push(Block::Heading(level, std::mem::take(&mut spans)));
                }
                TagEnd::Paragraph => flush(&mut spans, &mut blocks, &lists, &mut pending_bullet, quote),
                TagEnd::Item => flush(&mut spans, &mut blocks, &lists, &mut pending_bullet, quote),
                TagEnd::List(_) => {
                    flush(&mut spans, &mut blocks, &lists, &mut pending_bullet, quote);
                    lists.pop();
                }
                TagEnd::BlockQuote(_) => {
                    flush(&mut spans, &mut blocks, &lists, &mut pending_bullet, quote);
                    quote = quote.saturating_sub(1);
                }
                TagEnd::CodeBlock => {
                    if let Some((lang, text)) = code.take() {
                        blocks.push(Block::Code { lang, text: text.trim_end().to_string() });
                    }
                }
                TagEnd::TableCell => row.push(std::mem::take(&mut spans)),
                TagEnd::TableHead => {
                    if let Some((head, _, in_head)) = &mut table {
                        *head = std::mem::take(&mut row);
                        *in_head = false;
                    }
                }
                TagEnd::TableRow => {
                    if let Some((_, rows, _)) = &mut table {
                        rows.push(std::mem::take(&mut row));
                    }
                }
                TagEnd::Table => {
                    if let Some((head, rows, _)) = table.take() {
                        blocks.push(Block::Table { head, rows });
                    }
                }
                TagEnd::Strong => bold = bold.saturating_sub(1),
                TagEnd::Emphasis => italic = italic.saturating_sub(1),
                TagEnd::Strikethrough => strike = strike.saturating_sub(1),
                TagEnd::Link => link = link.saturating_sub(1),
                _ => {}
            },
            Event::Text(text) => {
                if let Some((_, buf)) = &mut code {
                    buf.push_str(&text);
                } else {
                    spans.push(Span {
                        text: text.to_string(),
                        bold: bold > 0,
                        italic: italic > 0,
                        code: false,
                        link: link > 0,
                        strike: strike > 0,
                    });
                }
            }
            Event::Code(text) => {
                spans.push(Span { text: text.to_string(), bold: bold > 0, code: true, ..Default::default() })
            }
            Event::SoftBreak => spans.push(Span { text: " ".into(), ..Default::default() }),
            Event::HardBreak => spans.push(Span { text: "\n".into(), ..Default::default() }),
            Event::Rule => {
                flush(&mut spans, &mut blocks, &lists, &mut pending_bullet, quote);
                blocks.push(Block::Rule);
            }
            Event::Html(h) | Event::InlineHtml(h) => spans.push(Span { text: h.to_string(), ..Default::default() }),
            _ => {}
        }
    }
    flush(&mut spans, &mut blocks, &lists, &mut pending_bullet, quote);
    blocks
}

fn job(spans: &[Span], t: &Tokens, size: f32, base_bold: bool, wrap_width: f32) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap_width;
    for s in spans {
        let mut f = TextFormat {
            font_id: FontId::new(size, if s.code { FontFamily::Monospace } else { FontFamily::Proportional }),
            color: t.text,
            ..Default::default()
        };
        if s.code {
            f.font_id = FontId::new(size * 0.9, FontFamily::Monospace);
            f.color = t.accent;
            f.background = if t.dark { Color32::from_rgb(28, 34, 66) } else { Color32::from_rgb(229, 246, 244) };
        }
        if s.bold || base_bold {
            f.color = if s.code { t.accent } else { strong(t) };
        }
        if s.italic {
            f.italics = true;
        }
        if s.link {
            f.color = t.accent;
            f.underline = Stroke::new(1.0, t.accent);
        }
        if s.strike {
            f.strikethrough = Stroke::new(1.0, t.muted);
        }
        job.append(&s.text, 0.0, f);
    }
    job
}

fn strong(t: &Tokens) -> Color32 {
    if t.dark { Color32::WHITE } else { Color32::BLACK }
}

fn plain(spans: &[Span]) -> String {
    spans.iter().map(|s| s.text.as_str()).collect()
}

pub fn is_shell(lang: &str) -> bool {
    matches!(lang, "bash" | "sh" | "shell" | "console" | "zsh")
}

/// Draw `md`. `id` keeps widget ids unique when several documents are on screen.
pub fn show(ui: &mut egui::Ui, t: &Tokens, md: &str, id: &str) -> Option<Action> {
    let mut action = None;
    let width = ui.available_width();
    for (i, block) in parse(md).iter().enumerate() {
        match block {
            Block::Heading(level, spans) => {
                let size = match level {
                    1 => 26.0,
                    2 => 21.0,
                    3 => 17.5,
                    _ => 15.5,
                };
                ui.add_space(if *level <= 2 { 14.0 } else { 8.0 });
                let mut j = job(spans, t, size, true, width);
                for s in &mut j.sections {
                    s.format.color = strong(t);
                }
                ui.label(j);
                ui.add_space(2.0);
            }
            Block::Para { spans, indent, bullet, quote } => {
                let j = job(spans, t, 15.0, false, (width - *indent as f32 * 18.0 - 24.0).max(120.0));
                if *quote {
                    egui::Frame::new()
                        .fill(t.accent_soft.gamma_multiply(if t.dark { 0.45 } else { 0.6 }))
                        .stroke(Stroke::new(1.0, t.accent.gamma_multiply(0.5)))
                        .corner_radius(CornerRadius::same(8))
                        .inner_margin(Margin::symmetric(12, 8))
                        .show(ui, |ui| {
                            ui.set_width(width - 28.0);
                            ui.label(j);
                        });
                } else if let Some(b) = bullet {
                    ui.horizontal_top(|ui| {
                        ui.add_space(*indent as f32 * 18.0 - 10.0);
                        ui.label(RichText::new(b).color(t.accent).strong());
                        ui.label(j);
                    });
                } else if *indent > 0 {
                    ui.horizontal_top(|ui| {
                        ui.add_space(*indent as f32 * 18.0 + 8.0);
                        ui.label(j);
                    });
                } else {
                    ui.label(j);
                }
                ui.add_space(3.0);
            }
            Block::Code { lang, text } => {
                if let Some(a) = code_block(ui, t, lang, text, &format!("{id}-{i}")) {
                    action = Some(a);
                }
            }
            Block::Table { head, rows } => {
                ui.add_space(4.0);
                egui::Frame::new()
                    .stroke(Stroke::new(1.0, t.border))
                    .corner_radius(CornerRadius::same(8))
                    .inner_margin(Margin::same(8))
                    .show(ui, |ui| {
                        egui::Grid::new(format!("{id}-table-{i}")).striped(true).spacing([18.0, 6.0]).show(ui, |ui| {
                            for cell in head {
                                ui.label(RichText::new(plain(cell)).strong().color(strong(t)));
                            }
                            ui.end_row();
                            for r in rows {
                                for cell in r {
                                    ui.label(job(cell, t, 14.0, false, 360.0));
                                }
                                ui.end_row();
                            }
                        });
                    });
                ui.add_space(4.0);
            }
            Block::Rule => {
                ui.separator();
            }
        }
    }
    action
}

/// A code block with a language tag, Copy, and (for shell) Run.
pub fn code_block(ui: &mut egui::Ui, t: &Tokens, lang: &str, text: &str, id: &str) -> Option<Action> {
    let mut action = None;
    ui.add_space(2.0);
    egui::Frame::new()
        .fill(t.code_bg)
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                if !lang.is_empty() {
                    ui.label(RichText::new(lang).size(11.0).color(Color32::from_rgb(120, 130, 170)));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("Copy").on_hover_text("Copy to the clipboard").clicked() {
                        ui.ctx().copy_text(text.to_string());
                    }
                    if is_shell(lang)
                        && ui
                            .small_button("▶ Run")
                            .on_hover_text("Run in the console, on your chosen machine")
                            .clicked()
                    {
                        action = Some(Action::Run(text.to_string()));
                    }
                });
            });
            egui::ScrollArea::horizontal().id_salt(id).show(ui, |ui| {
                ui.label(RichText::new(text).monospace().color(t.code_text));
            });
        });
    ui.add_space(4.0);
    action
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_common_blocks() {
        let md = "# Title\n\nSome **bold** and `code`.\n\n- one\n- two\n\n```bash\necho hi\n```\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n> note\n";
        let b = parse(md);
        assert!(matches!(&b[0], Block::Heading(1, _)));
        assert!(matches!(&b[1], Block::Para { bullet: None, .. }));
        assert!(matches!(&b[2], Block::Para { bullet: Some(x), .. } if x == "•"));
        assert!(matches!(&b[4], Block::Code { lang, text } if lang == "bash" && text == "echo hi"));
        assert!(matches!(&b[5], Block::Table { rows, .. } if rows.len() == 1));
        assert!(matches!(&b[6], Block::Para { quote: true, .. }));
    }

    #[test]
    fn ordered_lists_count() {
        let b = parse("1. a\n2. b\n");
        assert!(matches!(&b[1], Block::Para { bullet: Some(x), .. } if x == "2."));
    }
}
