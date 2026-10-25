use egui::RichText;
use poddojo_runner::{LabEnv, doctor};

use super::title;
use crate::App;
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
                if ui.small_button("⟳").clicked() {
                    self.doctor = None;
                }
            });
            match &self.doctor {
                None => {
                    ui.spinner();
                }
                Some(tools) => {
                    egui::Grid::new("tools").spacing([16.0, 6.0]).show(ui, |ui| {
                        for tool in tools {
                            match &tool.version {
                                Some(v) => {
                                    ui.label(RichText::new("✔").color(t.good));
                                    ui.label(RichText::new(tool.name).strong());
                                    ui.label(RichText::new(v).small().color(t.muted));
                                }
                                None => {
                                    ui.label(
                                        RichText::new(if tool.required { "✖" } else { "!" }).color(if tool.required {
                                            t.bad
                                        } else {
                                            t.amber
                                        }),
                                    );
                                    ui.label(RichText::new(tool.name).strong());
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new(tool.install).monospace().size(12.0));
                                        if ui.small_button("Copy").clicked() {
                                            ctx.copy_text(
                                                tool.install.split("  ").next().unwrap_or(tool.install).to_string(),
                                            );
                                        }
                                    });
                                }
                            }
                            ui.label(RichText::new(tool.why).small().color(t.muted));
                            ui.end_row();
                        }
                    });
                }
            }
        });
    }

    fn machines_card(&mut self, ui: &mut egui::Ui, t: &Tokens, ctx: &egui::Context) {
        theme::card(t).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new("Machines").size(17.0).strong());
                if ui.small_button("⟳").clicked() {
                    self.refresh_targets(ctx);
                }
            });
            ui.label(RichText::new("Every Docker context is a machine you can train on: Docker Desktop here, or any Linux box you can SSH into.").color(t.muted));
            if let Some(e) = &self.targets_error {
                ui.label(RichText::new(e).color(t.bad));
            }
            let current = self.target().map(|c| c.name);
            let mut pick = None;
            let mut probe = None;
            egui::Grid::new("machines").striped(true).spacing([16.0, 8.0]).show(ui, |ui| {
                for target in &self.targets {
                    let selected = current.as_deref() == Some(target.name.as_str());
                    if ui.radio(selected, RichText::new(&target.name).strong()).clicked() && !selected {
                        pick = Some(target.name.clone());
                    }
                    ui.label(RichText::new(&target.endpoint).monospace().size(12.0));
                    match self.engine.get(&target.name) {
                        Some(Ok(i)) => {
                            ui.label(RichText::new(format!("Docker {} · {} · {}", i.server_version, i.os, i.arch)).small().color(t.good));
                        }
                        Some(Err(e)) => {
                            ui.label(RichText::new(poddojo_core::check::shorten(e, 60)).small().color(t.bad));
                        }
                        None => {
                            if ui.small_button("test").clicked() {
                                probe = Some(target.clone());
                            }
                        }
                    }
                    ui.end_row();
                }
            });
            if let Some(name) = pick {
                self.select_target(ctx, &name);
            }
            if let Some(target) = probe {
                self.probe_target(ctx, &target);
            }
            ui.add_space(6.0);
            ui.label(RichText::new("Add a machine (on this computer, once):").small().color(t.muted));
            let cmd = "docker context create my-server --docker host=ssh://user@192.168.1.20";
            ui.horizontal(|ui| {
                ui.label(RichText::new(cmd).monospace().size(12.0));
                if ui.small_button("Copy").clicked() {
                    ctx.copy_text(cmd.into());
                }
            });
            ui.label(RichText::new("The remote machine needs Docker Engine and key-based SSH. Published ports and the lab cluster's API are then reached on its address.").small().color(t.muted));
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
                theme::apply(ctx, &self.t, self.settings.zoom);
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
