use egui::RichText;
use poddojo_runner::{LabEnv, doctor};

use super::title;
use crate::App;
use crate::icons::{self, Icon};
use crate::jobs::Done;
use crate::theme::{self, Tokens};

impl App {
    pub fn setup_page(&mut self, ui: &mut egui::Ui) {
        let t = self.t;
        let ctx = ui.ctx().clone();
        if self.doctor.is_none() && !self.jobs.busy("doctor") {
            let env = LabEnv {
                docker_context: String::new(),
                kubeconfig: self.paths.kubeconfig(),
                workdir: self.paths.root.clone(),
                host: "127.0.0.1".into(),
                cluster: "poddojo".into(),
            };
            self.jobs.spawn(&ctx, "doctor", move |_| Done::Doctor(doctor::check(&env)));
        }
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            title(ui, &t, "Setup", "Tools, machines and the lab cluster.");
            self.tools_card(ui, &t, &ctx);
            ui.add_space(10.0);
            self.machines_card(ui, &t, &ctx);
            ui.add_space(10.0);
            self.prefs_card(ui, &t, &ctx);
        });
    }

    fn tools_card(&mut self, ui: &mut egui::Ui, t: &Tokens, ctx: &egui::Context) {
        theme::card(t).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new("Tools on this computer").size(17.0).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if icons::button(ui, Icon::Refresh, "Check again").clicked() {
                        self.doctor = None;
                    }
                });
            });
            ui.add_space(6.0);
            let Some(tools) = self.doctor.clone() else {
                ui.spinner();
                return;
            };
            let server = self.cluster.as_ref().and_then(|c| c.version()).map(str::to_string);
            for tool in &tools {
                ui.horizontal(|ui| {
                    let (color, ok) = match &tool.version {
                        Some(_) => (t.good, true),
                        None if tool.required => (t.bad, false),
                        None => (t.amber, false),
                    };
                    let (rect, _) = ui.allocate_exact_size(egui::vec2(30.0, 30.0), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 8.0, color.gamma_multiply(0.14));
                    if ok {
                        icons::paint(ui.painter(), rect.shrink(8.0), Icon::Check, color);
                    } else {
                        ui.painter().text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "!",
                            egui::FontId::proportional(16.0),
                            color,
                        );
                    }
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(tool.name).strong().size(15.0));
                            if let Some(v) = &tool.version {
                                ui.label(RichText::new(v).monospace().size(12.0).color(t.muted));
                            }
                        });
                        ui.label(RichText::new(tool.why).size(12.5).color(t.muted));
                    });
                    if tool.version.is_none() {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("Copy").clicked() {
                                ctx.copy_text(tool.install.split("  ").next().unwrap_or(tool.install).to_string());
                            }
                            ui.label(RichText::new(tool.install).monospace().size(12.0));
                        });
                    }
                });
                if tool.name == "kubectl"
                    && let (Some(v), Some(server)) = (&tool.version, &server)
                    && let Some(warning) = doctor::kubectl_skew(v, server)
                {
                    egui::Frame::new()
                        .fill(t.amber.gamma_multiply(0.12))
                        .corner_radius(8)
                        .inner_margin(egui::Margin::symmetric(12, 8))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.label(RichText::new(warning).color(t.amber));
                        });
                }
                ui.add_space(6.0);
            }
        });
    }

    fn machines_card(&mut self, ui: &mut egui::Ui, t: &Tokens, ctx: &egui::Context) {
        // Test every machine once when the page opens (each probe runs on its own thread).
        if !self.setup_probed && !self.targets.is_empty() {
            self.setup_probed = true;
            for target in self.targets.clone() {
                if !self.engine.contains_key(&target.name) {
                    self.probe_target(ctx, &target);
                }
            }
        }
        theme::card(t).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new("Machines").size(17.0).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if icons::button(ui, Icon::Refresh, "Test all").clicked() {
                        self.engine.clear();
                        self.setup_probed = false;
                        self.refresh_targets(ctx);
                    }
                });
            });
            ui.label(RichText::new("Every Docker context is a machine you can train on: Docker Desktop here, or any Linux box you can SSH into.").color(t.muted));
            if let Some(e) = &self.targets_error {
                ui.label(RichText::new(e).color(t.bad));
            }
            ui.add_space(6.0);
            let current = self.target().map(|c| c.name);
            let mut pick = None;
            for target in &self.targets {
                let selected = current.as_deref() == Some(target.name.as_str());
                let probing = self.jobs.busy(&format!("probe {}", target.name));
                let (status, color) = match self.engine.get(&target.name) {
                    Some(Ok(i)) => (format!("Docker {} · {} · {} CPUs", i.server_version, i.arch, i.cpus), t.good),
                    Some(Err(e)) => (poddojo_core::check::shorten(e, 70), t.bad),
                    None if probing => ("testing…".into(), t.muted),
                    None => ("not tested".into(), t.muted),
                };
                let resp = egui::Frame::new()
                    .fill(if selected { t.accent_soft } else { t.card_hover.gamma_multiply(0.5) })
                    .stroke(egui::Stroke::new(if selected { 1.5 } else { 1.0 }, if selected { t.accent } else { t.border }))
                    .corner_radius(10)
                    .inner_margin(egui::Margin::symmetric(12, 8))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            let (rect, _) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());
                            ui.painter().circle_stroke(rect.center(), 7.5, egui::Stroke::new(1.5, if selected { t.accent } else { t.muted }));
                            if selected {
                                ui.painter().circle_filled(rect.center(), 4.0, t.accent);
                            }
                            icons::show(ui, Icon::Server, 18.0, if selected { t.accent } else { t.muted });
                            ui.label(RichText::new(&target.name).strong().size(15.0));
                            ui.label(RichText::new(if target.is_remote() { target.endpoint.as_str() } else { "this computer" }).monospace().size(12.0).color(t.muted));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(RichText::new(status).size(12.5).color(color));
                                let (r, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                                ui.painter().circle_filled(r.center(), 4.0, color);
                            });
                        });
                    })
                    .response
                    .interact(egui::Sense::click());
                resp.widget_info(|| {
                    egui::WidgetInfo::selected(egui::WidgetType::RadioButton, true, selected, format!("Machine {}", target.name))
                });
                if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() && !selected {
                    pick = Some(target.name.clone());
                }
                ui.add_space(4.0);
            }
            if let Some(name) = pick {
                self.select_target(ctx, &name);
            }
            ui.add_space(8.0);
            ui.label(RichText::new("Add a machine (on this computer, once). It needs Docker Engine and key-based SSH:").size(12.5).color(t.muted));
            let cmd = "docker context create my-server --docker host=ssh://user@192.168.1.20";
            ui.horizontal(|ui| {
                ui.label(RichText::new(cmd).monospace().size(12.5));
                if ui.small_button("Copy").clicked() {
                    ctx.copy_text(cmd.into());
                }
            });
        });
    }

    fn prefs_card(&mut self, ui: &mut egui::Ui, t: &Tokens, ctx: &egui::Context) {
        theme::card(t).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new("Preferences").size(17.0).strong());
            let mut changed = false;
            ui.horizontal(|ui| {
                ui.label("Theme");
                changed |= ui.selectable_value(&mut self.settings.dark, true, "Dark").changed();
                changed |= ui.selectable_value(&mut self.settings.dark, false, "Light").changed();
                ui.separator();
                ui.label("Zoom");
                changed |= ui.add(egui::Slider::new(&mut self.settings.zoom, 0.75..=1.6).step_by(0.05)).changed();
                ui.separator();
                changed |= ui
                    .checkbox(&mut self.settings.motion, "Animations")
                    .on_hover_text("The living logo and transitions. Turn off for reduced motion.")
                    .changed();
            });
            ui.horizontal(|ui| {
                changed |= ui.checkbox(&mut self.settings.worker, "Lab cluster with a worker node").changed();
                ui.label("API port");
                changed |= ui.add(egui::DragValue::new(&mut self.settings.api_port).range(1024..=65000)).changed();
                ui.label("Node image");
                changed |= ui
                    .add(
                        egui::TextEdit::singleline(&mut self.settings.node_image)
                            .hint_text("kind default")
                            .desired_width(200.0),
                    )
                    .changed();
            });
            ui.label(
                RichText::new("Cluster options apply the next time you create the lab cluster.").small().color(t.muted),
            );
            if changed {
                self.t = Tokens::get(self.settings.dark);
                theme::apply(ctx, &self.t, self.settings.zoom, self.settings.motion);
                self.save_settings();
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("Data folder: {}", self.paths.root.display())).small().color(t.muted));
                if ui.small_button("Copy path").clicked() {
                    ctx.copy_text(self.paths.root.display().to_string());
                }
            });
        });
    }
}
