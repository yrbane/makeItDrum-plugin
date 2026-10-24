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

    #[persist = "editor-state-v2"]
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
            editor_state: EguiState::from_size(600, 560),
            kit_state: Arc::new(RwLock::new(KitState::default())),
        }
    }
}

impl KitPlayerParams {
    /// Le cadrage de la génération d'après les deux paramètres.
    pub fn shaping(&self) -> Shaping {
        Shaping::new(self.fidelity.value() as f64, self.length.value() as f64)
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
