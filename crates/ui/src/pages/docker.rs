use egui::RichText;
use poddojo_runner::exec::shell_quote;
use poddojo_runner::explore::{self, Row};

use super::title;
use crate::jobs::Done;
use crate::theme::{self, Tokens};
use crate::{App, DockerTab};

/// Created by PodDojo's labs (or its lab cluster), by the naming rule every lab follows.
fn is_dojo(name: &str) -> bool {
    let name = name.trim_start_matches('/');
    name.starts_with("dojo-")
        || name.starts_with("dojo/")
        || name.starts_with("poddojo")
        || name.starts_with("kindest/")
        || name == "kind"
}

fn field<'a>(r: &'a Row, k: &str) -> &'a str {
    r.get(k).map_or("", String::as_str)
}

impl App {
    pub fn refresh_docker(&mut self, ctx: &egui::Context) {
        if self.jobs.busy("docker list") {
            return;
        }
        let Some(env) = self.env_for("playground") else { return };
        let target = self.target_name();
        self.docker.loaded_for = Some(target.clone());
        self.jobs.spawn(ctx, "docker list", move |_| Done::Docker {
            target,
            containers: explore::containers(&env),
            images: explore::images(&env),
            volumes: explore::volumes(&env),
            networks: explore::networks(&env),
        });
    }

    fn load_layers(&mut self, ctx: &egui::Context, image: String) {
        let Some(env) = self.env_for("playground") else { return };
        self.docker.image = Some(image.clone());
        self.docker.layers.clear();
        self.docker.layers_error = None;
        let target = self.target_name();
        self.jobs.spawn(ctx, "docker layers", move |_| Done::Layers {
            target,
            layers: explore::history(&env, &image),
            image,
        });
    }

    pub fn docker_page(&mut self, ui: &mut egui::Ui) {
        let t = self.t;
        let ctx = ui.ctx().clone();
        if self.docker.loaded_for.as_deref() != Some(self.target_name().as_str()) && self.target().is_some() {
            self.refresh_docker(&ctx);
        }
        title(
            ui,
            &t,
            "Docker",
            &format!(
                "Everything on {}: run commands from the buttons and watch them in the terminal, so you learn the CLI as you click.",
                self.target_name()
            ),
        );
        ui.horizontal(|ui| {
            for (tab, label, n) in [
                (DockerTab::Containers, "Containers", self.docker.containers.len()),
                (DockerTab::Images, "Images", self.docker.images.len()),
                (DockerTab::Volumes, "Volumes", self.docker.volumes.len()),
                (DockerTab::Networks, "Networks", self.docker.networks.len()),
            ] {
                if theme::tab(ui, &t, &format!("{label} · {n}"), self.docker.tab == tab, false).clicked() {
                    self.docker.tab = tab;
                }
            }
            ui.separator();
            egui::Frame::new()
                .fill(t.card_hover.gamma_multiply(0.5))
                .stroke(egui::Stroke::new(1.0, t.border))
                .corner_radius(10)
                .inner_margin(egui::Margin::symmetric(10, 4))
                .show(ui, |ui| {
                    crate::icons::show(ui, crate::icons::Icon::Search, 14.0, t.muted);
                    ui.add(
                        egui::TextEdit::singleline(&mut self.docker.filter)
                            .frame(egui::Frame::NONE)
                            .hint_text("Filter")
                            .desired_width(150.0),
                    );
                });
            ui.checkbox(&mut self.docker.show_all, "Show everything on this machine")
                .on_hover_text("Off: only what PodDojo's labs created (dojo-… names and the lab cluster)");
            if crate::icons::button(ui, crate::icons::Icon::Refresh, "Refresh").clicked() {
                self.refresh_docker(&ctx);
            }
            if self.jobs.busy("docker list") {
                ui.spinner();
            }
        });
        if let Some(e) = &self.docker.error {
            ui.label(RichText::new(e).color(t.bad));
        }
        ui.add_space(6.0);
        match self.docker.tab {
            DockerTab::Containers => self.containers_tab(ui, &t, &ctx),
            DockerTab::Images => self.images_tab(ui, &t, &ctx),
            DockerTab::Volumes => self.simple_tab(ui, &t, "volume"),
            DockerTab::Networks => self.simple_tab(ui, &t, "network"),
        }
    }

    fn filtered(&self, rows: &[Row], keys: &[&str]) -> Vec<Row> {
        let f = self.docker.filter.to_lowercase();
        rows.iter()
            .filter(|r| self.docker.show_all || keys.first().is_some_and(|k| is_dojo(field(r, k))))
            .filter(|r| f.is_empty() || keys.iter().any(|k| field(r, k).to_lowercase().contains(&f)))
            .cloned()
            .collect()
    }

    fn containers_tab(&mut self, ui: &mut egui::Ui, t: &Tokens, ctx: &egui::Context) {
        let rows = self.filtered(&self.docker.containers, &["Names", "Image", "Status"]);
        let mut run: Option<String> = None;
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            if rows.is_empty() {
                let hidden = if self.docker.show_all { 0 } else { self.docker.containers.len() };
                if hidden > 0 {
                    ui.label(
                        RichText::new(format!(
                            "No PodDojo containers ({hidden} other container(s) hidden: tick \"Show everything\")."
                        ))
                        .color(t.muted),
                    );
                }
                ui.label(
                    RichText::new("No containers. Try: docker run -d --name dojo-web -p 18080:80 nginx:1.27-alpine")
                        .color(t.muted),
                );
                if ui.button("▶ Run that").clicked() {
                    run = Some("docker run -d --name dojo-web -p 18080:80 nginx:1.27-alpine".into());
                }
            }
            for r in &rows {
                let name = field(r, "Names").to_string();
                let running = field(r, "State") == "running";
                theme::card(t).inner_margin(egui::Margin::symmetric(14, 10)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        let (rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                        ui.painter().circle_filled(rect.center(), 5.0, if running { t.good } else { t.muted });
                        ui.label(RichText::new(&name).strong().size(15.0));
                        let ours = is_dojo(&name);
                        if ours {
                            theme::chip(ui, "dojo", t.accent, t.accent_soft);
                        }
                        let image = field(r, "Image");
                        let image = image.split('@').next().unwrap_or(image);
                        ui.label(RichText::new(poddojo_core::check::shorten(image, 44)).monospace().color(t.muted))
                            .on_hover_text(field(r, "Image"));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let q = shell_quote(&name);
                            if ui.small_button("remove").on_hover_text(format!("docker rm -f {name}")).clicked() {
                                self.docker.confirm_remove = Some(name.clone());
                            }
                            if ui.small_button("inspect").clicked() {
                                run = Some(format!("docker inspect {q}"));
                            }
                            if ui.small_button("logs").clicked() {
                                run = Some(format!("docker logs --tail 100 {q}"));
                            }
                            if running {
                                if ui.small_button("top").clicked() {
                                    run = Some(format!("docker top {q}"));
                                }
                                if ui.small_button("stop").clicked() {
                                    if ours {
                                        run = Some(format!("docker stop {q}"));
                                    } else {
                                        self.docker.confirm_stop = Some(name.clone());
                                    }
                                }
                            } else if ui.small_button("start").clicked() {
                                run = Some(format!("docker start {q}"));
                            }
                        });
                    });
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(field(r, "Status")).small().color(t.muted));
                        let ports = field(r, "Ports");
                        if !ports.is_empty() {
                            ui.label(RichText::new(format!("· {ports}")).small().color(t.accent));
                        }
                    });
                });
                ui.add_space(4.0);
            }
        });
        if let Some(name) = self.docker.confirm_stop.clone() {
            let mut keep_open = true;
            egui::Window::new("Stop this container?")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .show(ctx, |ui| {
                    ui.label(RichText::new(format!("⚠ `{name}` was not created by PodDojo.")).color(t.amber));
                    ui.label("Something else on this machine may depend on it.");
                    ui.horizontal(|ui| {
                        if ui.button("Stop it").clicked() {
                            run = Some(format!("docker stop {}", shell_quote(&name)));
                            keep_open = false;
                        }
                        if ui.button("Cancel").clicked() {
                            keep_open = false;
                        }
                    });
                });
            if !keep_open {
                self.docker.confirm_stop = None;
            }
        }
        if let Some(name) = self.docker.confirm_remove.clone() {
            let mut keep_open = true;
            egui::Window::new("Remove container?")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .show(ctx, |ui| {
                    ui.label(format!(
                        "This stops and deletes `{name}` on {}. Its writable layer is lost.",
                        self.target_name()
                    ));
                    if !is_dojo(&name) {
                        ui.label(RichText::new("⚠ This container was not created by PodDojo.").color(t.amber));
                    }
                    ui.horizontal(|ui| {
                        if ui.button(RichText::new("Remove").color(t.bad)).clicked() {
                            run = Some(format!("docker rm -f {}", shell_quote(&name)));
                            keep_open = false;
                        }
                        if ui.button("Cancel").clicked() {
                            keep_open = false;
                        }
                    });
                });
            if !keep_open {
                self.docker.confirm_remove = None;
            }
        }
        if let Some(cmd) = run {
            self.run_in_terminal(&cmd);
            self.docker.loaded_for = None;
        }
    }

    fn images_tab(&mut self, ui: &mut egui::Ui, t: &Tokens, ctx: &egui::Context) {
        let mut run: Option<String> = None;
        let mut layers_for: Option<String> = None;
        ui.horizontal(|ui| {
            ui.label("Pull an image:");
            let resp = ui.add(
                egui::TextEdit::singleline(&mut self.docker.pull).hint_text("nginx:1.27-alpine").desired_width(260.0),
            );
            let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if (super::accent_button(ui, t, "Pull").clicked() || enter) && !self.docker.pull.trim().is_empty() {
                run = Some(format!("docker pull {}", shell_quote(self.docker.pull.trim())));
            }
        });
        ui.add_space(6.0);
        let rows = self.filtered(&self.docker.images, &["Repository", "Tag"]);
        ui.columns(2, |cols| {
            egui::ScrollArea::vertical().id_salt("images").auto_shrink([false, false]).show(&mut cols[0], |ui| {
                for r in &rows {
                    let name = if field(r, "Tag") == "<none>" { field(r, "ID").to_string() } else { format!("{}:{}", field(r, "Repository"), field(r, "Tag")) };
                    let selected = self.docker.image.as_deref() == Some(name.as_str());
                    theme::card(t).inner_margin(egui::Margin::symmetric(12, 8)).stroke(egui::Stroke::new(1.0, if selected { t.accent } else { t.border })).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            if ui.selectable_label(selected, RichText::new(&name).strong()).clicked() {
                                layers_for = Some(name.clone());
                            }
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(RichText::new(field(r, "Size")).color(t.accent));
                            });
                        });
                        ui.label(RichText::new(format!("{} · {}", field(r, "ID"), field(r, "CreatedSince"))).small().color(t.muted));
                    });
                    ui.add_space(3.0);
                }
            });
            let ui = &mut cols[1];
            match &self.docker.image {
                None => {
                    ui.label(RichText::new("Select an image to see its layers: every Dockerfile instruction that changed the filesystem adds one, and they are shared between images.").color(t.muted));
                }
                Some(img) => {
                    ui.label(RichText::new(format!("Layers of {img}")).strong().size(16.0));
                    ui.label(RichText::new("Newest on top. Bars show each layer's size.").small().color(t.muted));
                    if let Some(e) = &self.docker.layers_error {
                        ui.label(RichText::new(e).color(t.bad));
                    }
                    let max = self.docker.layers.iter().map(|l| l.bytes).max().unwrap_or(1).max(1);
                    let total: u64 = self.docker.layers.iter().map(|l| l.bytes).sum();
                    ui.label(RichText::new(format!("{} layers · {}", self.docker.layers.len(), explore::human_bytes(total))).color(t.accent));
                    egui::ScrollArea::vertical().id_salt("layers").auto_shrink([false, false]).show(ui, |ui| {
                        for l in &self.docker.layers {
                            let w = ui.available_width();
                            ui.horizontal(|ui| {
                                let frac = l.bytes as f32 / max as f32;
                                theme::bar(ui, t, frac.max(0.01), (w * 0.35).max(60.0), if l.bytes == 0 { t.muted } else { t.accent });
                                ui.label(RichText::new(&l.size).monospace().size(12.0));
                            });
                            ui.label(RichText::new(&l.created_by).monospace().size(12.0).color(t.code_text));
                            ui.add_space(4.0);
                        }
                    });
                    ui.horizontal(|ui| {
                        let q = shell_quote(img);
                        if ui.small_button("history").clicked() {
                            run = Some(format!("docker history {q}"));
                        }
                        if ui.small_button("inspect").clicked() {
                            run = Some(format!("docker image inspect {q}"));
                        }
                        if ui.small_button("run it").clicked() {
                            run = Some(format!("docker run --rm {q} echo hello from {q}"));
                        }
                    });
                }
            }
        });
        if let Some(img) = layers_for {
            self.load_layers(ctx, img);
        }
        if let Some(cmd) = run {
            self.run_in_terminal(&cmd);
            self.docker.loaded_for = None;
        }
    }

    fn simple_tab(&mut self, ui: &mut egui::Ui, t: &Tokens, what: &str) {
        let (rows, keys): (Vec<Row>, [&str; 2]) = if what == "volume" {
            (self.filtered(&self.docker.volumes, &["Name", "Driver"]), ["Name", "Driver"])
        } else {
            (self.filtered(&self.docker.networks, &["Name", "Driver"]), ["Name", "Driver"])
        };
        let mut run = None;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            egui::Grid::new(format!("grid-{what}")).striped(true).spacing([24.0, 8.0]).show(ui, |ui| {
                ui.label(RichText::new("Name").strong());
                ui.label(RichText::new("Driver").strong());
                ui.label(RichText::new(if what == "volume" { "Mountpoint" } else { "Scope" }).strong());
                ui.label("");
                ui.end_row();
                for r in &rows {
                    let name = field(r, keys[0]);
                    ui.label(RichText::new(name).monospace());
                    ui.label(field(r, keys[1]));
                    ui.label(
                        RichText::new(field(r, if what == "volume" { "Mountpoint" } else { "Scope" }))
                            .small()
                            .color(t.muted),
                    );
                    if ui.small_button("inspect").clicked() {
                        run = Some(format!("docker {what} inspect {}", shell_quote(name)));
                    }
                    ui.end_row();
                }
            });
        });
        if let Some(cmd) = run {
            self.run_in_terminal(&cmd);
        }
    }
}
