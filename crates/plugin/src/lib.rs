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
pub mod slot;

use std::num::NonZeroU32;
use std::path::PathBuf;
use std::sync::Arc;

use nih_plug::prelude::*;

use make_it_drum_core::engine::Engine;
use make_it_drum_core::kit::PAD_COUNT;

use loader::{
    first_kit, generate_from_path, kits_dir, load_from_path, load_from_state, regenerate,
    replace_pad, reshape,
};
use params::KitPlayerParams;
use slot::KitSlot;

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
    /// Recharger le kit décrit par l'état persistant (ouverture de projet).
    Restore,
}

pub struct KitPlayer {
    params: Arc<KitPlayerParams>,
    slot: Arc<KitSlot>,
    engine: Engine,
    seen_generation: u64,
    loaded_fingerprint: String,
}

impl Default for KitPlayer {
    fn default() -> Self {
        Self {
            params: Arc::new(KitPlayerParams::default()),
            slot: Arc::new(KitSlot::new()),
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

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: None,
        main_output_channels: NonZeroU32::new(2),
        aux_input_ports: &[],
        aux_output_ports: &[],
        names: PortNames::const_default(),
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
        Box::new(move |task| {
            let dropped = matches!(
                task,
                Task::DropSample { .. } | Task::Generate(_) | Task::Regenerate(_) | Task::Reshape
            );
            let shaping = params.shaping();
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
        editor::create(self.params.clone(), self.slot.clone(), async_executor)
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        context: &mut impl InitContext<Self>,
    ) -> bool {
        self.engine.set_sample_rate(buffer_config.sample_rate);
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
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
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
                if let Some(NoteEvent::NoteOn { note, velocity, .. }) = next_event {
                    self.engine.note_on(note, velocity);
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

        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            let gain = self.params.gain.smoothed.next();
            *l *= gain;
            *r *= gain;
        }
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
