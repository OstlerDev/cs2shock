use std::{
    process,
    sync::{mpsc, Arc},
    time::{Duration, Instant},
};

use eframe::icon_data::from_png_bytes;
use egui::{widgets::DragValue, Button, ComboBox, Id, TextEdit, ViewportBuilder};
use log::{debug, error, info};
use tokio::sync::RwLock;

use crate::{
    config::{Config, ShockMode, MAX_SHOCK_DURATION, MIN_SHOCK_DURATION},
    pishock,
    pishock_session_controller::{PishockSessionController, SessionAsyncResult},
};

const AUTO_SAVE_DEBOUNCE: Duration = Duration::from_millis(400);

pub async fn run(config: Arc<RwLock<Config>>) {
    let png_bytes = include_bytes!("../assets/icon.png");
    let viewport = ViewportBuilder::default()
        .with_inner_size([360.0, 670.0])
        .with_resizable(false)
        .with_icon(Arc::new(
            from_png_bytes(png_bytes).expect("Failed to load icon"),
        ));

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    let changes = config.read().await.clone();
    let (async_result_tx, async_result_rx) = mpsc::channel();
    let _ = eframe::run_native(
        "CS2 Shock",
        options,
        Box::new(move |_cc| {
            Box::new(MyApp::new(
                config,
                changes,
                async_result_tx,
                async_result_rx,
            ))
        }),
    );
}

struct MyApp {
    config: Arc<RwLock<Config>>,
    changes: Config,
    revealed_auth_field: Option<AuthField>,
    async_result_tx: mpsc::Sender<SessionAsyncResult>,
    async_result_rx: mpsc::Receiver<SessionAsyncResult>,
    session_controller: PishockSessionController,
    auto_save: AutoSaveState,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum AuthField {
    ApiKey,
}

#[derive(Debug, Default)]
struct AutoSaveState {
    pending_immediate_save: bool,
    last_debounced_change_at: Option<Instant>,
}

impl AutoSaveState {
    fn request_immediate_save(&mut self) {
        self.pending_immediate_save = true;
    }

    fn request_debounced_save(&mut self) {
        self.request_debounced_save_at(Instant::now());
    }

    fn request_debounced_save_at(&mut self, changed_at: Instant) {
        self.last_debounced_change_at = Some(changed_at);
    }

    fn take_save_due(&mut self) -> bool {
        self.take_save_due_at(Instant::now())
    }

    fn take_save_due_at(&mut self, now: Instant) -> bool {
        if self.pending_immediate_save {
            self.pending_immediate_save = false;
            self.last_debounced_change_at = None;
            return true;
        }

        if let Some(changed_at) = self.last_debounced_change_at {
            if now.duration_since(changed_at) >= AUTO_SAVE_DEBOUNCE {
                self.last_debounced_change_at = None;
                return true;
            }
        }

        false
    }

    fn has_pending(&self) -> bool {
        self.pending_immediate_save || self.last_debounced_change_at.is_some()
    }
}

fn text_row(ui: &mut egui::Ui, label: &str, value: &mut String, edit_id: Id) -> egui::Response {
    ui.horizontal(|ui| {
        let mut label_id = Id::NULL;
        ui.horizontal(|ui| {
            ui.set_width(85.0);
            label_id = ui.label(label).id;
        });

        ui.add(TextEdit::singleline(value).id_source(edit_id))
            .labelled_by(label_id)
    })
    .inner
}

fn secret_text_row(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut String,
    edit_id: Id,
    auth_field: AuthField,
    revealed_auth_field: &mut Option<AuthField>,
) -> egui::Response {
    ui.horizontal(|ui| {
        let mut label_id = Id::NULL;
        ui.horizontal(|ui| {
            ui.set_width(85.0);
            label_id = ui.label(label).id;
        });

        let is_revealed = *revealed_auth_field == Some(auth_field);
        let response = ui
            .add(
                TextEdit::singleline(value)
                    .id_source(edit_id)
                    .password(!is_revealed),
            )
            .labelled_by(label_id);

        if response.has_focus() && *revealed_auth_field != Some(auth_field) {
            *revealed_auth_field = Some(auth_field);
            ui.ctx().request_repaint();
        } else if response.lost_focus() && *revealed_auth_field == Some(auth_field) {
            *revealed_auth_field = None;
            ui.ctx().request_repaint();
        }
        response
    })
    .inner
}

fn has_selected_target(config: &Config) -> bool {
    config.selected_client_id.is_some() && config.selected_shocker_id.is_some()
}

fn should_require_selected_target(config: &Config) -> bool {
    !config.username.trim().is_empty() || !config.apikey.trim().is_empty()
}

fn apply_selected_target(config: &mut Config, target: &pishock::DiscoveredTarget) {
    config.selected_client_id = Some(target.client_id);
    config.selected_shocker_id = Some(target.shocker_id);
    config.selected_device_name = target.device_name.clone();
    config.selected_shocker_name = target.shocker_name.clone();
}

fn discovered_target_label(target: &pishock::DiscoveredTarget) -> String {
    let paused_suffix = if target.is_paused { " (paused)" } else { "" };
    format!(
        "{} / {}{}",
        target.device_name, target.shocker_name, paused_suffix
    )
}

fn selected_target_label(config: &Config) -> String {
    match (
        config.selected_device_name.trim(),
        config.selected_shocker_name.trim(),
        config.selected_client_id,
        config.selected_shocker_id,
    ) {
        (device_name, shocker_name, Some(_), Some(_))
            if !device_name.is_empty() && !shocker_name.is_empty() =>
        {
            format!("{device_name} / {shocker_name}")
        }
        (_, _, Some(client_id), Some(shocker_id)) => {
            format!("Device {client_id} / Shocker {shocker_id}")
        }
        _ => "Choose a PiShock shocker".into(),
    }
}

impl MyApp {
    fn new(
        config: Arc<RwLock<Config>>,
        changes: Config,
        async_result_tx: mpsc::Sender<SessionAsyncResult>,
        async_result_rx: mpsc::Receiver<SessionAsyncResult>,
    ) -> Self {
        let mut session_controller = PishockSessionController::new(&changes);
        session_controller.sync_startup(&async_result_tx, &changes);

        Self {
            config,
            changes,
            revealed_auth_field: None,
            async_result_tx,
            async_result_rx,
            session_controller,
            auto_save: AutoSaveState::default(),
        }
    }

    fn persist_changes_if_needed(&mut self) {
        let Some(current_config) = self.config.try_read().ok().map(|config| config.to_owned())
        else {
            return;
        };
        if current_config == self.changes {
            return;
        }

        if let Ok(mut owned_config) = self.config.clone().try_write() {
            *owned_config = self.changes.clone();
            match owned_config.try_write_to_file("cs2shock-config.json") {
                Ok(()) => debug!(target: "GUI", "Auto-saved config"),
                Err(err) => error!(target: "GUI", "Failed to auto-save config: {}", err),
            }
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok(result) = self.async_result_rx.try_recv() {
            let previous_changes = self.changes.clone();
            self.session_controller
                .handle_async_result(result, &mut self.changes);
            if self.changes != previous_changes {
                self.auto_save.request_immediate_save();
            }
            ctx.request_repaint();
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("CS2 Shock");

            let username_response = text_row(
                ui,
                "Username: ",
                &mut self.changes.username,
                ui.make_persistent_id("username_field"),
            );
            let api_key_response = secret_text_row(
                ui,
                "API key: ",
                &mut self.changes.apikey,
                ui.make_persistent_id("api_key_field"),
                AuthField::ApiKey,
                &mut self.revealed_auth_field,
            );

            if username_response.lost_focus() || api_key_response.lost_focus() {
                self.session_controller
                    .refresh_after_auth_commit(&self.async_result_tx, &mut self.changes);
                self.auto_save.request_immediate_save();
            }

            ui.vertical_centered_justified(|ui| {
                ui.separator();
                ui.horizontal(|ui| {
                    let discovery_button = if self.session_controller.discovery_in_progress() {
                        Button::new("Loading shockers...")
                    } else {
                        Button::new("Refresh shockers")
                    };

                    if ui
                        .add_enabled(
                            !self.session_controller.discovery_in_progress(),
                            discovery_button,
                        )
                        .clicked()
                    {
                        info!(target: "GUI", "Loading PiShock shockers");
                        self.session_controller
                            .refresh_manually(&self.async_result_tx, &self.changes);
                    }

                    let has_target = has_selected_target(&self.changes);
                    if ui
                        .add_enabled(has_target, Button::new("Test beep"))
                        .clicked()
                    {
                        info!(target: "GUI", "Sending test beep");
                        let config = Arc::new(RwLock::new(self.changes.clone()));
                        tokio::spawn(async move {
                            pishock::beep(config, 1).await;
                        });
                    }
                });
            });

            if !self.session_controller.discovered_targets().is_empty() {
                let mut chosen_target = None;

                ui.horizontal(|ui| {
                    ui.horizontal(|ui| {
                        ui.set_width(85.0);
                        ui.label("Shocker: ");
                    });

                    ComboBox::from_id_source("pishock_target_picker")
                        .selected_text(selected_target_label(&self.changes))
                        .show_ui(ui, |ui| {
                            for target in self.session_controller.discovered_targets() {
                                let is_selected = Some(target.client_id)
                                    == self.changes.selected_client_id
                                    && Some(target.shocker_id) == self.changes.selected_shocker_id;

                                if ui
                                    .selectable_label(is_selected, discovered_target_label(target))
                                    .clicked()
                                {
                                    chosen_target = Some(target.clone());
                                }
                            }
                        });
                });

                if let Some(target) = chosen_target {
                    apply_selected_target(&mut self.changes, &target);
                    self.session_controller.set_target_status(format!(
                        "Selected {}.",
                        discovered_target_label(&target)
                    ));
                    self.auto_save.request_immediate_save();
                }
            } else if has_selected_target(&self.changes) {
                ui.label(format!(
                    "Saved shocker: {}",
                    selected_target_label(&self.changes)
                ));
            }

            if should_require_selected_target(&self.changes) && !has_selected_target(&self.changes)
            {
                ui.label("Select a PiShock shocker before sending commands.");
            }

            if let Some(status) = self.session_controller.target_status() {
                ui.label(status);
            }

            ui.label(self.session_controller.broker_status_label());

            ui.vertical_centered(|ui| {
                ui.separator();
                ui.label("Shock Mode: ");
            });
            ui.vertical_centered_justified(|ui| {
                let random =
                    ui.selectable_value(&mut self.changes.shock_mode, ShockMode::Random, "Random");
                let last_hit = ui.selectable_value(
                    &mut self.changes.shock_mode,
                    ShockMode::LastHitPercentage,
                    "Last Hit Percentage",
                );
                if random.changed() || last_hit.changed() {
                    self.auto_save.request_immediate_save();
                }
            });
            ui.vertical_centered(|ui| ui.separator());

            ui.horizontal(|ui| {
                let indensity_label = ui.label("Intensity: ");

                let min_intensity = ui.add(
                    DragValue::new(&mut self.changes.min_intensity)
                        .speed(1)
                        .clamp_range(1..=self.changes.max_intensity)
                        .prefix("Min "),
                );
                let min_intensity_changed = min_intensity.changed();
                min_intensity.labelled_by(indensity_label.id);
                if min_intensity_changed {
                    self.auto_save.request_debounced_save();
                }

                let max_intensity = ui.add(
                    DragValue::new(&mut self.changes.max_intensity)
                        .speed(1)
                        .clamp_range(self.changes.min_intensity.max(1)..=100)
                        .prefix("Max "),
                );
                let max_intensity_changed = max_intensity.changed();
                max_intensity.labelled_by(indensity_label.id);
                if max_intensity_changed {
                    self.auto_save.request_debounced_save();
                }
            });
            ui.horizontal(|ui| {
                let duration_label = ui.label("Duration: ");
                let min_duration = ui.add(
                    DragValue::new(&mut self.changes.min_duration)
                        .speed(0.1)
                        .clamp_range(MIN_SHOCK_DURATION..=self.changes.max_duration)
                        .min_decimals(1)
                        .max_decimals(1)
                        .prefix("Min ")
                        .suffix(" sec"),
                );
                let min_duration_changed = min_duration.changed();
                min_duration.labelled_by(duration_label.id);
                if min_duration_changed {
                    self.auto_save.request_debounced_save();
                }

                let max_duration = ui.add(
                    DragValue::new(&mut self.changes.max_duration)
                        .speed(0.1)
                        .clamp_range(
                            self.changes.min_duration.max(MIN_SHOCK_DURATION)..=MAX_SHOCK_DURATION,
                        )
                        .min_decimals(1)
                        .max_decimals(1)
                        .prefix("Max ")
                        .suffix(" sec"),
                );
                let max_duration_changed = max_duration.changed();
                max_duration.labelled_by(duration_label.id);
                if max_duration_changed {
                    self.auto_save.request_debounced_save();
                }
            });

            let beep_on_match_start = ui.add(egui::Checkbox::new(
                &mut self.changes.beep_on_match_start,
                "Beep on match start",
            ));
            if beep_on_match_start.changed() {
                self.auto_save.request_immediate_save();
            }

            let beep_on_round_start = ui.add(egui::Checkbox::new(
                &mut self.changes.beep_on_round_start,
                "Beep on round start",
            ));
            if beep_on_round_start.changed() {
                self.auto_save.request_immediate_save();
            }

            let warning_beep_before_shock = ui.add(egui::Checkbox::new(
                &mut self.changes.warning_beep_before_shock,
                "Play warning beep",
            ));
            if warning_beep_before_shock.changed() {
                self.auto_save.request_immediate_save();
            }

            let shock_on_round_loss_only = ui.add(egui::Checkbox::new(
                &mut self.changes.shock_on_round_loss_only,
                "Only shock if team loses round",
            ));
            if shock_on_round_loss_only.changed() {
                self.auto_save.request_immediate_save();
            }

            let prevent_shock_if_round_kills_reached = ui.add(egui::Checkbox::new(
                &mut self.changes.prevent_shock_if_round_kills_reached,
                "Prevent shock after round kills",
            ));
            if prevent_shock_if_round_kills_reached.changed() {
                self.auto_save.request_immediate_save();
            }

            ui.horizontal(|ui| {
                let round_kill_label = ui.label("Kill threshold: ");
                let round_kills = ui.add_enabled(
                    self.changes.prevent_shock_if_round_kills_reached,
                    DragValue::new(&mut self.changes.round_kills_to_prevent_shock)
                        .speed(1)
                        .clamp_range(1..=5)
                        .suffix(" kills"),
                );
                let round_kills_changed = round_kills.changed();
                round_kills.labelled_by(round_kill_label.id);
                if round_kills_changed {
                    self.auto_save.request_debounced_save();
                }
            });
            ui.horizontal(|ui| {
                let warning_beep_label = ui.label("Warning beep duration: ");
                let warning_duration = ui.add_enabled(
                    self.changes.warning_beep_before_shock,
                    DragValue::new(&mut self.changes.warning_beep_duration)
                        .speed(1)
                        .clamp_range(1..=15)
                        .suffix(" sec"),
                );
                let warning_duration_changed = warning_duration.changed();
                warning_duration.labelled_by(warning_beep_label.id);
                if warning_duration_changed {
                    self.auto_save.request_debounced_save();
                }
            });
            ui.horizontal(|ui| {
                let shock_chance_label = ui.label("Chance to shock: ");
                let shock_chance = ui.add(
                    DragValue::new(&mut self.changes.shock_chance)
                        .speed(1)
                        .clamp_range(0..=100)
                        .suffix("%"),
                );
                let shock_chance_changed = shock_chance.changed();
                shock_chance.labelled_by(shock_chance_label.id);
                if shock_chance_changed {
                    self.auto_save.request_debounced_save();
                }
            });

            ui.vertical_centered(|ui| {
                ui.separator();
            });

            if self.auto_save.take_save_due() {
                self.persist_changes_if_needed();
            }

            if ctx.input(|i| i.viewport().close_requested()) {
                info!(target: "GUI", "Closing");
                if self.auto_save.has_pending() {
                    debug!(target: "GUI", "Flushing pending auto-save before close");
                }
                self.persist_changes_if_needed();
                process::exit(0);
            }
        });

        // Keep periodic UI ticks for heartbeat freshness and debounce checks.
        ctx.request_repaint_after(Duration::from_millis(100));
    }
}

#[cfg(test)]
mod tests {
    use super::{AutoSaveState, AUTO_SAVE_DEBOUNCE};
    use std::time::{Duration, Instant};

    #[test]
    fn immediate_save_triggers_once() {
        let mut state = AutoSaveState::default();
        state.request_immediate_save();

        assert!(state.take_save_due_at(Instant::now()));
        assert!(!state.take_save_due_at(Instant::now()));
    }

    #[test]
    fn debounced_save_waits_for_idle_window() {
        let mut state = AutoSaveState::default();
        let started_at = Instant::now();
        state.request_debounced_save_at(started_at);

        assert!(!state.take_save_due_at(started_at + AUTO_SAVE_DEBOUNCE - Duration::from_millis(1)));
        assert!(state.take_save_due_at(started_at + AUTO_SAVE_DEBOUNCE));
    }

    #[test]
    fn immediate_save_clears_pending_debounced_save() {
        let mut state = AutoSaveState::default();
        let started_at = Instant::now();
        state.request_debounced_save_at(started_at);
        state.request_immediate_save();

        assert!(state.take_save_due_at(started_at));
        assert!(!state.has_pending());
    }
}
