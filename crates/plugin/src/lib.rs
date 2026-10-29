//! make it drum — kit player : un instrument VST3/CLAP qui joue les kits de
//! 20 percussions produits par la page make it drum, depuis le MIDI.
// `nih_export_vst3!` (macro vst3_com::vtable) déclenche ce lint, renommé selon la version de rustc.
#![allow(unknown_lints)]
#![allow(
    semicolon_in_expressions_from_macros,
    semicolon_in_expressions_from_non_local_macros
)]
pub mod capture;
pub mod editor;
pub mod export;
pub mod history;
pub mod loader;
pub mod params;
pub mod potard;
pub mod slot;
pub mod variations;

use std::num::NonZeroU32;
use std::path::PathBuf;
use std::sync::Arc;

use nih_plug::prelude::*;

use make_it_drum_core::engine::Engine;
use make_it_drum_core::kit::PAD_COUNT;

use capture::Recorder;
use history::History;
use loader::{
    exports_dir, first_kit, generate_from_path, generate_from_sample, kits_dir, load_from_path,
    load_from_state, regenerate, regenerate_with_seeds, rename_pad, replace_pad, reshape, seeds_of,
    set_cut_all, set_settings,
};
use make_it_drum_core::kit::PadSettings;
use params::KitPlayerParams;
use slot::KitSlot;
use variations::Variations;

/// Tâches exécutées hors du thread audio.
pub enum Task {
    /// Charger un kit depuis un fichier ou un dossier (GUI).
    LoadPath(PathBuf),
    /// Mettre un fichier audio déposé sur le pad `pad` du kit courant (GUI).
    DropSample { pad: usize, path: PathBuf },
    /// Fabriquer un kit de 20 percussions à partir d'un fichier audio (GUI).
    Generate(PathBuf),
    /// Nouvelle variation d'un pad, ou de tous (`None`), depuis le son source (GUI).
    Regenerate(Option<usize>),
    /// Rejouer les pads générés avec leurs graines et les réglages Fidélité / Durée courants (GUI).
    Reshape,
    /// Changer les réglages de lecture d'un pad (GUI).
    SetSettings { pad: usize, settings: PadSettings },
    /// Renommer un pad (GUI).
    RenamePad { pad: usize, label: String },
    /// Mettre « cut itself » et/ou « cut by all » sur tous les pads (GUI).
    SetCutAll {
        cut_self: Option<bool>,
        cut_by_all: Option<bool>,
    },
    /// Revenir à la série de variations précédente (GUI).
    VariationsBack,
    /// Revenir à la série de variations suivante (GUI).
    VariationsForward,
    /// Revenir au kit précédent (GUI).
    Undo,
    /// Enregistrer le kit dans le dossier des kits sous ce nom (GUI).
    SaveKit(String),
    /// Exporter un WAV par pad dans le dossier des exports (GUI).
    ExportWavs(String),
    /// Exporter un Drum Rack de Live (.adg + WAV) dans le dossier des exports (GUI).
    ExportDrumRack(String),
    /// Fabriquer un kit à partir de la capture audio terminée (GUI).
    Capture,
    /// Recharger le kit décrit par l'état persistant (ouverture de projet).
    Restore,
}

pub struct KitPlayer {
    params: Arc<KitPlayerParams>,
    slot: Arc<KitSlot>,
    history: Arc<History>,
    recorder: Arc<Recorder>,
    variations: Arc<Variations>,
    engine: Engine,
    seen_generation: u64,
    loaded_fingerprint: String,
}

impl Default for KitPlayer {
    fn default() -> Self {
        Self {
            params: Arc::new(KitPlayerParams::default()),
            slot: Arc::new(KitSlot::new()),
            history: Arc::new(History::new()),
            recorder: Arc::new(Recorder::new()),
            variations: Arc::new(Variations::new()),
            engine: Engine::new(44100.0),
            seen_generation: 0,
            loaded_fingerprint: String::new(),
        }
    }
}

impl Plugin for KitPlayer {
    const NAME: &'static str = "make it drum";
    const VENDOR: &'static str = "Masao « Saiya Geez »";
    const URL: &'static str = "https://github.com/yrbane/makeItDrum-plugin";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    // une entrée auxiliaire (sidechain) : la source de la capture audio
    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: None,
        main_output_channels: NonZeroU32::new(2),
        aux_input_ports: &[new_nonzero_u32(2)],
        aux_output_ports: &[],
        names: PortNames {
            layout: Some("Stéréo + capture"),
            main_input: None,
            main_output: Some("Sortie"),
            aux_inputs: &["Capture"],
            aux_outputs: &[],
        },
    }];
    const MIDI_INPUT: MidiConfig = MidiConfig::Basic;
    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type SysExMessage = ();
    type BackgroundTask = Task;

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn task_executor(&mut self) -> TaskExecutor<Self> {
        let params = self.params.clone();
        let slot = self.slot.clone();
        let history = self.history.clone();
        let recorder = self.recorder.clone();
        let variations = self.variations.clone();
        Box::new(move |task| {
            // ce que la tâche fait aux séries de variations
            let records_seeds = matches!(task, Task::Regenerate(_));
            let resets_seeds = matches!(
                task,
                Task::Generate(_) | Task::Capture | Task::LoadPath(_) | Task::Restore
            );
            // tâches qui écrivent leur propre statut, et celles qui méritent un « Annuler »
            let dropped = !matches!(task, Task::LoadPath(_) | Task::Restore | Task::Undo);
            let undoable = matches!(
                task,
                Task::DropSample { .. }
                    | Task::Generate(_)
                    | Task::Regenerate(_)
                    | Task::Reshape
                    | Task::VariationsBack
                    | Task::VariationsForward
                    | Task::Capture
                    | Task::LoadPath(_)
            );
            let shaping = params.shaping();
            let previous = undoable.then(|| {
                slot.current().map(|kit| {
                    let state = params
                        .kit_state
                        .read()
                        .map(|s| s.clone())
                        .unwrap_or_default();
                    (kit, state)
                })
            });
            let result = match task {
                Task::Generate(path) => {
                    let name = path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned();
                    slot.set_status(format!("Analyse de {name}…"));
                    generate_from_path(&path, shaping, |kit, i| {
                        slot.set_status(format!("Génération {}/{PAD_COUNT} — {}", i + 1, name));
                        slot.publish(Arc::new(kit.clone()));
                    })
                    .map(|(kit, state)| {
                        slot.set_status(format!("Kit « {} » généré depuis {name}", kit.name));
                        (kit, state)
                    })
                }
                Task::Regenerate(pad) => match slot.current() {
                    Some(current) => regenerate(&current, pad, shaping).map(|(kit, state)| {
                        slot.set_status(match pad {
                            Some(i) => format!("Pad {} : nouvelle variation", i + 1),
                            None => "20 nouvelles variations".to_string(),
                        });
                        (kit, state)
                    }),
                    None => Err(make_it_drum_core::kit::KitError::NoSource),
                },
                Task::Reshape => match slot.current() {
                    Some(current) if current.shaping != shaping => {
                        reshape(&current, shaping, |kit, i| {
                            slot.set_status(format!("Rendu {}/{PAD_COUNT}", i + 1));
                            slot.publish(Arc::new(kit.clone()));
                        })
                        .map(|(kit, state)| {
                            slot.set_status(format!(
                                "Fidélité {:.0} % · durée {:.0} %",
                                shaping.fidelity * 100.0,
                                shaping.length * 100.0
                            ));
                            (kit, state)
                        })
                    }
                    Some(_) => return,
                    None => Err(make_it_drum_core::kit::KitError::NoSource),
                },
                Task::SetSettings { pad, settings } => match slot.current() {
                    Some(current) => set_settings(&current, pad, settings),
                    None => Err(make_it_drum_core::kit::KitError::Empty),
                },
                Task::RenamePad { pad, label } => match slot.current() {
                    Some(current) => rename_pad(&current, pad, &label),
                    None => Err(make_it_drum_core::kit::KitError::Empty),
                },
                Task::SetCutAll {
                    cut_self,
                    cut_by_all,
                } => match slot.current() {
                    Some(current) => set_cut_all(&current, cut_self, cut_by_all),
                    None => Err(make_it_drum_core::kit::KitError::Empty),
                },
                Task::VariationsBack | Task::VariationsForward => {
                    let seeds = if matches!(task, Task::VariationsBack) {
                        variations.back()
                    } else {
                        variations.forward()
                    };
                    match (slot.current(), seeds) {
                        (Some(current), Some(seeds)) => {
                            regenerate_with_seeds(&current, &seeds, shaping, |kit, i| {
                                slot.set_status(format!("Rendu {}/{PAD_COUNT}", i + 1));
                                slot.publish(Arc::new(kit.clone()));
                            })
                            .map(|(kit, state)| {
                                let (c, n) = variations.position();
                                slot.set_status(format!("Variations {c}/{n}"));
                                (kit, state)
                            })
                        }
                        (None, _) => Err(make_it_drum_core::kit::KitError::NoSource),
                        (_, None) => return,
                    }
                }
                Task::Undo => match history.pop() {
                    Some((kit, state)) => {
                        slot.set_status(format!("Retour au kit « {} »", kit.name));
                        Ok((kit, state))
                    }
                    None => {
                        slot.set_status("Rien à annuler");
                        return;
                    }
                },
                Task::SaveKit(name) => match slot.current() {
                    Some(current) => {
                        match export::save_kit(&current, &kits_dir(), &name) {
                            Ok(path) => slot.set_status(format!("Enregistré : {}", path.display())),
                            Err(e) => slot.set_status(format!("Échec de l'enregistrement : {e}")),
                        }
                        return;
                    }
                    None => return,
                },
                Task::ExportWavs(name) => match slot.current() {
                    Some(current) => {
                        match export::export_wavs(&current, &exports_dir(), &name) {
                            Ok((folder, files)) => slot.set_status(format!(
                                "{} WAV dans {}",
                                files.len(),
                                folder.display()
                            )),
                            Err(e) => slot.set_status(format!("Échec de l'export : {e}")),
                        }
                        return;
                    }
                    None => return,
                },
                Task::ExportDrumRack(name) => match slot.current() {
                    Some(current) => {
                        match export::export_drum_rack(&current, &exports_dir(), &name) {
                            Ok(path) => slot.set_status(format!("Drum Rack : {}", path.display())),
                            Err(e) => slot.set_status(format!("Échec de l'export : {e}")),
                        }
                        return;
                    }
                    None => return,
                },
                Task::Capture => match recorder.take() {
                    Some(sample) => {
                        let peak = sample.data.iter().fold(0f32, |m, v| m.max(v.abs()));
                        if peak < 1e-4 {
                            slot.set_status(
                                "Capture silencieuse : routez une source vers l'entrée « Capture » du plugin",
                            );
                            return;
                        }
                        slot.set_status("Analyse de la capture…");
                        generate_from_sample("capture", &sample, shaping, |kit, i| {
                            slot.set_status(format!("Génération {}/{PAD_COUNT} — capture", i + 1));
                            slot.publish(Arc::new(kit.clone()));
                        })
                        .map(|(kit, state)| {
                            slot.set_status("Kit « capture » : 20 pads depuis l'entrée audio");
                            (kit, state)
                        })
                    }
                    None => return,
                },
                Task::LoadPath(path) => load_from_path(&path),
                Task::DropSample { pad, path } => {
                    let current = slot.current();
                    replace_pad(current.as_deref(), pad, &path).map(|(kit, state)| {
                        slot.set_status(format!(
                            "Pad {} ← {}",
                            pad + 1,
                            path.file_name().unwrap_or_default().to_string_lossy()
                        ));
                        (kit, state)
                    })
                }
                Task::Restore => {
                    let state = params
                        .kit_state
                        .read()
                        .map(|s| s.clone())
                        .unwrap_or_default();
                    if state.path.is_none() && state.zip_base64.is_none() {
                        // instance neuve : le premier kit du dossier, s'il y en a un
                        match first_kit(&kits_dir()) {
                            Some(path) => load_from_path(&path),
                            None => {
                                slot.set_status(format!(
                                    "Déposez un kit (.zip) dans {}",
                                    kits_dir().display()
                                ));
                                return;
                            }
                        }
                    } else {
                        load_from_state(&state)
                    }
                }
            };
            match &result {
                Ok((kit, _)) => nih_log!(
                    "tâche terminée : kit « {} », {} pads",
                    kit.name,
                    kit.pads.len()
                ),
                Err(e) => nih_log!("tâche échouée : {e}"),
            }
            match result {
                Ok((kit, state)) => {
                    if let Some(Some((prev_kit, prev_state))) = previous {
                        history.push(prev_kit, prev_state);
                    }
                    if resets_seeds {
                        variations.reset(seeds_of(&kit));
                    } else if records_seeds {
                        variations.record(seeds_of(&kit));
                    }
                    if !dropped {
                        let n = kit.pads.iter().filter(|p| !p.is_empty()).count();
                        slot.set_status(format!(
                            "Kit « {} » — {n} pad{}",
                            kit.name,
                            if n > 1 { "s" } else { "" }
                        ));
                    }
                    if let Ok(mut s) = params.kit_state.write() {
                        *s = state;
                    }
                    slot.publish(kit);
                }
                Err(e) => slot.set_status(format!("Échec du chargement : {e}")),
            }
        })
    }

    fn editor(&mut self, async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        editor::create(
            self.params.clone(),
            self.slot.clone(),
            self.history.clone(),
            self.recorder.clone(),
            self.variations.clone(),
            async_executor,
        )
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        context: &mut impl InitContext<Self>,
    ) -> bool {
        self.engine.set_sample_rate(buffer_config.sample_rate);
        self.slot.set_sample_rate(buffer_config.sample_rate);
        // l'état persistant a pu changer (ouverture de projet) : recharger si besoin
        let fingerprint = self.params.kit_fingerprint();
        if fingerprint != self.loaded_fingerprint {
            self.loaded_fingerprint = fingerprint;
            context.execute(Task::Restore);
        }
        true
    }

    fn reset(&mut self) {
        self.engine.stop_all();
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        // capture : l'entrée auxiliaire va au tampon d'enregistrement
        if self.recorder.is_recording() {
            if let Some(input) = aux.inputs.first_mut() {
                let channels = input.as_slice_immutable();
                self.recorder.push(channels);
            }
        }
        self.engine.set_melodic(self.params.melodic_pad_index());
        self.engine.set_gate(self.params.gate.value());
        // nouveau kit publié ? (clone d'Arc, sans allocation ni blocage)
        let generation = self.slot.generation();
        if generation != self.seen_generation {
            if let Some(current) = self.slot.try_current() {
                self.engine.set_kit(current);
                self.seen_generation = generation;
            }
        }
        // pads cliqués dans l'interface
        let triggers = self.slot.take_triggers();
        for pad in 0..PAD_COUNT {
            if triggers & (1 << pad) != 0 {
                self.engine.trigger_pad(pad, 1.0);
            }
        }

        let total = buffer.samples();
        let output = buffer.as_slice();
        let [left, right, ..] = output else {
            return ProcessStatus::Normal;
        };
        left.fill(0.0);
        right.fill(0.0);

        // rendu par tranches, coupées aux événements MIDI
        let mut next_event = context.next_event();
        let mut pos = 0;
        while pos < total {
            while next_event
                .as_ref()
                .is_some_and(|e| (e.timing() as usize) <= pos)
            {
                match next_event {
                    Some(NoteEvent::NoteOn { note, velocity, .. }) => {
                        self.engine.note_on(note, velocity)
                    }
                    Some(NoteEvent::NoteOff { note, .. }) => self.engine.note_off(note),
                    _ => {}
                }
                next_event = context.next_event();
            }
            let end = next_event
                .as_ref()
                .map(|e| (e.timing() as usize).clamp(pos + 1, total))
                .unwrap_or(total);
            self.engine
                .render(&mut left[pos..end], &mut right[pos..end]);
            pos = end;
        }

        let mut peak = 0.0f32;
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            let gain = self.params.gain.smoothed.next();
            *l *= gain;
            *r *= gain;
            peak = peak.max(l.abs()).max(r.abs());
        }
        self.slot.set_level(peak);
        self.slot.set_playing(self.engine.playing_mask());
        ProcessStatus::Normal
    }
}

impl ClapPlugin for KitPlayer {
    const CLAP_ID: &'static str = "io.github.yrbane.make-it-drum";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("Joue les kits de 20 percussions de make it drum depuis le MIDI");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::Instrument,
        ClapFeature::Drum,
        ClapFeature::Sampler,
        ClapFeature::Stereo,
    ];
}

impl Vst3Plugin for KitPlayer {
    const VST3_CLASS_ID: [u8; 16] = *b"MakeItDrumKitPlr";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[
        Vst3SubCategory::Instrument,
        Vst3SubCategory::Drum,
        Vst3SubCategory::Sampler,
    ];
}

nih_export_clap!(KitPlayer);
nih_export_vst3!(KitPlayer);
