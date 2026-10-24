//! Les 20 recettes de percussion, port des données de la page : comment le
//! sample importé est découpé, filtré, ré-enveloppé et saturé. Aucun tirage
//! ici, les variations sont appliquées par le planificateur de rendu.
use crate::analysis::{clamp, Analysis};
use crate::dsp::FilterKind;

/// Fréquence dépendant de l'analyse du son.
pub type FreqFn = fn(&Analysis) -> f64;

#[derive(Debug, Clone, Copy)]
pub struct PitchBend {
    pub from: f64,
    pub to: f64,
    /// Durée du glissando ; `None` = 60 % de la durée totale.
    pub dur: Option<f64>,
}

#[derive(Debug, Clone, Copy)]
pub struct Layer {
    pub rate: f64,
    pub pitch_bend: Option<PitchBend>,
    pub filter: Option<FilterKind>,
    pub freq: FreqFn,
    pub q: f64,
    pub gain: f64,
    pub hold_mul: f64,
    pub decay_mul: f64,
    pub offset_max: f64,
    pub reverse: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct Sub {
    pub rate: f64,
    pub cutoff: FreqFn,
    pub drive: f64,
    pub gain: f64,
    pub decay_mul: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct Phaser {
    pub stages: usize,
    pub base_freq: FreqFn,
    pub spread: f64,
    pub sweep: bool,
    pub sweep_rate: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct Comb {
    pub delay_ms: f64,
    pub feedback: f64,
    pub mix: f64,
    pub invert: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct Sat {
    pub drive1: f64,
    pub asym1: f64,
    pub fold_amt: f64,
    pub drive2: f64,
    pub asym2: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct BassBoost {
    pub freq: f64,
    pub db: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct Crush {
    pub chance: f64,
    pub bits: u32,
    pub hold: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct Flavor {
    pub name: &'static str,
    pub attack: f64,
    pub hold: f64,
    pub decay: f64,
    pub curve_pow: f64,
    pub attack_click: f64,
    pub layers: &'static [Layer],
    pub sub: Option<Sub>,
    pub phaser: Option<Phaser>,
    pub comb: Option<Comb>,
    pub sat: Sat,
    pub bass_boost: Option<BassBoost>,
    pub rev_tail: f64,
    pub rev_decay: f64,
    pub rev_wet: f64,
    pub crush: Option<Crush>,
    pub out_gain: f64,
    pub peak: f64,
}

const L: Layer = Layer {
    rate: 1.0,
    pitch_bend: None,
    filter: None,
    freq: |_| 1000.0,
    q: 1.0,
    gain: 1.0,
    hold_mul: 1.0,
    decay_mul: 1.0,
    offset_max: 0.75,
    reverse: false,
};

use FilterKind::{Bandpass, Highpass, Lowpass};

/// Les 20 recettes, dans l'ordre des pads (BOOM = pad 1 … CHAOS = pad 20).
pub static FLAVORS: [Flavor; 20] = [
    // 1. BOOM grave — corps du sample très filtré passe-bas + renfort de basses
    Flavor {
        name: "BOOM",
        attack: 0.0008,
        hold: 0.02,
        decay: 0.5,
        curve_pow: 1.8,
        attack_click: 0.6,
        layers: &[
            Layer {
                rate: 0.28,
                filter: Some(Lowpass),
                freq: |a| clamp(a.fundamental * 4.0, 120.0, 500.0),
                q: 0.7,
                gain: 0.85,
                offset_max: 0.7,
                ..L
            },
            Layer {
                rate: 0.16,
                filter: Some(Bandpass),
                freq: |a| clamp(a.fundamental * 2.0, 60.0, 300.0),
                q: 1.1,
                gain: 0.4,
                decay_mul: 0.75,
                offset_max: 0.85,
                ..L
            },
        ],
        sub: Some(Sub {
            rate: 0.18,
            cutoff: |a| clamp(a.fundamental * 0.7, 40.0, 90.0),
            drive: 1.5,
            gain: 0.9,
            decay_mul: 1.1,
        }),
        phaser: None,
        comb: None,
        sat: Sat {
            drive1: 2.4,
            asym1: 0.16,
            fold_amt: 0.0,
            drive2: 1.8,
            asym2: 0.22,
        },
        bass_boost: Some(BassBoost {
            freq: 75.0,
            db: 6.0,
        }),
        rev_tail: 0.12,
        rev_decay: 2.1,
        rev_wet: 0.16,
        crush: None,
        out_gain: 1.4,
        peak: 0.97,
    },
    // 2. KICK punchy — transitoire net + fold
    Flavor {
        name: "KICK",
        attack: 0.0006,
        hold: 0.006,
        decay: 0.19,
        curve_pow: 2.0,
        attack_click: 0.8,
        layers: &[
            Layer {
                rate: 0.32,
                filter: Some(Lowpass),
                freq: |a| clamp(a.fundamental * 3.4, 150.0, 650.0),
                q: 0.8,
                gain: 0.9,
                offset_max: 0.7,
                ..L
            },
            Layer {
                rate: 1.7,
                filter: Some(Bandpass),
                freq: |a| 800.0 + a.brightness * 3000.0,
                q: 1.0,
                gain: 0.55,
                hold_mul: 0.0,
                decay_mul: 0.15,
                offset_max: 0.3,
                ..L
            },
        ],
        sub: Some(Sub {
            rate: 0.24,
            cutoff: |a| clamp(a.fundamental * 0.9, 45.0, 100.0),
            drive: 2.1,
            gain: 0.75,
            decay_mul: 0.8,
        }),
        phaser: None,
        comb: None,
        sat: Sat {
            drive1: 3.4,
            asym1: 0.2,
            fold_amt: 0.35,
            drive2: 2.2,
            asym2: 0.28,
        },
        bass_boost: Some(BassBoost {
            freq: 85.0,
            db: 7.0,
        }),
        rev_tail: 0.05,
        rev_decay: 1.6,
        rev_wet: 0.12,
        crush: Some(Crush {
            chance: 0.35,
            bits: 9,
            hold: 2,
        }),
        out_gain: 1.45,
        peak: 0.97,
    },
    // 3. THUD mat — mi-grave, léger comb pour un corps sourd
    Flavor {
        name: "THUD",
        attack: 0.0009,
        hold: 0.01,
        decay: 0.3,
        curve_pow: 1.7,
        attack_click: 0.6,
        layers: &[
            Layer {
                rate: 0.4,
                filter: Some(Lowpass),
                freq: |a| clamp(a.fundamental * 5.0, 180.0, 700.0),
                q: 0.7,
                gain: 0.85,
                offset_max: 0.75,
                ..L
            },
            Layer {
                rate: 0.55,
                filter: Some(Bandpass),
                freq: |a| clamp(a.mid_peak * 0.6, 250.0, 1200.0),
                q: 1.3,
                gain: 0.35,
                decay_mul: 0.6,
                offset_max: 0.6,
                ..L
            },
        ],
        sub: None,
        phaser: None,
        comb: Some(Comb {
            delay_ms: 9.0,
            feedback: 0.25,
            mix: 0.3,
            invert: false,
        }),
        sat: Sat {
            drive1: 2.8,
            asym1: 0.18,
            fold_amt: 0.0,
            drive2: 2.0,
            asym2: 0.24,
        },
        bass_boost: Some(BassBoost {
            freq: 100.0,
            db: 5.0,
        }),
        rev_tail: 0.14,
        rev_decay: 1.9,
        rev_wet: 0.18,
        crush: None,
        out_gain: 1.35,
        peak: 0.96,
    },
    // 4. SNAP tonale — corps + crack aigu, léger phaser
    Flavor {
        name: "SNAP",
        attack: 0.0006,
        hold: 0.004,
        decay: 0.1,
        curve_pow: 2.1,
        attack_click: 0.75,
        layers: &[
            Layer {
                rate: 0.55,
                filter: Some(Bandpass),
                freq: |a| clamp(a.fundamental * 1.4, 150.0, 320.0),
                q: 1.3,
                gain: 0.85,
                offset_max: 0.6,
                ..L
            },
            Layer {
                rate: 2.4,
                filter: Some(Highpass),
                freq: |a| 2600.0 + a.brightness * 3200.0,
                q: 0.9,
                gain: 0.5,
                decay_mul: 0.7,
                offset_max: 0.4,
                ..L
            },
        ],
        sub: None,
        phaser: Some(Phaser {
            stages: 2,
            base_freq: |a| clamp(a.mid_peak, 600.0, 2000.0),
            spread: 400.0,
            sweep: false,
            sweep_rate: 0.0,
        }),
        comb: None,
        sat: Sat {
            drive1: 2.6,
            asym1: 0.16,
            fold_amt: 0.0,
            drive2: 2.0,
            asym2: 0.24,
        },
        bass_boost: Some(BassBoost {
            freq: 130.0,
            db: 4.0,
        }),
        rev_tail: 0.09,
        rev_decay: 1.8,
        rev_wet: 0.2,
        crush: None,
        out_gain: 1.3,
        peak: 0.96,
    },
    // 5. CRACK sec — comb inversé (annulation de phase ciblée)
    Flavor {
        name: "CRACK",
        attack: 0.0005,
        hold: 0.003,
        decay: 0.13,
        curve_pow: 2.3,
        attack_click: 0.7,
        layers: &[
            Layer {
                rate: 2.0,
                filter: Some(Highpass),
                freq: |a| 3000.0 + a.brightness * 3500.0,
                q: 0.8,
                gain: 0.75,
                offset_max: 0.45,
                ..L
            },
            Layer {
                rate: 0.7,
                filter: Some(Bandpass),
                freq: |a| clamp(a.mid_peak, 700.0, 2400.0),
                q: 1.5,
                gain: 0.4,
                decay_mul: 0.6,
                offset_max: 0.6,
                ..L
            },
        ],
        sub: None,
        phaser: None,
        comb: Some(Comb {
            delay_ms: 3.2,
            feedback: 0.2,
            mix: 0.4,
            invert: true,
        }),
        sat: Sat {
            drive1: 3.0,
            asym1: 0.2,
            fold_amt: 0.2,
            drive2: 2.1,
            asym2: 0.26,
        },
        bass_boost: None,
        rev_tail: 0.08,
        rev_decay: 1.7,
        rev_wet: 0.16,
        crush: None,
        out_gain: 1.3,
        peak: 0.95,
    },
    // 6. BUZZ phasé — phaser à balayage marqué
    Flavor {
        name: "BUZZ",
        attack: 0.0008,
        hold: 0.01,
        decay: 0.24,
        curve_pow: 1.8,
        attack_click: 0.6,
        layers: &[
            Layer {
                rate: 1.5,
                filter: Some(Bandpass),
                freq: |a| clamp(a.mid_peak, 700.0, 2600.0),
                q: 1.5,
                gain: 0.8,
                offset_max: 0.65,
                ..L
            },
            Layer {
                rate: 0.9,
                filter: Some(Highpass),
                freq: |_| 1800.0,
                q: 0.8,
                gain: 0.4,
                decay_mul: 0.8,
                offset_max: 0.5,
                ..L
            },
        ],
        sub: None,
        phaser: Some(Phaser {
            stages: 4,
            base_freq: |a| clamp(a.mid_peak * 0.7, 400.0, 1400.0),
            spread: 600.0,
            sweep: true,
            sweep_rate: 2.4,
        }),
        comb: None,
        sat: Sat {
            drive1: 3.2,
            asym1: 0.22,
            fold_amt: 0.3,
            drive2: 2.0,
            asym2: 0.28,
        },
        bass_boost: None,
        rev_tail: 0.16,
        rev_decay: 1.8,
        rev_wet: 0.22,
        crush: None,
        out_gain: 1.3,
        peak: 0.95,
    },
    // 7. RING métallique — comb très court (résonance) + fold
    Flavor {
        name: "RING",
        attack: 0.0004,
        hold: 0.002,
        decay: 0.14,
        curve_pow: 1.9,
        attack_click: 0.65,
        layers: &[
            Layer {
                rate: 2.6,
                filter: Some(Highpass),
                freq: |a| 4000.0 + a.brightness * 4000.0,
                q: 1.0,
                gain: 0.75,
                offset_max: 0.5,
                ..L
            },
            Layer {
                rate: 3.4,
                filter: Some(Bandpass),
                freq: |a| clamp(a.centroid * 0.8, 1500.0, 6000.0),
                q: 2.2,
                gain: 0.5,
                decay_mul: 0.7,
                offset_max: 0.4,
                ..L
            },
        ],
        sub: None,
        phaser: None,
        comb: Some(Comb {
            delay_ms: 1.1,
            feedback: 0.45,
            mix: 0.5,
            invert: false,
        }),
        sat: Sat {
            drive1: 2.4,
            asym1: 0.1,
            fold_amt: 0.45,
            drive2: 1.8,
            asym2: 0.2,
        },
        bass_boost: None,
        rev_tail: 0.1,
        rev_decay: 1.6,
        rev_wet: 0.14,
        crush: None,
        out_gain: 1.15,
        peak: 0.9,
    },
    // 8. TICK ultra-court — transitoire quasi sec
    Flavor {
        name: "TICK",
        attack: 0.00012,
        hold: 0.001,
        decay: 0.02,
        curve_pow: 2.6,
        attack_click: 0.4,
        layers: &[Layer {
            rate: 3.6,
            filter: Some(Highpass),
            freq: |a| 5500.0 + a.brightness * 4500.0,
            q: 0.9,
            gain: 0.9,
            offset_max: 0.55,
            ..L
        }],
        sub: None,
        phaser: None,
        comb: None,
        sat: Sat {
            drive1: 1.8,
            asym1: 0.06,
            fold_amt: 0.0,
            drive2: 1.4,
            asym2: 0.1,
        },
        bass_boost: None,
        rev_tail: 0.0,
        rev_decay: 1.7,
        rev_wet: 0.22,
        crush: None,
        out_gain: 1.1,
        peak: 0.88,
    },
    // 9. GLASS clair — aigu, léger phaser
    Flavor {
        name: "GLASS",
        attack: 0.0004,
        hold: 0.006,
        decay: 0.08,
        curve_pow: 1.7,
        attack_click: 0.45,
        layers: &[Layer {
            rate: 2.1,
            filter: Some(Highpass),
            freq: |a| 3600.0 + a.brightness * 3600.0,
            q: 0.9,
            gain: 0.85,
            offset_max: 0.6,
            ..L
        }],
        sub: None,
        phaser: Some(Phaser {
            stages: 2,
            base_freq: |a| clamp(a.centroid, 2000.0, 6000.0),
            spread: 800.0,
            sweep: true,
            sweep_rate: 1.2,
        }),
        comb: None,
        sat: Sat {
            drive1: 1.7,
            asym1: 0.06,
            fold_amt: 0.0,
            drive2: 1.4,
            asym2: 0.1,
        },
        bass_boost: None,
        rev_tail: 0.1,
        rev_decay: 1.6,
        rev_wet: 0.22,
        crush: None,
        out_gain: 1.15,
        peak: 0.9,
    },
    // 10. WASH aéré — long, grande reverb, phaser lent
    Flavor {
        name: "WASH",
        attack: 0.0006,
        hold: 0.05,
        decay: 1.0,
        curve_pow: 0.7,
        attack_click: 0.35,
        layers: &[Layer {
            rate: 1.8,
            filter: Some(Highpass),
            freq: |a| 3000.0 + a.brightness * 3000.0,
            q: 0.8,
            gain: 0.85,
            offset_max: 0.6,
            ..L
        }],
        sub: None,
        phaser: Some(Phaser {
            stages: 3,
            base_freq: |a| clamp(a.centroid * 0.6, 1200.0, 3500.0),
            spread: 900.0,
            sweep: true,
            sweep_rate: 0.5,
        }),
        comb: None,
        sat: Sat {
            drive1: 1.6,
            asym1: 0.06,
            fold_amt: 0.0,
            drive2: 1.3,
            asym2: 0.08,
        },
        bass_boost: None,
        rev_tail: 0.55,
        rev_decay: 1.5,
        rev_wet: 0.4,
        crush: None,
        out_gain: 1.1,
        peak: 0.87,
    },
    // 11. BEND glissant — pitch-sweep + léger comb
    Flavor {
        name: "BEND",
        attack: 0.0008,
        hold: 0.003,
        decay: 0.17,
        curve_pow: 2.0,
        attack_click: 0.6,
        layers: &[Layer {
            pitch_bend: Some(PitchBend {
                from: 1.1,
                to: 2.6,
                dur: Some(0.15),
            }),
            filter: Some(Bandpass),
            freq: |a| clamp(a.mid_peak, 500.0, 3000.0),
            q: 1.4,
            gain: 0.85,
            offset_max: 0.7,
            ..L
        }],
        sub: None,
        phaser: None,
        comb: Some(Comb {
            delay_ms: 12.0,
            feedback: 0.3,
            mix: 0.28,
            invert: false,
        }),
        sat: Sat {
            drive1: 2.4,
            asym1: 0.16,
            fold_amt: 0.0,
            drive2: 1.9,
            asym2: 0.22,
        },
        bass_boost: None,
        rev_tail: 0.15,
        rev_decay: 1.7,
        rev_wet: 0.22,
        crush: None,
        out_gain: 1.25,
        peak: 0.94,
    },
    // 12. SWELL inversé — sample joué à l'envers, montée puis coupure nette
    Flavor {
        name: "SWELL",
        attack: 0.14,
        hold: 0.0,
        decay: 0.05,
        curve_pow: 1.0,
        attack_click: 0.15,
        layers: &[Layer {
            rate: 0.6,
            filter: Some(Bandpass),
            freq: |a| clamp(a.mid_peak, 400.0, 2200.0),
            q: 1.0,
            gain: 0.9,
            reverse: true,
            offset_max: 0.5,
            ..L
        }],
        sub: None,
        phaser: None,
        comb: None,
        sat: Sat {
            drive1: 1.9,
            asym1: 0.1,
            fold_amt: 0.0,
            drive2: 1.5,
            asym2: 0.12,
        },
        bass_boost: None,
        rev_tail: 0.25,
        rev_decay: 1.8,
        rev_wet: 0.3,
        crush: None,
        out_gain: 1.2,
        peak: 0.92,
    },
    // 13. STUTTER glitché — enveloppe très courte + comb feedback élevé + bitcrush
    Flavor {
        name: "STUTTER",
        attack: 0.0004,
        hold: 0.002,
        decay: 0.05,
        curve_pow: 2.5,
        attack_click: 0.7,
        layers: &[Layer {
            rate: 1.3,
            filter: Some(Bandpass),
            freq: |a| clamp(a.mid_peak, 400.0, 2600.0),
            q: 1.6,
            gain: 0.85,
            offset_max: 0.8,
            ..L
        }],
        sub: None,
        phaser: None,
        comb: Some(Comb {
            delay_ms: 6.0,
            feedback: 0.65,
            mix: 0.5,
            invert: false,
        }),
        sat: Sat {
            drive1: 3.0,
            asym1: 0.2,
            fold_amt: 0.4,
            drive2: 2.2,
            asym2: 0.3,
        },
        bass_boost: None,
        rev_tail: 0.06,
        rev_decay: 1.5,
        rev_wet: 0.12,
        crush: Some(Crush {
            chance: 0.85,
            bits: 5,
            hold: 3,
        }),
        out_gain: 1.3,
        peak: 0.95,
    },
    // 14. CRUSH replié — wavefolder + bitcrush lourd
    Flavor {
        name: "CRUSH",
        attack: 0.0006,
        hold: 0.004,
        decay: 0.15,
        curve_pow: 2.1,
        attack_click: 0.65,
        layers: &[Layer {
            rate: 0.85,
            filter: Some(Bandpass),
            freq: |a| clamp(a.mid_peak, 500.0, 2400.0),
            q: 1.2,
            gain: 0.9,
            offset_max: 0.7,
            ..L
        }],
        sub: None,
        phaser: None,
        comb: None,
        sat: Sat {
            drive1: 2.6,
            asym1: 0.18,
            fold_amt: 0.55,
            drive2: 2.4,
            asym2: 0.32,
        },
        bass_boost: None,
        rev_tail: 0.08,
        rev_decay: 1.6,
        rev_wet: 0.14,
        crush: Some(Crush {
            chance: 0.9,
            bits: 6,
            hold: 3,
        }),
        out_gain: 1.3,
        peak: 0.95,
    },
    // 15. PHASE ambiante — nombreux étages allpass, sweep lent, texture longue
    Flavor {
        name: "PHASE",
        attack: 0.0025,
        hold: 0.05,
        decay: 0.9,
        curve_pow: 0.9,
        attack_click: 0.3,
        layers: &[Layer {
            rate: 0.5,
            filter: Some(Bandpass),
            freq: |a| clamp(a.mid_peak * 0.8, 300.0, 1800.0),
            q: 1.0,
            gain: 0.85,
            offset_max: 0.6,
            ..L
        }],
        sub: None,
        phaser: Some(Phaser {
            stages: 6,
            base_freq: |a| clamp(a.fundamental * 3.0, 200.0, 900.0),
            spread: 700.0,
            sweep: true,
            sweep_rate: 0.35,
        }),
        comb: None,
        sat: Sat {
            drive1: 1.8,
            asym1: 0.08,
            fold_amt: 0.0,
            drive2: 1.4,
            asym2: 0.1,
        },
        bass_boost: None,
        rev_tail: 0.5,
        rev_decay: 1.6,
        rev_wet: 0.35,
        crush: None,
        out_gain: 1.15,
        peak: 0.9,
    },
    // 16. GROWL grave — très saturé et asymétrique
    Flavor {
        name: "GROWL",
        attack: 0.0012,
        hold: 0.015,
        decay: 0.34,
        curve_pow: 1.6,
        attack_click: 0.7,
        layers: &[Layer {
            rate: 0.3,
            filter: Some(Lowpass),
            freq: |a| clamp(a.fundamental * 3.5, 110.0, 450.0),
            q: 0.9,
            gain: 0.9,
            offset_max: 0.7,
            ..L
        }],
        sub: Some(Sub {
            rate: 0.2,
            cutoff: |a| clamp(a.fundamental * 0.65, 40.0, 85.0),
            drive: 2.6,
            gain: 1.0,
            decay_mul: 1.0,
        }),
        phaser: None,
        comb: None,
        sat: Sat {
            drive1: 4.2,
            asym1: 0.4,
            fold_amt: 0.15,
            drive2: 2.8,
            asym2: 0.42,
        },
        bass_boost: Some(BassBoost {
            freq: 70.0,
            db: 7.0,
        }),
        rev_tail: 0.1,
        rev_decay: 1.9,
        rev_wet: 0.15,
        crush: None,
        out_gain: 1.45,
        peak: 0.97,
    },
    // 17. PERC clic — transitoire net + comb léger
    Flavor {
        name: "PERC",
        attack: 0.0004,
        hold: 0.002,
        decay: 0.09,
        curve_pow: 2.2,
        attack_click: 0.75,
        layers: &[Layer {
            rate: 1.6,
            filter: Some(Bandpass),
            freq: |a| clamp(a.mid_peak, 500.0, 2800.0),
            q: 1.5,
            gain: 0.9,
            offset_max: 0.55,
            ..L
        }],
        sub: None,
        phaser: None,
        comb: Some(Comb {
            delay_ms: 5.0,
            feedback: 0.2,
            mix: 0.25,
            invert: false,
        }),
        sat: Sat {
            drive1: 2.3,
            asym1: 0.14,
            fold_amt: 0.0,
            drive2: 1.8,
            asym2: 0.2,
        },
        bass_boost: None,
        rev_tail: 0.09,
        rev_decay: 1.7,
        rev_wet: 0.18,
        crush: None,
        out_gain: 1.25,
        peak: 0.95,
    },
    // 18. BELL aérienne — résonance à Q élevé façon cloche, phaser subtil
    Flavor {
        name: "BELL",
        attack: 0.0015,
        hold: 0.01,
        decay: 0.6,
        curve_pow: 1.2,
        attack_click: 0.4,
        layers: &[Layer {
            rate: 1.1,
            filter: Some(Bandpass),
            freq: |a| clamp(a.mid_peak * 1.3, 600.0, 3200.0),
            q: 6.5,
            gain: 0.9,
            offset_max: 0.55,
            ..L
        }],
        sub: None,
        phaser: Some(Phaser {
            stages: 2,
            base_freq: |a| clamp(a.mid_peak, 600.0, 2400.0),
            spread: 500.0,
            sweep: true,
            sweep_rate: 0.8,
        }),
        comb: None,
        sat: Sat {
            drive1: 1.6,
            asym1: 0.06,
            fold_amt: 0.0,
            drive2: 1.3,
            asym2: 0.08,
        },
        bass_boost: None,
        rev_tail: 0.3,
        rev_decay: 1.7,
        rev_wet: 0.3,
        crush: None,
        out_gain: 1.15,
        peak: 0.9,
    },
    // 19. DARK wash — long, sombre, sub renforcé, phaser lent, grande reverb
    Flavor {
        name: "DARK",
        attack: 0.0018,
        hold: 0.06,
        decay: 1.2,
        curve_pow: 0.9,
        attack_click: 0.3,
        layers: &[Layer {
            rate: 0.35,
            filter: Some(Lowpass),
            freq: |a| clamp(a.fundamental * 3.0, 150.0, 600.0),
            q: 0.8,
            gain: 0.85,
            offset_max: 0.6,
            ..L
        }],
        sub: Some(Sub {
            rate: 0.2,
            cutoff: |a| clamp(a.fundamental * 0.7, 40.0, 90.0),
            drive: 1.6,
            gain: 0.7,
            decay_mul: 1.1,
        }),
        phaser: Some(Phaser {
            stages: 3,
            base_freq: |a| clamp(a.fundamental * 2.5, 150.0, 700.0),
            spread: 500.0,
            sweep: true,
            sweep_rate: 0.3,
        }),
        comb: None,
        sat: Sat {
            drive1: 2.0,
            asym1: 0.12,
            fold_amt: 0.0,
            drive2: 1.6,
            asym2: 0.16,
        },
        bass_boost: Some(BassBoost {
            freq: 80.0,
            db: 5.0,
        }),
        rev_tail: 0.7,
        rev_decay: 1.6,
        rev_wet: 0.4,
        crush: None,
        out_gain: 1.25,
        peak: 0.93,
    },
    // 20. CHAOS blast — combo agressif : fold + phaser + comb + bitcrush
    Flavor {
        name: "CHAOS",
        attack: 0.0004,
        hold: 0.004,
        decay: 0.13,
        curve_pow: 2.0,
        attack_click: 0.85,
        layers: &[
            Layer {
                rate: 1.4,
                filter: Some(Bandpass),
                freq: |a| clamp(a.mid_peak, 500.0, 2600.0),
                q: 1.4,
                gain: 0.85,
                offset_max: 0.7,
                ..L
            },
            Layer {
                rate: 2.8,
                filter: Some(Highpass),
                freq: |a| 3200.0 + a.brightness * 3500.0,
                q: 0.9,
                gain: 0.5,
                decay_mul: 0.5,
                offset_max: 0.4,
                ..L
            },
        ],
        sub: None,
        phaser: Some(Phaser {
            stages: 3,
            base_freq: |a| clamp(a.mid_peak * 0.9, 500.0, 1800.0),
            spread: 700.0,
            sweep: true,
            sweep_rate: 3.2,
        }),
        comb: Some(Comb {
            delay_ms: 4.5,
            feedback: 0.4,
            mix: 0.35,
            invert: true,
        }),
        sat: Sat {
            drive1: 3.6,
            asym1: 0.24,
            fold_amt: 0.5,
            drive2: 2.6,
            asym2: 0.34,
        },
        bass_boost: Some(BassBoost {
            freq: 95.0,
            db: 5.0,
        }),
        rev_tail: 0.12,
        rev_decay: 1.7,
        rev_wet: 0.18,
        crush: Some(Crush {
            chance: 0.6,
            bits: 6,
            hold: 2,
        }),
        out_gain: 1.4,
        peak: 0.97,
    },
];

/// La recette du pad `idx` (modulo 20).
pub fn flavor_at(idx: usize) -> &'static Flavor {
    &FLAVORS[idx % FLAVORS.len()]
}
