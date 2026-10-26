use std::time::Duration;

use egui::RichText;
use poddojo_runner::cluster::{self, Status};
use poddojo_runner::exec::shell_quote;
use poddojo_runner::explore::{self, Health};

use super::{accent_button, title};
use crate::App;
use crate::jobs::Done;
use crate::theme::{self, Tokens};

impl App {
    fn refresh_kube(&mut self, ctx: &egui::Context) {
        let Some(env) = self.env_for("cluster") else { return };
        if !self.jobs.busy("kube namespaces") {
            let env2 = env.clone();
            self.jobs.spawn(ctx, "kube namespaces", move |_| Done::Namespaces(explore::namespaces(&env2)));
        }
        let kind = self.kube.kind.clone();
        let ns = self.kube.namespace.clone();
        self.jobs.spawn(ctx, "kube get", move |_| Done::Kube {
            objects: explore::kube_get(&env, &kind, ns.as_deref()),
            kind,
        });
    }

    pub fn cluster_page(&mut self, ui: &mut egui::Ui) {
        let t = self.t;
        let ctx = ui.ctx().clone();
        title(
            ui,
            &t,
            "Lab cluster",
            "A real Kubernetes cluster (kind) inside Docker on your chosen machine. PodDojo keeps its kubeconfig separate: your other clusters are never touched.",
        );
        self.cluster_card(ui, &t, &ctx);
        ui.add_space(10.0);
        if self.cluster_running() {
            self.cluster_explorer(ui, &t, &ctx);
        }
    }

    fn cluster_card(&mut self, ui: &mut egui::Ui, t: &Tokens, ctx: &egui::Context) {
        theme::card(t).show(ui, |ui| {
            ui.set_width(ui.available_width());
            let busy = self.jobs.busy("cluster change");
            let on = self.cluster_record.as_ref().map(|r| r.target.clone());
            match self.cluster.clone() {
                Some(Status::Running { nodes }) => {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("☸ kind-poddojo is running").size(17.0).strong().color(t.good));
                        if let Some(on) = &on {
                            ui.label(RichText::new(format!("on {on}")).color(t.muted));
                        }
                    });
                    for n in &nodes {
                        ui.label(format!("{}  {}  ·  {}  ·  {}", if n.ready { "✔" } else { "✖" }, n.name, n.role, n.version));
                    }
                    if let Some(on) = &on
                        && *on != self.target_name()
                    {
                        ui.label(RichText::new(format!("⚠ The cluster lives on {on}, but labs now run on {}. Switch back, or delete and recreate it here.", self.target_name())).color(t.amber));
                    }
                    ui.horizontal(|ui| {
                        if crate::icons::button(ui, crate::icons::Icon::Refresh, "Refresh").clicked() {
                            self.refresh_cluster(ctx);
                            self.refresh_kube(ctx);
                        }
                        if ui.add_enabled(!busy, egui::Button::new(RichText::new("Delete cluster").color(t.bad))).clicked() {
                            self.kube.confirm_delete = true;
                        }
                        if busy {
                            ui.spinner();
                        }
                    });
                }
                Some(Status::Missing) | None => {
                    ui.label(RichText::new("No lab cluster yet").size(17.0).strong());
                    ui.label(format!(
                        "Creates kind cluster `poddojo` on {}: 1 control-plane + {} worker, Kubernetes from kind's default node image (CKAD {}), NodePort 30080 published on the machine.",
                        self.target_name(),
                        if self.settings.worker { "1" } else { "0" },
                        poddojo_core::CKAD_CURRICULUM_VERSION
                    ));
                    ui.label(RichText::new("Needs `kind` and `kubectl` on this computer (see Setup). Takes about a minute the first time.").small().color(t.muted));
                    ui.horizontal(|ui| {
                        if ui.add_enabled(!busy, egui::Button::new("Create lab cluster")).clicked() {
                            self.create_cluster(ctx);
                        }
                        if busy {
                            ui.spinner();
                            ui.label("creating…");
                        }
                    });
                }
                Some(Status::Unreachable(e)) => {
                    ui.label(RichText::new("The lab cluster is not answering").size(17.0).strong().color(t.bad));
                    ui.label(RichText::new(e).color(t.muted));
                    ui.label("Is the machine on and Docker running? If the cluster's containers were removed, delete and recreate it.");
                    ui.horizontal(|ui| {
                        if crate::icons::button(ui, crate::icons::Icon::Refresh, "Retry").clicked() {
                            self.refresh_cluster(ctx);
                        }
                        if ui.add_enabled(!busy, egui::Button::new(RichText::new("Delete cluster").color(t.bad))).clicked() {
                            self.kube.confirm_delete = true;
                        }
                    });
                }
            }
            if let Some(id) = self.jobs.running.iter().find(|(_, r)| r.label.starts_with("cluster change")).map(|(id, _)| *id)
                && let Some(live) = self.jobs.live(id)
            {
                ui.label(RichText::new(live.trim_end()).monospace().size(12.0));
            } else if !self.cluster_log.is_empty() {
                egui::CollapsingHeader::new(RichText::new("Last log").small()).id_salt("clusterlog").show(ui, |ui| {
                    ui.label(RichText::new(self.cluster_log.trim_end()).monospace().size(12.0));
                });
            }
        });
        if self.kube.confirm_delete {
            let mut open = true;
            egui::Window::new("Delete the lab cluster?")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .show(ctx, |ui| {
                    ui.label("Everything you created in kind-poddojo is removed. Your progress is kept.");
                    ui.horizontal(|ui| {
                        if ui.button(RichText::new("Delete").color(t.bad)).clicked() {
                            self.delete_cluster(ctx);
                            open = false;
                        }
                        if ui.button("Cancel").clicked() {
                            open = false;
                        }
                    });
                });
            self.kube.confirm_delete = open;
        }
    }

    pub fn create_cluster(&mut self, ctx: &egui::Context) {
        let Some(target) = self.target() else { return };
        let env = target.env(&self.paths, self.paths.root.clone());
        let cmd = match cluster::create_command(&self.paths, &self.settings, &target) {
            Ok(c) => c,
            Err(e) => {
                self.toast(format!("Could not write the kind config: {e}"), false);
                return;
            }
        };
        let name = target.name.clone();
        self.jobs.spawn(ctx, "cluster change", move |live| {
            let out = poddojo_runner::exec::run_live(&cmd, &env, Duration::from_secs(900), Some(live));
            Done::ClusterChanged {
                ok: out.success(),
                log: format!("$ {cmd}\n{}", out.combined()),
                created_on: Some(name),
            }
        });
    }

    fn delete_cluster(&mut self, ctx: &egui::Context) {
        let selected = self.target_name();
        let target = match cluster::delete_target(&self.targets, self.cluster_record.as_ref(), &selected) {
            Ok(t) => t.clone(),
            Err(e) => {
                self.toast(e, false);
                return;
            }
        };
        let env = target.env(&self.paths, self.paths.root.clone());
        let cmd = cluster::delete_command(&self.paths);
        self.jobs.spawn(ctx, "cluster change", move |live| {
            let out = poddojo_runner::exec::run_live(&cmd, &env, Duration::from_secs(300), Some(live));
            Done::ClusterChanged { ok: out.success(), log: format!("$ {cmd}\n{}", out.combined()), created_on: None }
        });
    }

    fn cluster_explorer(&mut self, ui: &mut egui::Ui, t: &Tokens, ctx: &egui::Context) {
        if self.kube.namespaces.is_empty() && !self.jobs.busy("kube") {
            self.refresh_kube(ctx);
        }
        let mut reload = false;
        let mut run: Option<String> = None;
        ui.horizontal(|ui| {
            ui.label("Namespace");
            let current = self.kube.namespace.clone().unwrap_or_else(|| "all namespaces".into());
            egui::ComboBox::from_id_salt("ns").selected_text(current).width(180.0).show_ui(ui, |ui| {
                if ui.selectable_label(self.kube.namespace.is_none(), "all namespaces").clicked() {
                    self.kube.namespace = None;
                    reload = true;
                }
                for ns in self.kube.namespaces.clone() {
                    if ui.selectable_label(self.kube.namespace.as_deref() == Some(ns.as_str()), &ns).clicked() {
                        self.kube.namespace = Some(ns);
                        reload = true;
                    }
                }
            });
            if crate::icons::button(ui, crate::icons::Icon::Refresh, "").on_hover_text("Refresh").clicked() {
                reload = true;
            }
            if self.jobs.busy("kube") {
                ui.spinner();
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let scope = self.kube.namespace.as_ref().map_or("-A".to_string(), |n| format!("-n {}", shell_quote(n)));
                if ui.small_button("events").clicked() {
                    run = Some(format!("kubectl get events {scope} --sort-by=.lastTimestamp"));
                }
                if ui.small_button("get all").clicked() {
                    run = Some(format!("kubectl get all {scope}"));
                }
            });
        });
        ui.add_space(6.0);
        super::split_left(ui, 170.0, |ui, left| {
            if left {
                egui::ScrollArea::vertical().id_salt("kinds").show(ui, |ui| {
                    for kind in explore::KINDS {
                        if ui.selectable_label(self.kube.kind == *kind, *kind).clicked() {
                            self.kube.kind = kind.to_string();
                            self.kube.objects.clear();
                            self.kube.selected = None;
                            reload = true;
                        }
                    }
                });
                return;
            }
            if let Some(e) = &self.kube.error {
                ui.label(RichText::new(e).color(t.bad));
            }
            if self.kube.objects.is_empty() {
                ui.label(RichText::new(format!("No {} here.", self.kube.kind)).color(t.muted));
            }
            egui::ScrollArea::vertical().id_salt("objects").auto_shrink([false, false]).show(ui, |ui| {
                for o in self.kube.objects.clone() {
                    let color = match o.health {
                        Health::Good => t.good,
                        Health::Bad => t.bad,
                        Health::Pending => t.amber,
                        Health::Neutral => t.muted,
                    };
                    let key = (o.namespace.clone(), o.name.clone());
                    let selected = self.kube.selected.as_ref() == Some(&key);
                    theme::card(t)
                        .inner_margin(egui::Margin::symmetric(12, 8))
                        .stroke(egui::Stroke::new(1.0, if selected { t.accent } else { t.border }))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                let (rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                                ui.painter().circle_filled(rect.center(), 5.0, color);
                                if ui.selectable_label(selected, RichText::new(&o.name).strong()).clicked() {
                                    self.kube.selected = if selected { None } else { Some(key.clone()) };
                                }
                                if !o.namespace.is_empty() {
                                    ui.label(RichText::new(&o.namespace).small().color(t.muted));
                                }
                                ui.label(RichText::new(&o.status).color(color));
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.label(RichText::new(&o.age).small().color(t.muted));
                                });
                            });
                            if !o.detail.is_empty() {
                                ui.label(RichText::new(&o.detail).small().color(t.muted));
                            }
                            if selected {
                                let ns = if o.namespace.is_empty() {
                                    String::new()
                                } else {
                                    format!(" -n {}", shell_quote(&o.namespace))
                                };
                                let target = format!("{} {}{ns}", self.kube.kind, shell_quote(&o.name));
                                ui.horizontal(|ui| {
                                    if ui.small_button("describe").clicked() {
                                        run = Some(format!("kubectl describe {target}"));
                                    }
                                    if ui.small_button("yaml").clicked() {
                                        run = Some(format!("kubectl get {target} -o yaml"));
                                    }
                                    if self.kube.kind == "pods" && ui.small_button("logs").clicked() {
                                        run = Some(format!(
                                            "kubectl logs {} --all-containers --tail=100{ns}",
                                            shell_quote(&o.name)
                                        ));
                                    }
                                    if matches!(self.kube.kind.as_str(), "deployments" | "statefulsets" | "daemonsets")
                                        && ui.small_button("rollout history").clicked()
                                    {
                                        run = Some(format!("kubectl rollout history {target}"));
                                    }
                                });
                            }
                        });
                    ui.add_space(3.0);
                }
            });
        });
        if reload {
            self.refresh_kube(ctx);
        }
        if let Some(cmd) = run {
            self.run_in_terminal(&cmd);
        }
        let _ = accent_button;
    }
}
