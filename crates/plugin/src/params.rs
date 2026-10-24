//! Paramètres exposés à l'hôte et état persistant.
use std::sync::{Arc, RwLock};

use nih_plug::prelude::*;
use nih_plug_egui::EguiState;

use crate::loader::KitState;

#[derive(Params)]
pub struct KitPlayerParams {
    #[id = "gain"]
    pub gain: FloatParam,

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
            editor_state: EguiState::from_size(600, 560),
            kit_state: Arc::new(RwLock::new(KitState::default())),
        }
    }
}

impl KitPlayerParams {
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
