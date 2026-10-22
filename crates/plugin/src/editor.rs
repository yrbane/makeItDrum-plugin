//! Interface egui : selectbox de kits, chemin, gain, grille de pads.
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use nih_plug::prelude::*;
use nih_plug_egui::{create_egui_editor, egui, widgets};

use make_it_drum_core::kit::PAD_COUNT;

use crate::loader::{display_name, kits_dir, scan_kits};
use crate::params::KitPlayerParams;
use crate::slot::KitSlot;
use crate::{KitPlayer, Task};

pub const CREDIT_URL: &str = "https://beacons.ai/saiyageez";
pub const PROJECT_URL: &str = "https://github.com/yrbane/makeItDrum-plugin";

struct EditorState {
    kits: Vec<PathBuf>,
    path_input: String,
}

impl EditorState {
    fn new() -> Self {
        Self {
            kits: scan_kits(&kits_dir()),
            path_input: String::new(),
        }
    }
}

const ACCENT: egui::Color32 = egui::Color32::from_rgb(0x3c, 0xb0, 0x43);
const PAD_IDLE: egui::Color32 = egui::Color32::from_rgb(0x2b, 0x2b, 0x30);
const PAD_LIT: egui::Color32 = egui::Color32::from_rgb(0x8f, 0xe3, 0x6b);

pub fn create(
    params: Arc<KitPlayerParams>,
    slot: Arc<KitSlot>,
    async_executor: AsyncExecutor<KitPlayer>,
) -> Option<Box<dyn Editor>> {
    let egui_state = params.editor_state.clone();
    create_egui_editor(
        egui_state,
        EditorState::new(),
        |_, _| {},
        move |ctx, setter, state| {
            // fichiers déposés sur la fenêtre
            let dropped: Vec<PathBuf> = ctx.input(|i| {
                i.raw
                    .dropped_files
                    .iter()
                    .filter_map(|f| f.path.clone())
                    .collect()
            });
            if let Some(p) = dropped.into_iter().next() {
                async_executor.execute_background(Task::LoadPath(p));
            }

            egui::CentralPanel::default().show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading(egui::RichText::new("make it drum").color(ACCENT).strong());
                    ui.label(
                        egui::RichText::new(format!("kit player v{}", env!("CARGO_PKG_VERSION")))
                            .weak(),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("un projet de");
                    ui.hyperlink_to("🥁 Masao « Saiya Geez » ↗", CREDIT_URL);
                    ui.label("·");
                    ui.hyperlink_to("code ↗", PROJECT_URL);
                });
                ui.add_space(6.0);

                // --- kits
                let current_name = params
                    .kit_state
                    .read()
                    .ok()
                    .and_then(|s| s.name.clone())
                    .unwrap_or_else(|| "— aucun kit —".to_string());
                ui.horizontal(|ui| {
                    ui.label("Kit");
                    egui::ComboBox::from_id_salt("kit-select")
                        .selected_text(&current_name)
                        .width(300.0)
                        .show_ui(ui, |ui| {
                            if state.kits.is_empty() {
                                ui.label(
                                    egui::RichText::new(format!(
                                        "Aucun .zip dans {}",
                                        kits_dir().display()
                                    ))
                                    .weak(),
                                );
                            }
                            for p in &state.kits {
                                let name = display_name(p);
                                if ui.selectable_label(name == current_name, &name).clicked() {
                                    async_executor.execute_background(Task::LoadPath(p.clone()));
                                }
                            }
                        });
                    if ui
                        .button("⟳")
                        .on_hover_text("Relire le dossier des kits")
                        .clicked()
                    {
                        state.kits = scan_kits(&kits_dir());
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Chemin");
                    ui.add(
                        egui::TextEdit::singleline(&mut state.path_input)
                            .desired_width(300.0)
                            .hint_text("…/makeitdrum_pack.zip ou dossier de WAV"),
                    );
                    if ui.button("Charger").clicked() && !state.path_input.trim().is_empty() {
                        async_executor.execute_background(Task::LoadPath(PathBuf::from(
                            state.path_input.trim(),
                        )));
                    }
                });
                ui.label(egui::RichText::new(slot.status()).small().weak());
                ui.add_space(6.0);

                // --- gain
                ui.horizontal(|ui| {
                    ui.label("Gain");
                    ui.add(widgets::ParamSlider::for_param(&params.gain, setter));
                });
                ui.add_space(8.0);

                // --- pads
                let playing = slot.playing();
                let labels: Vec<(String, u8)> = slot
                    .try_current()
                    .flatten()
                    .map(|k| k.pads.iter().map(|p| (p.label.clone(), p.note)).collect())
                    .unwrap_or_default();
                let avail = ui.available_width();
                let pad_w = ((avail - 4.0 * 6.0) / 5.0).max(60.0);
                egui::Grid::new("pads").spacing([6.0, 6.0]).show(ui, |ui| {
                    for i in 0..PAD_COUNT {
                        let lit = playing & (1 << i) != 0;
                        let (label, note) = labels
                            .get(i)
                            .cloned()
                            .unwrap_or_else(|| (format!("{:02}", i + 1), 36 + i as u8));
                        let text = egui::RichText::new(format!("{label}\n{note}")).color(if lit {
                            egui::Color32::BLACK
                        } else {
                            egui::Color32::LIGHT_GRAY
                        });
                        let button = egui::Button::new(text)
                            .fill(if lit { PAD_LIT } else { PAD_IDLE })
                            .min_size(egui::vec2(pad_w, 52.0));
                        if ui.add_enabled(labels.len() > i, button).clicked() {
                            slot.request_trigger(i);
                        }
                        if i % 5 == 4 {
                            ui.end_row();
                        }
                    }
                });
            });
            ctx.request_repaint_after(Duration::from_millis(40));
        },
    )
}
