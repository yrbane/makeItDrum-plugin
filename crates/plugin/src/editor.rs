//! Interface egui : selectbox de kits, chemin, gain, grille de pads.
//!
//! Glisser-déposer : un fichier audio (WAV, AIFF, MP3, FLAC, OGG) déposé
//! hors des pads devient un kit de 20 percussions, comme sur la page ; déposé
//! sur un pad, il remplace le son de ce pad. Un `.zip` ou un dossier
//! remplace le kit. Clic droit sur un pad : nouvelle variation. Les curseurs
//! Fidélité et Durée, relâchés, rejouent les pads générés avec leurs graines.
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use nih_plug::prelude::*;
use nih_plug_egui::{create_egui_editor, egui, widgets};

use make_it_drum_core::kit::PAD_COUNT;
use make_it_drum_core::wav::is_audio_file;

use crate::loader::{display_name, kits_dir, scan_kits};
use crate::params::KitPlayerParams;
use crate::slot::KitSlot;
use crate::{KitPlayer, Task};

pub const CREDIT_URL: &str = "https://beacons.ai/saiyageez";
pub const PROJECT_URL: &str = "https://github.com/yrbane/makeItDrum-plugin";

struct EditorState {
    kits: Vec<PathBuf>,
    path_input: String,
    /// Le cadrage déjà demandé au moteur ; un écart, curseur relâché, relance le rendu.
    shaping_sent: Option<make_it_drum_core::render::Shaping>,
}

impl EditorState {
    fn new() -> Self {
        Self {
            kits: scan_kits(&kits_dir()),
            path_input: String::new(),
            shaping_sent: None,
        }
    }
}

const ACCENT: egui::Color32 = egui::Color32::from_rgb(0x3c, 0xb0, 0x43);
const PAD_IDLE: egui::Color32 = egui::Color32::from_rgb(0x2b, 0x2b, 0x30);
const PAD_LIT: egui::Color32 = egui::Color32::from_rgb(0x8f, 0xe3, 0x6b);
const PAD_TARGET: egui::Color32 = egui::Color32::from_rgb(0x1f, 0x4a, 0x24);

/// Ce que l'utilisateur dépose sur la fenêtre.
#[derive(Debug, Clone, PartialEq)]
pub enum Drop {
    /// Un kit (`.zip` ou dossier) : remplace le kit courant.
    Kit(PathBuf),
    /// Un fichier audio sur un pad : remplace le son de ce pad.
    Sample { path: PathBuf, pad: usize },
    /// Un fichier audio hors des pads : devient un kit de 20 percussions.
    Generate(PathBuf),
    /// Autre chose : refusé.
    Unsupported(PathBuf),
}

/// Interprète un dépôt : `target` est le pad sous le curseur, s'il y en a un.
pub fn classify_drop(path: &Path, target: Option<usize>) -> Drop {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if path.is_dir() || name.to_ascii_lowercase().ends_with(".zip") {
        return Drop::Kit(path.to_path_buf());
    }
    if is_audio_file(&name) {
        return match target {
            Some(pad) => Drop::Sample {
                path: path.to_path_buf(),
                pad,
            },
            None => Drop::Generate(path.to_path_buf()),
        };
    }
    Drop::Unsupported(path.to_path_buf())
}

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
            // fichiers survolés / déposés sur la fenêtre, et position du curseur
            let (hovering_audio, dropped, pointer) = ctx.input(|i| {
                let hovering_audio = i.raw.hovered_files.iter().any(|f| {
                    f.path
                        .as_ref()
                        .map(|p| is_audio_file(&p.to_string_lossy()))
                        .unwrap_or(false)
                });
                let dropped: Option<PathBuf> =
                    i.raw.dropped_files.iter().find_map(|f| f.path.clone());
                (hovering_audio, dropped, i.pointer.latest_pos())
            });
            let current_kit = slot.current();

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
                    ui.hyperlink_to("Masao « Saiya Geez » ↗", CREDIT_URL);
                    ui.label("·");
                    ui.hyperlink_to("code ↗", PROJECT_URL);
                });
                ui.add_space(6.0);

                // --- zone de dépôt : un son devient un kit
                let has_source = current_kit.as_ref().is_some_and(|k| k.has_source());
                let zone = egui::Frame::new()
                    .fill(if hovering_audio { PAD_TARGET } else { PAD_IDLE })
                    .stroke(egui::Stroke::new(
                        1.0_f32,
                        if hovering_audio { ACCENT } else { egui::Color32::DARK_GRAY },
                    ))
                    .corner_radius(6.0_f32)
                    .inner_margin(egui::Margin::symmetric(12, 10))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.vertical_centered(|ui| {
                            ui.label(
                                egui::RichText::new(
                                    "Déposez un son ici : il devient un kit de 20 percussions",
                                )
                                .strong(),
                            );
                            ui.label(
                                egui::RichText::new(
                                    "WAV, AIFF, MP3, FLAC, OGG — depuis le navigateur de Live ou le Finder. \
                                     Sur un pad : remplace ce pad seulement. Clic droit sur un pad : variation.",
                                )
                                .small()
                                .weak(),
                            );
                            ui.horizontal(|ui| {
                                ui.label("Fidélité")
                                    .on_hover_text("0 % : la page. 100 % : le son d'origine, presque tel quel");
                                ui.add(widgets::ParamSlider::for_param(&params.fidelity, setter));
                                ui.label("Durée")
                                    .on_hover_text("0 % : des percussions. 100 % : étirées vers la durée du son");
                                ui.add(widgets::ParamSlider::for_param(&params.length, setter));
                            });
                            if has_source
                                && ui
                                    .button("Nouvelles variations des 20 pads")
                                    .on_hover_text("Rejoue chaque recette avec une nouvelle graine")
                                    .clicked()
                            {
                                async_executor.execute_background(Task::Regenerate(None));
                            }
                        });
                    });
                // curseurs relâchés et cadrage changé : rejouer les pads avec leurs graines
                let shaping = params.shaping();
                let pointer_down = ctx.input(|i| i.pointer.any_down());
                if state.shaping_sent.is_none() {
                    state.shaping_sent = Some(shaping);
                } else if !pointer_down && state.shaping_sent != Some(shaping) {
                    state.shaping_sent = Some(shaping);
                    if has_source {
                        async_executor.execute_background(Task::Reshape);
                    }
                }
                let _ = zone;
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
                let pads: Vec<Option<(String, u8)>> = (0..PAD_COUNT)
                    .map(|i| {
                        current_kit
                            .as_ref()
                            .and_then(|k| k.pads.get(i))
                            .filter(|p| !p.is_empty())
                            .map(|p| (p.label.clone(), p.note))
                    })
                    .collect();
                let avail = ui.available_width();
                let pad_w = ((avail - 4.0 * 6.0) / 5.0).max(60.0);
                let mut target: Option<usize> = None;
                egui::Grid::new("pads").spacing([6.0, 6.0]).show(ui, |ui| {
                    for (i, pad) in pads.iter().enumerate() {
                        let lit = playing & (1 << i) != 0;
                        let (label, note) = pad
                            .clone()
                            .unwrap_or_else(|| ("—".to_string(), 36 + i as u8));
                        let text = egui::RichText::new(format!("{label}\n{note}")).color(if lit {
                            egui::Color32::BLACK
                        } else {
                            egui::Color32::LIGHT_GRAY
                        });
                        let button = egui::Button::new(text)
                            .fill(if lit { PAD_LIT } else { PAD_IDLE })
                            .min_size(egui::vec2(pad_w, 58.0));
                        let response = ui
                            .add_enabled(pad.is_some(), button)
                            .on_hover_text(if has_source {
                                "Clic : jouer · clic droit : nouvelle variation"
                            } else {
                                "Clic : jouer"
                            });
                        if response.clicked() {
                            slot.request_trigger(i);
                        }
                        if response.secondary_clicked() && has_source {
                            async_executor.execute_background(Task::Regenerate(Some(i)));
                        }
                        if pointer.is_some_and(|p| response.rect.contains(p)) {
                            target = Some(i);
                            // pendant un drag audio, le pad visé s'entoure d'accent
                            if hovering_audio {
                                ui.painter().rect(
                                    response.rect,
                                    4.0_f32,
                                    PAD_TARGET.gamma_multiply(0.6),
                                    egui::Stroke::new(2.0_f32, ACCENT),
                                    egui::StrokeKind::Inside,
                                );
                            }
                        }
                        if i % 5 == 4 {
                            ui.end_row();
                        }
                    }
                });

                if let Some(path) = dropped {
                    nih_log!("dépôt : {} (pad visé : {target:?})", path.display());
                    match classify_drop(&path, target) {
                        Drop::Kit(p) => async_executor.execute_background(Task::LoadPath(p)),
                        Drop::Sample { path, pad } => {
                            async_executor.execute_background(Task::DropSample { pad, path })
                        }
                        Drop::Generate(p) => async_executor.execute_background(Task::Generate(p)),
                        Drop::Unsupported(p) => {
                            let name = p.file_name().unwrap_or_default().to_string_lossy();
                            nih_log!("dépôt refusé, format non pris en charge : {name}");
                            slot.set_status(format!("Format non pris en charge : {name}"));
                        }
                    }
                }
            });
            ctx.request_repaint_after(Duration::from_millis(40));
        },
    )
}
