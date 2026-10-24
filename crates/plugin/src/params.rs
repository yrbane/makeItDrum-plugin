//! Paramètres exposés à l'hôte et état persistant.
use std::sync::{Arc, RwLock};

use nih_plug::prelude::*;
use nih_plug_egui::EguiState;

use make_it_drum_core::render::Shaping;

use crate::loader::KitState;

#[derive(Params)]
pub struct KitPlayerParams {
    #[id = "gain"]
    pub gain: FloatParam,

    /// Fidélité au son d'origine : 0 = la page, 1 = le son presque tel quel.
    #[id = "fidelity"]
    pub fidelity: FloatParam,

    /// Durée : 0 = les percussions de la page, 1 = étirées vers la durée du son.
    #[id = "length"]
    pub length: FloatParam,

    /// Départ : point de départ dans le son source, 0 = début, 1 = fin.
    #[id = "start"]
    pub start: FloatParam,

    /// Frappes : sons joués à tour de rôle par pad (1 à 4).
    #[id = "round_robin"]
    pub round_robin: IntParam,

    /// Mode mélodique : un pad joué sur tout le clavier, transposé.
    #[id = "melodic"]
    pub melodic: BoolParam,

    /// Le pad du mode mélodique (1 à 20).
    #[id = "melodic_pad"]
    pub melodic_pad: IntParam,

    #[persist = "editor-state-v4"]
    pub editor_state: Arc<EguiState>,

    #[persist = "kit"]
    pub kit_state: Arc<RwLock<KitState>>,
}

impl Default for KitPlayerParams {
    fn default() -> Self {
        Self {
            gain: FloatParam::new(
                "Gain",
                util::db_to_gain(0.0),
                FloatRange::Skewed {
                    min: util::db_to_gain(-30.0),
                    max: util::db_to_gain(6.0),
                    factor: FloatRange::gain_skew_factor(-30.0, 6.0),
                },
            )
            .with_smoother(SmoothingStyle::Logarithmic(50.0))
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_gain_to_db(2))
            .with_string_to_value(formatters::s2v_f32_gain_to_db()),
            fidelity: FloatParam::new("Fidélité", 0.0, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_unit(" %")
                .with_value_to_string(formatters::v2s_f32_percentage(0))
                .with_string_to_value(formatters::s2v_f32_percentage()),
            length: FloatParam::new("Durée", 0.0, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_unit(" %")
                .with_value_to_string(formatters::v2s_f32_percentage(0))
                .with_string_to_value(formatters::s2v_f32_percentage()),
            start: FloatParam::new("Départ", 0.0, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_unit(" %")
                .with_value_to_string(formatters::v2s_f32_percentage(0))
                .with_string_to_value(formatters::s2v_f32_percentage()),
            round_robin: IntParam::new(
                "Frappes",
                1,
                IntRange::Linear {
                    min: 1,
                    max: make_it_drum_core::kit::MAX_ROUND_ROBIN as i32,
                },
            ),
            melodic: BoolParam::new("Mode mélodique", false),
            melodic_pad: IntParam::new(
                "Pad mélodique",
                1,
                IntRange::Linear {
                    min: 1,
                    max: make_it_drum_core::kit::PAD_COUNT as i32,
                },
            ),
            editor_state: EguiState::from_size(820, 540),
            kit_state: Arc::new(RwLock::new(KitState::default())),
        }
    }
}

impl KitPlayerParams {
    /// Le cadrage de la génération d'après les deux paramètres.
    pub fn shaping(&self) -> Shaping {
        Shaping::new(self.fidelity.value() as f64, self.length.value() as f64)
            .with_start(self.start.value() as f64)
    }

    /// Le pad du mode mélodique, si le mode est actif (index 0 à 19).
    pub fn melodic_pad_index(&self) -> Option<usize> {
        self.melodic
            .value()
            .then(|| (self.melodic_pad.value().max(1) - 1) as usize)
    }

    /// Empreinte de l'état persistant, pour savoir s'il faut recharger.
    pub fn kit_fingerprint(&self) -> String {
        let s = self.kit_state.read().map(|s| s.clone()).unwrap_or_default();
        format!(
            "{}|{}",
            s.path.unwrap_or_default(),
            s.zip_base64.map(|b| b.len()).unwrap_or(0)
        )
    }
}
