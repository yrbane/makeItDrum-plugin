//! Interface egui, dans l'esprit de la page : panneau beige, barre de titre
//! bleue, pads à petit écran LCD vert avec la forme d'onde du son.
//!
//! Glisser-déposer : un fichier audio déposé hors des pads devient un kit de
//! 20 percussions ; déposé sur un pad, il remplace le son de ce pad. Un `.zip`
//! ou un dossier remplace le kit. Clic sur un pad : le joue et le sélectionne ;
//! clic droit : nouvelle variation. Le panneau de droite règle le pad
//! sélectionné (nom, gain, hauteur, panoramique, chute, sens, étouffement,
//! verrou). Les curseurs Fidélité, Durée et Départ, relâchés, rejouent les
//! pads générés avec leurs graines.
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use nih_plug::prelude::*;
use nih_plug_egui::{create_egui_editor, egui, widgets};

use make_it_drum_core::kit::{Kit, Pad, PadSettings, MAX_ROUND_ROBIN, PAD_COUNT};
use make_it_drum_core::render::Shaping;
use make_it_drum_core::wav::{is_audio_file, Sample};

use crate::capture::Recorder;
use crate::history::History;
use crate::loader::{display_name, exports_dir, kits_dir, scan_kits};
use crate::params::KitPlayerParams;
use crate::slot::KitSlot;
use crate::{KitPlayer, Task};

pub const CREDIT_URL: &str = "https://beacons.ai/saiyageez";
pub const PROJECT_URL: &str = "https://github.com/yrbane/makeItDrum-plugin";

// ---- palette, celle de la page
const XP_BLUE_DARK: egui::Color32 = egui::Color32::from_rgb(0x0a, 0x24, 0x6a);
const XP_BLUE_MID: egui::Color32 = egui::Color32::from_rgb(0x3a, 0x7c, 0xe8);
const PANEL: egui::Color32 = egui::Color32::from_rgb(0xd6, 0xd3, 0xca);
const PANEL_LIGHT: egui::Color32 = egui::Color32::from_rgb(0xec, 0xeb, 0xe5);
const PANEL_DARK: egui::Color32 = egui::Color32::from_rgb(0xb7, 0xb3, 0xa6);
const PANEL_BORDER: egui::Color32 = egui::Color32::from_rgb(0x84, 0x80, 0x6f);
const INK: egui::Color32 = egui::Color32::from_rgb(0x1b, 0x1b, 0x1b);
const INK_SOFT: egui::Color32 = egui::Color32::from_rgb(0x4a, 0x48, 0x40);
const GREEN: egui::Color32 = egui::Color32::from_rgb(0x39, 0xff, 0x6a);
const GREEN_DIM: egui::Color32 = egui::Color32::from_rgb(0x2a, 0xa5, 0x48);
const LCD: egui::Color32 = egui::Color32::from_rgb(0x00, 0x1a, 0x06);
const LCD_LIT: egui::Color32 = egui::Color32::from_rgb(0x05, 0x3b, 0x13);
const PAD_FACE: egui::Color32 = egui::Color32::from_rgb(0x6b, 0x69, 0x5f);
const PAD_FACE_LIGHT: egui::Color32 = egui::Color32::from_rgb(0x8c, 0x8a, 0x80);
const PAD_EDGE: egui::Color32 = egui::Color32::from_rgb(0x47, 0x44, 0x3c);
const RECORD_RED: egui::Color32 = egui::Color32::from_rgb(0xd8, 0x2c, 0x2c);

const PAD_W: f32 = 96.0;
const PAD_H: f32 = 60.0;
const PAD_GAP: f32 = 8.0;
const WAVE_COLUMNS: usize = 40;

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

/// Nom d'une note MIDI, do médian = C3 (convention de Live) : 36 → « C1 ».
pub fn note_name(note: u8) -> String {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    let octave = note as i32 / 12 - 2;
    format!("{}{octave}", NAMES[note as usize % 12])
}

/// Forme d'onde réduite à `columns` colonnes (min, max), pour l'écran d'un pad.
pub fn waveform_columns(sample: &Sample, columns: usize) -> Vec<(f32, f32)> {
    let frames = sample.frames();
    if frames == 0 || columns == 0 {
        return Vec::new();
    }
    let ch = sample.channels.max(1) as usize;
    (0..columns)
        .map(|c| {
            let from = c * frames / columns;
            let to = ((c + 1) * frames / columns).max(from + 1).min(frames);
            let mut lo = f32::MAX;
            let mut hi = f32::MIN;
            for i in from..to {
                let v = sample.data[i * ch];
                lo = lo.min(v);
                hi = hi.max(v);
            }
            (lo.clamp(-1.0, 1.0), hi.clamp(-1.0, 1.0))
        })
        .collect()
}

struct EditorState {
    kits: Vec<PathBuf>,
    path_input: String,
    save_name: String,
    save_name_for: String,
    /// Le cadrage déjà demandé au moteur ; un écart, curseur relâché, relance le rendu.
    shaping_sent: Option<Shaping>,
    round_robin_sent: Option<usize>,
    selected: Option<usize>,
    rename: Option<(usize, String)>,
    /// Réglages en cours d'édition (pad, valeurs) tant que le curseur est tenu.
    editing: Option<(usize, PadSettings)>,
    waves: Vec<Vec<(f32, f32)>>,
    waves_generation: u64,
    capture_seconds: f32,
    show_path: bool,
}

impl EditorState {
    fn new() -> Self {
        Self {
            kits: scan_kits(&kits_dir()),
            path_input: String::new(),
            save_name: String::new(),
            save_name_for: String::new(),
            shaping_sent: None,
            round_robin_sent: None,
            selected: None,
            rename: None,
            editing: None,
            waves: Vec::new(),
            waves_generation: u64::MAX,
            capture_seconds: 2.0,
            show_path: false,
        }
    }

    fn refresh_waves(&mut self, kit: Option<&Kit>, generation: u64) {
        if self.waves_generation == generation {
            return;
        }
        self.waves_generation = generation;
        self.waves = (0..PAD_COUNT)
            .map(|i| {
                kit.and_then(|k| k.pads.get(i))
                    .map(|p| waveform_columns(&p.sample, WAVE_COLUMNS))
                    .unwrap_or_default()
            })
            .collect();
    }
}

fn style(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::light();
    visuals.panel_fill = PANEL;
    visuals.window_fill = PANEL_LIGHT;
    visuals.extreme_bg_color = egui::Color32::WHITE;
    visuals.faint_bg_color = PANEL_LIGHT;
    visuals.override_text_color = Some(INK);
    visuals.hyperlink_color = XP_BLUE_DARK;
    visuals.selection.bg_fill = XP_BLUE_MID;
    visuals.selection.stroke = egui::Stroke::new(1.0_f32, egui::Color32::WHITE);
    visuals.widgets.noninteractive.bg_fill = PANEL;
    visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0_f32, PANEL_BORDER);
    visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0_f32, INK);
    visuals.widgets.inactive.bg_fill = PANEL_LIGHT;
    visuals.widgets.inactive.weak_bg_fill = PANEL_LIGHT;
    visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0_f32, PANEL_BORDER);
    visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0_f32, INK);
    visuals.widgets.hovered.bg_fill = egui::Color32::WHITE;
    visuals.widgets.hovered.weak_bg_fill = egui::Color32::WHITE;
    visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0_f32, XP_BLUE_MID);
    visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.5_f32, INK);
    visuals.widgets.active.bg_fill = PANEL_DARK;
    visuals.widgets.active.weak_bg_fill = PANEL_DARK;
    visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0_f32, XP_BLUE_DARK);
    visuals.widgets.active.fg_stroke = egui::Stroke::new(1.5_f32, INK);
    visuals.widgets.open.bg_fill = PANEL_LIGHT;
    visuals.widgets.open.bg_stroke = egui::Stroke::new(1.0_f32, XP_BLUE_MID);
    let rounding = egui::CornerRadius::same(3);
    visuals.widgets.inactive.corner_radius = rounding;
    visuals.widgets.hovered.corner_radius = rounding;
    visuals.widgets.active.corner_radius = rounding;
    visuals.widgets.noninteractive.corner_radius = rounding;
    visuals.widgets.open.corner_radius = rounding;
    ctx.set_visuals(visuals);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(6.0, 5.0);
    style.spacing.button_padding = egui::vec2(8.0, 3.0);
    style.spacing.slider_width = 110.0;
    style.text_styles.insert(
        egui::TextStyle::Heading,
        egui::FontId::new(19.0, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Body,
        egui::FontId::new(12.5, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Button,
        egui::FontId::new(12.0, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Small,
        egui::FontId::new(10.0, egui::FontFamily::Proportional),
    );
    ctx.set_style(style);
}

/// Dessine l'écran LCD d'un pad (ou du panneau) avec sa forme d'onde.
fn draw_lcd(painter: &egui::Painter, rect: egui::Rect, wave: &[(f32, f32)], lit: bool) {
    painter.rect(
        rect,
        2.0_f32,
        if lit { LCD_LIT } else { LCD },
        egui::Stroke::new(1.0_f32, egui::Color32::BLACK),
        egui::StrokeKind::Inside,
    );
    if wave.is_empty() {
        return;
    }
    let inner = rect.shrink2(egui::vec2(3.0, 4.0));
    let mid = inner.center().y;
    let half = inner.height() / 2.0;
    let step = inner.width() / wave.len() as f32;
    let color = if lit { GREEN } else { GREEN_DIM };
    for (i, (lo, hi)) in wave.iter().enumerate() {
        let x = inner.left() + (i as f32 + 0.5) * step;
        let y0 = mid - hi * half;
        let y1 = mid - lo * half;
        painter.line_segment(
            [egui::pos2(x, y0.min(y1 - 1.0)), egui::pos2(x, y1)],
            egui::Stroke::new((step * 0.6).clamp(1.0, 3.0), color),
        );
    }
    // reflet de vitre
    painter.rect_filled(
        egui::Rect::from_min_max(
            rect.min,
            egui::pos2(rect.max.x, rect.min.y + rect.height() * 0.35),
        ),
        2.0_f32,
        egui::Color32::from_white_alpha(10),
    );
}

struct PadLook<'a> {
    index: usize,
    pad: Option<&'a Pad>,
    wave: &'a [(f32, f32)],
    playing: bool,
    selected: bool,
    drop_target: bool,
}

/// Un pad : face grise, écran, numéro, nom, note, badges. Renvoie la réponse.
fn draw_pad(ui: &mut egui::Ui, look: PadLook) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(PAD_W, PAD_H), egui::Sense::click_and_drag());
    let painter = ui.painter();
    let pressed = response.is_pointer_button_down_on();
    let face = if pressed {
        PAD_FACE
    } else if response.hovered() {
        PAD_FACE_LIGHT
    } else {
        PAD_FACE
    };
    painter.rect(
        rect.translate(egui::vec2(0.0, 2.0)),
        6.0_f32,
        egui::Color32::from_black_alpha(90),
        egui::Stroke::NONE,
        egui::StrokeKind::Inside,
    );
    painter.rect(
        rect,
        6.0_f32,
        face,
        egui::Stroke::new(1.0_f32, PAD_EDGE),
        egui::StrokeKind::Inside,
    );
    painter.line_segment(
        [
            egui::pos2(rect.left() + 6.0, rect.top() + 1.0),
            egui::pos2(rect.right() - 6.0, rect.top() + 1.0),
        ],
        egui::Stroke::new(1.0_f32, egui::Color32::from_white_alpha(70)),
    );
    let screen = rect.shrink(5.0);
    let empty = look.pad.is_none_or(|p| p.is_empty());
    draw_lcd(
        painter,
        screen,
        if empty { &[] } else { look.wave },
        look.playing,
    );

    let tiny = egui::FontId::new(8.5, egui::FontFamily::Proportional);
    let small = egui::FontId::new(10.5, egui::FontFamily::Proportional);
    let green = if look.playing { GREEN } else { GREEN_DIM };
    painter.text(
        screen.min + egui::vec2(4.0, 3.0),
        egui::Align2::LEFT_TOP,
        format!("{:02}", look.index + 1),
        tiny.clone(),
        green.gamma_multiply(0.8),
    );
    let note = look.pad.map(|p| p.note).unwrap_or(36 + look.index as u8);
    painter.text(
        egui::pos2(screen.right() - 4.0, screen.bottom() - 3.0),
        egui::Align2::RIGHT_BOTTOM,
        note_name(note),
        tiny.clone(),
        green.gamma_multiply(0.8),
    );
    let label = look
        .pad
        .filter(|p| !p.is_empty())
        .map(|p| p.label.clone())
        .unwrap_or_else(|| "—".to_string());
    let label: String = label.chars().take(11).collect();
    painter.text(
        egui::pos2(screen.left() + 4.0, screen.bottom() - 3.0),
        egui::Align2::LEFT_BOTTOM,
        label,
        small,
        if empty {
            GREEN_DIM.gamma_multiply(0.6)
        } else {
            GREEN
        },
    );
    if let Some(p) = look.pad {
        let mut badges = String::new();
        if p.settings.locked {
            badges.push('🔒');
        }
        if p.variant_count() > 1 {
            badges.push_str(&format!("×{}", p.variant_count()));
        }
        if p.settings.reverse {
            badges.push('◀');
        }
        if !badges.is_empty() {
            painter.text(
                egui::pos2(screen.right() - 4.0, screen.top() + 3.0),
                egui::Align2::RIGHT_TOP,
                badges,
                tiny,
                GREEN.gamma_multiply(0.9),
            );
        }
    }
    if look.playing {
        painter.rect(
            rect,
            6.0_f32,
            egui::Color32::TRANSPARENT,
            egui::Stroke::new(2.0_f32, GREEN),
            egui::StrokeKind::Outside,
        );
    }
    if look.selected {
        painter.rect(
            rect.expand(1.0),
            7.0_f32,
            egui::Color32::TRANSPARENT,
            egui::Stroke::new(2.0_f32, XP_BLUE_MID),
            egui::StrokeKind::Outside,
        );
    }
    if look.drop_target {
        painter.rect(
            rect,
            6.0_f32,
            GREEN.gamma_multiply(0.25),
            egui::Stroke::new(2.0_f32, GREEN),
            egui::StrokeKind::Inside,
        );
    }
    response
}

fn set_bool(setter: &ParamSetter, param: &BoolParam, value: bool) {
    setter.begin_set_parameter(param);
    setter.set_parameter(param, value);
    setter.end_set_parameter(param);
}

fn set_int(setter: &ParamSetter, param: &IntParam, value: i32) {
    setter.begin_set_parameter(param);
    setter.set_parameter(param, value);
    setter.end_set_parameter(param);
}

fn set_float(setter: &ParamSetter, param: &FloatParam, value: f32) {
    setter.begin_set_parameter(param);
    setter.set_parameter(param, value);
    setter.end_set_parameter(param);
}

pub fn create(
    params: Arc<KitPlayerParams>,
    slot: Arc<KitSlot>,
    history: Arc<History>,
    recorder: Arc<Recorder>,
    async_executor: AsyncExecutor<KitPlayer>,
) -> Option<Box<dyn Editor>> {
    let egui_state = params.editor_state.clone();
    create_egui_editor(
        egui_state,
        EditorState::new(),
        |ctx, _| style(ctx),
        move |ctx, setter, state| {
            // fichiers survolés / déposés sur la fenêtre, et position du curseur
            let (hovering_audio, dropped, pointer, pointer_down) = ctx.input(|i| {
                let hovering_audio = i.raw.hovered_files.iter().any(|f| {
                    f.path
                        .as_ref()
                        .map(|p| is_audio_file(&p.to_string_lossy()))
                        .unwrap_or(false)
                });
                let dropped: Option<PathBuf> =
                    i.raw.dropped_files.iter().find_map(|f| f.path.clone());
                (
                    hovering_audio,
                    dropped,
                    i.pointer.latest_pos(),
                    i.pointer.any_down(),
                )
            });
            let generation = slot.generation();
            let current_kit = slot.current();
            state.refresh_waves(current_kit.as_deref(), generation);
            let has_source = current_kit.as_ref().is_some_and(|k| k.has_source());
            let kit_name = current_kit
                .as_ref()
                .map(|k| k.name.clone())
                .unwrap_or_else(|| "— aucun kit —".to_string());
            if state.save_name_for != kit_name {
                state.save_name_for = kit_name.clone();
                state.save_name = kit_name.clone();
            }
            let playing = slot.playing();

            // ---- barre de titre
            egui::TopBottomPanel::top("title")
                .frame(
                    egui::Frame::new()
                        .fill(XP_BLUE_DARK)
                        .inner_margin(egui::Margin::symmetric(12, 6)),
                )
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("make it drum")
                                .heading()
                                .strong()
                                .color(egui::Color32::WHITE),
                        );
                        ui.label(
                            egui::RichText::new(format!(
                                "kit player v{}",
                                env!("CARGO_PKG_VERSION")
                            ))
                            .small()
                            .color(egui::Color32::from_rgb(0xcf, 0xe8, 0xff)),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.visuals_mut().hyperlink_color = egui::Color32::WHITE;
                            ui.hyperlink_to(egui::RichText::new("code ↗").small(), PROJECT_URL);
                            ui.label(egui::RichText::new("·").small().color(egui::Color32::WHITE));
                            ui.hyperlink_to(
                                egui::RichText::new("Masao « Saiya Geez » ↗").small(),
                                CREDIT_URL,
                            );
                            ui.label(
                                egui::RichText::new("un projet de")
                                    .small()
                                    .color(egui::Color32::from_rgb(0xcf, 0xe8, 0xff)),
                            );
                        });
                    });
                });

            // ---- pied : gain et statut
            egui::TopBottomPanel::bottom("status")
                .frame(
                    egui::Frame::new()
                        .fill(PANEL_DARK)
                        .inner_margin(egui::Margin::symmetric(10, 5)),
                )
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Gain");
                        ui.add(widgets::ParamSlider::for_param(&params.gain, setter));
                        ui.separator();
                        ui.label(egui::RichText::new(slot.status()).small().color(INK_SOFT));
                    });
                });

            let mut target: Option<usize> = None;
            egui::CentralPanel::default()
                .frame(
                    egui::Frame::new()
                        .fill(PANEL)
                        .inner_margin(egui::Margin::symmetric(10, 8)),
                )
                .show(ctx, |ui| {
                    // ---- barre de kit : choix, annuler, enregistrer, exporter
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("KIT").small().strong().color(XP_BLUE_DARK));
                        egui::ComboBox::from_id_salt("kit-select")
                            .selected_text(&kit_name)
                            .width(190.0)
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
                                    if ui.selectable_label(name == kit_name, &name).clicked() {
                                        async_executor
                                            .execute_background(Task::LoadPath(p.clone()));
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
                        if ui
                            .small_button("…")
                            .on_hover_text("Charger depuis un chemin")
                            .clicked()
                        {
                            state.show_path = !state.show_path;
                        }
                        ui.separator();
                        if ui
                            .add_enabled(!history.is_empty(), egui::Button::new("↶ Annuler"))
                            .on_hover_text("Revenir au kit précédent")
                            .clicked()
                        {
                            async_executor.execute_background(Task::Undo);
                        }
                        ui.separator();
                        ui.add(
                            egui::TextEdit::singleline(&mut state.save_name)
                                .desired_width(130.0)
                                .hint_text("nom du kit"),
                        );
                        let can_save = current_kit.is_some() && !state.save_name.trim().is_empty();
                        if ui
                            .add_enabled(can_save, egui::Button::new("Enregistrer"))
                            .on_hover_text(format!("En .kit.zip dans {}", kits_dir().display()))
                            .clicked()
                        {
                            async_executor
                                .execute_background(Task::SaveKit(state.save_name.trim().into()));
                            state.kits = scan_kits(&kits_dir());
                        }
                        ui.add_enabled_ui(can_save, |ui| {
                            ui.menu_button("Exporter ▾", |ui| {
                                ui.label(
                                    egui::RichText::new(format!("vers {}", exports_dir().display()))
                                        .small()
                                        .weak(),
                                );
                                if ui.button("Dossier de WAV (un par pad)").clicked() {
                                    async_executor.execute_background(Task::ExportWavs(
                                        state.save_name.trim().into(),
                                    ));
                                    ui.close_menu();
                                }
                                if ui
                                    .button("Drum Rack de Live (.adg + WAV)")
                                    .on_hover_text("À glisser sur une piste MIDI de Live")
                                    .clicked()
                                {
                                    async_executor.execute_background(Task::ExportDrumRack(
                                        state.save_name.trim().into(),
                                    ));
                                    ui.close_menu();
                                }
                            });
                        });
                    });
                    if state.show_path {
                        ui.horizontal(|ui| {
                            ui.label("Chemin");
                            ui.add(
                                egui::TextEdit::singleline(&mut state.path_input)
                                    .desired_width(420.0)
                                    .hint_text("…/makeitdrum_pack.zip ou dossier de WAV"),
                            );
                            if ui.button("Charger").clicked() && !state.path_input.trim().is_empty() {
                                async_executor.execute_background(Task::LoadPath(PathBuf::from(
                                    state.path_input.trim(),
                                )));
                            }
                        });
                    }
                    ui.add_space(4.0);

                    // ---- zone de dépôt + capture
                    let zone_fill = if hovering_audio { LCD_LIT } else { LCD };
                    egui::Frame::new()
                        .fill(zone_fill)
                        .stroke(egui::Stroke::new(
                            1.0_f32,
                            if hovering_audio { GREEN } else { egui::Color32::BLACK },
                        ))
                        .corner_radius(4.0_f32)
                        .inner_margin(egui::Margin::symmetric(12, 8))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    ui.label(
                                        egui::RichText::new(
                                            "Déposez un son ici : il devient un kit de 20 percussions",
                                        )
                                        .strong()
                                        .color(GREEN),
                                    );
                                    ui.label(
                                        egui::RichText::new(
                                            "WAV, AIFF, MP3, M4A, FLAC, OGG — navigateur de Live ou Finder. \
                                             Sur un pad : remplace ce pad seul.",
                                        )
                                        .small()
                                        .color(GREEN_DIM),
                                    );
                                });
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    let rate = slot.sample_rate();
                                    if recorder.is_recording() {
                                        let p = recorder.progress();
                                        ui.add(
                                            egui::ProgressBar::new(p)
                                                .desired_width(120.0)
                                                .fill(RECORD_RED)
                                                .text(egui::RichText::new("capture…").small()),
                                        );
                                        if ui.small_button("✖").clicked() {
                                            recorder.cancel();
                                        }
                                    } else {
                                        let button = egui::Button::new(
                                            egui::RichText::new(format!(
                                                "● Capturer {:.0} s",
                                                state.capture_seconds
                                            ))
                                            .color(egui::Color32::WHITE),
                                        )
                                        .fill(RECORD_RED);
                                        if ui
                                            .add_enabled(rate > 0.0, button)
                                            .on_hover_text(
                                                "Enregistre l'entrée « Capture » du plugin (sidechain) et en fait un kit",
                                            )
                                            .clicked()
                                        {
                                            recorder.arm(state.capture_seconds as f64, rate);
                                        }
                                        ui.add(
                                            egui::Slider::new(&mut state.capture_seconds, 0.5..=8.0)
                                                .show_value(false)
                                                .text(""),
                                        )
                                        .on_hover_text("Durée de la capture");
                                    }
                                });
                            });
                        });
                    if recorder.is_done() {
                        async_executor.execute_background(Task::Capture);
                    }
                    ui.add_space(4.0);

                    // ---- cadrage : fidélité, durée, départ
                    ui.horizontal(|ui| {
                        ui.label("Fidélité")
                            .on_hover_text("0 % : la page. 100 % : le son d'origine, presque tel quel");
                        ui.add(widgets::ParamSlider::for_param(&params.fidelity, setter));
                        ui.label("Durée")
                            .on_hover_text("0 % : des percussions. 100 % : étirées vers la durée du son");
                        ui.add(widgets::ParamSlider::for_param(&params.length, setter));
                        ui.label("Départ")
                            .on_hover_text("Point de départ dans le son source");
                        ui.add(widgets::ParamSlider::for_param(&params.start, setter));
                        if ui
                            .add_enabled(has_source, egui::Button::new("Au transitoire"))
                            .on_hover_text("Place le départ juste avant l'attaque la plus forte")
                            .clicked()
                        {
                            if let Some(onset) = current_kit.as_ref().and_then(|k| k.source_onset()) {
                                set_float(setter, &params.start, onset as f32);
                            }
                        }
                    });
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(has_source, egui::Button::new("Nouvelles variations"))
                            .on_hover_text("Rejoue chaque recette avec une nouvelle graine (sauf pads verrouillés)")
                            .clicked()
                        {
                            async_executor.execute_background(Task::Regenerate(None));
                        }
                        ui.separator();
                        ui.label("Frappes")
                            .on_hover_text("Sons joués à tour de rôle par pad, pour varier les frappes");
                        let mut rr = params.round_robin.value();
                        for n in 1..=MAX_ROUND_ROBIN as i32 {
                            if ui.selectable_label(rr == n, format!("{n}")).clicked() {
                                rr = n;
                                set_int(setter, &params.round_robin, n);
                            }
                        }
                        ui.separator();
                        let mut melodic = params.melodic.value();
                        if ui
                            .checkbox(&mut melodic, "Mélodique")
                            .on_hover_text("Un seul pad joué sur tout le clavier, transposé (do médian = tel quel)")
                            .changed()
                        {
                            set_bool(setter, &params.melodic, melodic);
                        }
                        let mut pad = params.melodic_pad.value();
                        let pad_label = |i: i32| {
                            current_kit
                                .as_ref()
                                .and_then(|k| k.pads.get((i - 1) as usize))
                                .filter(|p| !p.is_empty())
                                .map(|p| format!("{i} · {}", p.label))
                                .unwrap_or_else(|| format!("{i}"))
                        };
                        ui.add_enabled_ui(melodic, |ui| {
                            egui::ComboBox::from_id_salt("melodic-pad")
                                .selected_text(pad_label(pad))
                                .width(120.0)
                                .show_ui(ui, |ui| {
                                    for i in 1..=PAD_COUNT as i32 {
                                        if ui.selectable_value(&mut pad, i, pad_label(i)).changed() {
                                            set_int(setter, &params.melodic_pad, pad);
                                        }
                                    }
                                });
                        });
                    });
                    ui.add_space(6.0);

                    // ---- pads + panneau du pad
                    let pads_w = 5.0 * PAD_W + 4.0 * PAD_GAP;
                    let panel_w = (ui.available_width() - pads_w - 20.0).max(200.0);
                    let panel_h = 4.0 * PAD_H + 3.0 * PAD_GAP;
                    ui.horizontal_top(|ui| {
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing = egui::vec2(PAD_GAP, PAD_GAP);
                            for row in 0..4 {
                                ui.horizontal(|ui| {
                                    for col in 0..5 {
                                        let i = row * 5 + col;
                                        let pad = current_kit.as_ref().and_then(|k| k.pads.get(i));
                                        let response = draw_pad(
                                            ui,
                                            PadLook {
                                                index: i,
                                                pad,
                                                wave: state.waves.get(i).map(|w| w.as_slice()).unwrap_or(&[]),
                                                playing: playing & (1 << i) != 0,
                                                selected: state.selected == Some(i),
                                                drop_target: false,
                                            },
                                        );
                                        let sounding = pad.is_some_and(|p| !p.is_empty());
                                        let response = response.on_hover_text(if has_source {
                                            "Clic : jouer et sélectionner · clic droit : nouvelle variation"
                                        } else {
                                            "Clic : jouer et sélectionner"
                                        });
                                        if response.clicked() {
                                            state.selected = Some(i);
                                            if sounding {
                                                slot.request_trigger(i);
                                            }
                                        }
                                        if response.secondary_clicked() && has_source {
                                            state.selected = Some(i);
                                            async_executor.execute_background(Task::Regenerate(Some(i)));
                                        }
                                        if pointer.is_some_and(|p| response.rect.contains(p)) {
                                            target = Some(i);
                                            if hovering_audio {
                                                ui.painter().rect(
                                                    response.rect,
                                                    6.0_f32,
                                                    GREEN.gamma_multiply(0.25),
                                                    egui::Stroke::new(2.0_f32, GREEN),
                                                    egui::StrokeKind::Inside,
                                                );
                                            }
                                        }
                                    }
                                });
                            }
                        });
                        ui.add_space(8.0);
                        // panneau du pad sélectionné
                        ui.allocate_ui_with_layout(
                            egui::vec2(panel_w, panel_h),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                        egui::Frame::new()
                            .fill(PANEL_LIGHT)
                            .stroke(egui::Stroke::new(1.0_f32, PANEL_BORDER))
                            .corner_radius(5.0_f32)
                            .inner_margin(egui::Margin::symmetric(10, 8))
                            .show(ui, |ui| {
                                ui.set_width(panel_w - 20.0);
                                ui.set_min_height(panel_h - 16.0);
                                pad_panel(
                                    ui,
                                    state,
                                    &slot,
                                    &async_executor,
                                    current_kit.as_deref(),
                                    has_source,
                                );
                            });
                            },
                        );
                    });
                });

            // ---- réglages du pad en attente : envoyés quand le curseur est relâché
            if !pointer_down {
                if let Some((pad, settings)) = state.editing.take() {
                    async_executor.execute_background(Task::SetSettings { pad, settings });
                }
            }

            // ---- curseurs relâchés et cadrage ou frappes changés
            let shaping = params.shaping();
            if state.shaping_sent.is_none() {
                state.shaping_sent = Some(shaping);
            } else if !pointer_down && state.shaping_sent != Some(shaping) {
                state.shaping_sent = Some(shaping);
                if has_source {
                    async_executor.execute_background(Task::Reshape);
                }
            }
            let rr = params.round_robin.value().max(1) as usize;
            if state.round_robin_sent.is_none() {
                state.round_robin_sent = Some(rr);
            } else if !pointer_down && state.round_robin_sent != Some(rr) {
                state.round_robin_sent = Some(rr);
                if has_source {
                    async_executor.execute_background(Task::SetRoundRobin(rr));
                }
            }

            // ---- dépôt
            if let Some(path) = dropped {
                nih_log!("dépôt : {} (pad visé : {target:?})", path.display());
                match classify_drop(&path, target) {
                    Drop::Kit(p) => async_executor.execute_background(Task::LoadPath(p)),
                    Drop::Sample { path, pad } => {
                        state.selected = Some(pad);
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
            ctx.request_repaint_after(Duration::from_millis(40));
        },
    )
}

fn pad_panel(
    ui: &mut egui::Ui,
    state: &mut EditorState,
    slot: &KitSlot,
    async_executor: &AsyncExecutor<KitPlayer>,
    kit: Option<&Kit>,
    has_source: bool,
) {
    let Some(i) = state.selected else {
        ui.label(
            egui::RichText::new("PAD")
                .small()
                .strong()
                .color(XP_BLUE_DARK),
        );
        ui.add_space(6.0);
        ui.label(
            egui::RichText::new("Cliquez un pad pour le régler : nom, gain, hauteur, panoramique, chute, sens, groupe d'étouffement, verrou.")
                .color(INK_SOFT),
        );
        return;
    };
    let pad = kit.and_then(|k| k.pads.get(i));
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(format!("PAD {:02}", i + 1))
                .small()
                .strong()
                .color(XP_BLUE_DARK),
        );
        let note = pad.map(|p| p.note).unwrap_or(36 + i as u8);
        ui.label(
            egui::RichText::new(format!("note {note} · {}", note_name(note)))
                .small()
                .color(INK_SOFT),
        );
    });
    let Some(pad) = pad.filter(|p| !p.is_empty()) else {
        ui.add_space(6.0);
        ui.label(
            egui::RichText::new("Pad vide. Déposez un son dessus, ou générez un kit.")
                .color(INK_SOFT),
        );
        return;
    };

    // nom
    let editing_name = state.rename.as_ref().is_some_and(|(p, _)| *p == i);
    if !editing_name {
        state.rename = Some((i, pad.label.clone()));
    }
    let (_, name) = state.rename.as_mut().unwrap();
    let response = ui.add(
        egui::TextEdit::singleline(name)
            .desired_width(ui.available_width())
            .font(egui::TextStyle::Heading),
    );
    if (response.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)))
        && name.trim() != pad.label
        && !name.trim().is_empty()
    {
        async_executor.execute_background(Task::RenamePad {
            pad: i,
            label: name.trim().to_string(),
        });
    }

    // forme d'onde
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 54.0), egui::Sense::hover());
    let wave = waveform_columns(&pad.sample, 110);
    draw_lcd(ui.painter(), rect, &wave, slot.playing() & (1 << i) != 0);
    let secs = pad.sample.frames() as f32 / pad.sample.sample_rate.max(1) as f32;
    ui.label(
        egui::RichText::new(format!(
            "{:.2} s · {} Hz{}{}",
            secs,
            pad.sample.sample_rate,
            pad.seed
                .map(|s| format!(" · graine {s}"))
                .unwrap_or_default(),
            if pad.variant_count() > 1 {
                format!(" · {} frappes", pad.variant_count())
            } else {
                String::new()
            }
        ))
        .small()
        .color(INK_SOFT),
    );
    ui.add_space(2.0);

    // réglages
    let mut s = match state.editing {
        Some((p, s)) if p == i => s,
        _ => pad.settings,
    };
    let mut changed = false;
    changed |= ui
        .add(
            egui::Slider::new(&mut s.gain_db, -24.0..=12.0)
                .text("Gain")
                .suffix(" dB")
                .fixed_decimals(1),
        )
        .changed();
    changed |= ui
        .add(
            egui::Slider::new(&mut s.pitch, -24.0..=24.0)
                .text("Hauteur")
                .suffix(" st")
                .fixed_decimals(1),
        )
        .changed();
    changed |= ui
        .add(
            egui::Slider::new(&mut s.pan, -1.0..=1.0)
                .text("Pan")
                .fixed_decimals(2),
        )
        .changed();
    changed |= ui
        .add(
            egui::Slider::new(&mut s.decay, 0.05..=1.0)
                .text("Chute")
                .fixed_decimals(2),
        )
        .changed();
    ui.horizontal(|ui| {
        changed |= ui.checkbox(&mut s.reverse, "Inversé").changed();
        changed |= ui
            .checkbox(&mut s.locked, "Verrouillé")
            .on_hover_text("Variations, cadrage et frappes le laissent tel quel")
            .changed();
    });
    ui.horizontal(|ui| {
        ui.label("Étouffement")
            .on_hover_text("Les pads d'un même groupe se coupent entre eux");
        for g in 0..=PadSettings::MAX_CHOKE {
            let label = if g == 0 {
                "—".to_string()
            } else {
                g.to_string()
            };
            if ui.selectable_label(s.choke == g, label).clicked() {
                s.choke = g;
                changed = true;
            }
        }
    });
    if changed {
        state.editing = Some((i, s.clamped()));
    }
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        if ui.button("▶ Jouer").clicked() {
            slot.request_trigger(i);
        }
        if ui
            .add_enabled(
                has_source && pad.seed.is_some(),
                egui::Button::new("Variation"),
            )
            .on_hover_text("Nouvelle graine pour ce pad")
            .clicked()
        {
            async_executor.execute_background(Task::Regenerate(Some(i)));
        }
        if ui
            .add_enabled(
                !pad.settings.is_default(),
                egui::Button::new("Réinitialiser"),
            )
            .clicked()
        {
            state.editing = Some((i, PadSettings::default()));
        }
    });
}
