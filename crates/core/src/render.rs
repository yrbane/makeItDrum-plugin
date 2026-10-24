//! Le moteur : transformation du son importé, pas de génération. Port hors
//! ligne du renderer Web Audio de la page. Chaque percussion est construite
//! exclusivement à partir du sample (et de sa copie inversée) : découpage,
//! filtrage, ré-enveloppe, réseau de phase, comb, saturation, renfort de
//! basses, reverb, puis bitcrush et normalisation.
//!
//! Le rendu se fait en deux temps : `plan` tire tous les paramètres dans
//! l'ordre exact des tirages de la page (même graine, mêmes valeurs), puis
//! `render` exécute le graphe.
use crate::analysis::{analyze, clamp, Analysis};
use crate::curves::{curve_at, fold_curve, sat_curve, scaled_env, shape};
use crate::dsp::{
    bitcrush_in_place, convolve, normalize_peak_in_place, Biquad, Compressor, Delay, FilterKind,
};
use crate::flavors::Flavor;
use crate::prng::Random;
use crate::wav::Sample;

/// Le son importé, prêt pour le rendu : mono, sa copie inversée, son analyse.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceSample {
    pub data: Vec<f32>,
    pub reversed: Vec<f32>,
    pub sample_rate: u32,
    pub analysis: Analysis,
}

impl SourceSample {
    /// Mélange en mono (moyenne des canaux), inverse, analyse.
    pub fn prepare(sample: &Sample) -> Self {
        let ch = sample.channels.max(1) as usize;
        let data: Vec<f32> = sample
            .data
            .chunks(ch)
            .map(|frame| frame.iter().sum::<f32>() / ch as f32)
            .collect();
        let reversed: Vec<f32> = data.iter().rev().copied().collect();
        let analysis = analyze(&data, sample.sample_rate);
        Self {
            data,
            reversed,
            sample_rate: sample.sample_rate,
            analysis,
        }
    }

    /// Mono, pour l'embarquer dans un kit.
    pub fn to_sample(&self) -> Sample {
        Sample {
            data: self.data.clone(),
            channels: 1,
            sample_rate: self.sample_rate,
        }
    }

    pub fn duration(&self) -> f64 {
        self.data.len() as f64 / self.sample_rate as f64
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Timings {
    pub sr: f64,
    pub attack: f64,
    pub hold: f64,
    pub decay: f64,
    pub total: f64,
    pub rev_tail: f64,
    pub tail_pad: f64,
    pub length: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Rate {
    Const(f64),
    Ramp { from: f64, to: f64, dur: f64 },
}

impl Rate {
    fn at(&self, t: f64) -> f64 {
        match *self {
            Rate::Const(r) => r,
            Rate::Ramp { from, to, dur } => {
                if dur <= 0.0 || t >= dur {
                    to
                } else {
                    from + (to - from) * (t / dur)
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FilterSpec {
    pub kind: FilterKind,
    pub freq: f64,
    pub q: f64,
    pub gain_db: f64,
}

/// Une lecture du sample : vitesse, filtre, saturation, enveloppe, point de départ.
#[derive(Debug, Clone, PartialEq)]
pub struct SourcePlan {
    pub reverse: bool,
    pub rate: Rate,
    pub filter: Option<FilterSpec>,
    pub shaper: Option<Vec<f32>>,
    pub env: Vec<f32>,
    pub env_duration: f64,
    pub offset: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PhaserPlan {
    /// (fréquence du LFO, profondeur en Hz)
    pub lfo: Option<(f64, f64)>,
    /// (fréquence, Q) de chaque étage allpass
    pub stages: Vec<(f64, f64)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CombPlan {
    pub delay: f64,
    pub feedback: f64,
    pub mix: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SatPlan {
    pub drive1: Vec<f32>,
    pub fold: Option<Vec<f32>>,
    pub drive2: Vec<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReverbPlan {
    pub impulse: Vec<f32>,
    pub wet: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MasterPlan {
    pub out_gain: f64,
    pub ratio: f64,
    pub release: f64,
}

/// Tous les paramètres d'un rendu, tirés d'une graine.
#[derive(Debug, Clone, PartialEq)]
pub struct PadPlan {
    pub timings: Timings,
    pub sources: Vec<SourcePlan>,
    pub phaser: Option<PhaserPlan>,
    pub comb: Option<CombPlan>,
    pub sat: SatPlan,
    pub bass: Option<FilterSpec>,
    pub reverb: Option<ReverbPlan>,
    pub master: MasterPlan,
    /// (bits, hold) si le bitcrush a été tiré
    pub crush: Option<(u32, u32)>,
    pub peak: f64,
}

/// Les deux réglages qui cadrent la génération, en plus de la recette :
/// `fidelity` (0 à 1) rapproche chaque pad du son d'origine (hauteur vers 1,
/// filtres ouverts, saturation, comb, reverb, bitcrush et compression atténués,
/// lecture depuis le début) ; `length` (0 à 1) étire palier et chute vers la
/// durée du son (au plus `MAX_STRETCHED_TOTAL` s). À zéro : la page, telle quelle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shaping {
    pub fidelity: f64,
    pub length: f64,
}

impl Default for Shaping {
    fn default() -> Self {
        Self {
            fidelity: 0.0,
            length: 0.0,
        }
    }
}

impl Shaping {
    pub fn new(fidelity: f64, length: f64) -> Self {
        Self {
            fidelity: clamp(fidelity, 0.0, 1.0),
            length: clamp(length, 0.0, 1.0),
        }
    }

    pub fn is_default(&self) -> bool {
        self.fidelity == 0.0 && self.length == 0.0
    }
}

/// Durée maximale (attaque + palier + chute) qu'atteint `length` = 1.
pub const MAX_STRETCHED_TOTAL: f64 = 4.0;

#[inline]
fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// Mélange une table de WaveShaper avec l'identité : `t` = 1 ne déforme plus.
fn blend_identity(curve: &mut [f32], t: f64) {
    if t <= 0.0 {
        return;
    }
    let n = curve.len();
    for (i, v) in curve.iter_mut().enumerate() {
        let x = (i as f64 / (n - 1).max(1) as f64) * 2.0 - 1.0;
        *v = lerp(*v as f64, x, t) as f32;
    }
}

/// Rapproche un filtre de couche de la transparence.
fn open_filter(f: &mut FilterSpec, sr: f64, t: f64) {
    match f.kind {
        FilterKind::Lowpass => f.freq = lerp(f.freq, sr * 0.45, t),
        FilterKind::Highpass => f.freq = lerp(f.freq, 30.0, t),
        FilterKind::Bandpass => f.q = lerp(f.q, 0.25, t),
        _ => {}
    }
}

fn timings(cfg: &Flavor, sr: f64, rng: &mut Random) -> Timings {
    let attack = cfg.attack;
    let hold = rng.jitter(cfg.hold, 0.3);
    let decay = rng.jitter(cfg.decay, 0.25);
    let rev_tail = if cfg.rev_tail != 0.0 {
        rng.jitter(cfg.rev_tail, 0.3)
    } else {
        0.0
    };
    let tail_pad =
        0.1 + if cfg.phaser.is_some_and(|p| p.stages > 0) {
            0.08
        } else {
            0.0
        } + if cfg.comb.is_some() { 0.08 } else { 0.0 };
    Timings::finish(sr, attack, hold, decay, rev_tail, tail_pad)
}

impl Timings {
    fn finish(sr: f64, attack: f64, hold: f64, decay: f64, rev_tail: f64, tail_pad: f64) -> Self {
        let total = attack + hold + decay;
        let length = (((total + rev_tail + tail_pad) * sr).ceil() as usize).max(2);
        Timings {
            sr,
            attack,
            hold,
            decay,
            total,
            rev_tail,
            tail_pad,
            length,
        }
    }

    /// Étire palier et chute vers la durée du son, selon `length`.
    fn stretched(&self, duration: f64, length: f64) -> Self {
        let target = clamp(duration, self.total, MAX_STRETCHED_TOTAL).max(self.total);
        let body = (self.hold + self.decay).max(1e-6);
        let factor = lerp(1.0, (target - self.attack).max(0.0) / body, length);
        Timings::finish(
            self.sr,
            self.attack,
            self.hold * factor,
            self.decay * factor,
            self.rev_tail,
            self.tail_pad,
        )
    }
}

/// Tire tous les paramètres du rendu, dans l'ordre des tirages de la page.
pub fn plan(cfg: &Flavor, src: &SourceSample, rng: &mut Random) -> PadPlan {
    plan_with(cfg, src, rng, &Shaping::default())
}

/// Comme `plan`, cadré par `shaping`. Les tirages restent ceux de la page,
/// dans le même ordre : une graine donne la même « idée » de son quel que
/// soit le cadrage, seul le résultat est rapproché du son d'origine.
pub fn plan_with(cfg: &Flavor, src: &SourceSample, rng: &mut Random, shaping: &Shaping) -> PadPlan {
    let a = &src.analysis;
    let sr = src.sample_rate as f64;
    let duration = src.duration();
    let f = clamp(shaping.fidelity, 0.0, 1.0);
    let t = timings(cfg, sr, rng).stretched(duration, clamp(shaping.length, 0.0, 1.0));
    let mut sources = Vec::new();

    // transitoire d'attaque : le tout début du sample, passe-haut et saturé
    {
        let rate = rng.jitter(2.0, 0.25);
        let freq = clamp(
            rng.jitter(900.0 + a.brightness * 2500.0, 0.25),
            300.0,
            sr * 0.45,
        );
        let shaper = sat_curve(rng.jitter(3.4, 0.25), 0.22);
        let click_dur = rng.jitter(0.007, 0.3);
        let env = scaled_env(
            0.00015,
            0.0,
            click_dur,
            3.4,
            rng.jitter(cfg.attack_click, 0.2),
        );
        sources.push(SourcePlan {
            reverse: false,
            rate: Rate::Const(rate),
            filter: Some(FilterSpec {
                kind: FilterKind::Highpass,
                freq,
                q: 1.0,
                gain_db: 0.0,
            }),
            shaper: Some(shaper),
            env,
            env_duration: 0.00015 + click_dur,
            offset: 0.0,
        });
    }

    // couches directement issues du sample
    for layer in cfg.layers {
        let rate = match layer.pitch_bend {
            Some(pb) => {
                let dur = rng.jitter(pb.dur.unwrap_or(t.total * 0.6), 0.3);
                let from = rng.jitter(pb.from, 0.15);
                let to = rng.jitter(pb.to, 0.15);
                Rate::Ramp {
                    from: lerp(from, 1.0, f),
                    to: lerp(to, 1.0, f),
                    dur,
                }
            }
            None => Rate::Const(lerp(rng.jitter(layer.rate, 0.15), 1.0, f)),
        };
        let filter = layer.filter.map(|kind| {
            let freq = clamp(rng.jitter((layer.freq)(a), 0.22), 30.0, sr * 0.45);
            let q = rng.jitter(layer.q, 0.3);
            let mut spec = FilterSpec {
                kind,
                freq,
                q,
                gain_db: 0.0,
            };
            open_filter(&mut spec, sr, f);
            spec
        });
        let hold_l = t.hold * layer.hold_mul;
        let decay_l = t.decay * layer.decay_mul;
        let env = scaled_env(t.attack, hold_l, decay_l, cfg.curve_pow, layer.gain);
        let offset = rng.random_offset(duration, layer.offset_max) * (1.0 - f);
        sources.push(SourcePlan {
            // à forte fidélité, on n'inverse plus la lecture
            reverse: layer.reverse && f < 0.75,
            rate,
            filter,
            shaper: None,
            env,
            env_duration: t.attack + hold_l + decay_l,
            offset,
        });
    }

    // renfort de basses : copie passe-bas saturée
    if let Some(sub) = cfg.sub {
        // à forte fidélité, le renfort grave s'efface (vitesse vers 1, gain vers 0)
        let rate = lerp(rng.jitter(sub.rate, 0.15), 1.0, f);
        let freq = clamp(rng.jitter((sub.cutoff)(a), 0.2), 35.0, 220.0);
        let mut shaper = sat_curve(rng.jitter(sub.drive, 0.2), 0.32);
        blend_identity(&mut shaper, f);
        let decay_l = t.decay * sub.decay_mul;
        let env = scaled_env(
            t.attack,
            t.hold,
            decay_l,
            cfg.curve_pow,
            sub.gain * (1.0 - f),
        );
        let offset = rng.random_offset(duration, 0.7) * (1.0 - f);
        sources.push(SourcePlan {
            reverse: false,
            rate: Rate::Const(rate),
            filter: Some(FilterSpec {
                kind: FilterKind::Lowpass,
                freq,
                q: 1.0,
                gain_db: 0.0,
            }),
            shaper: Some(shaper),
            env,
            env_duration: t.attack + t.hold + decay_l,
            offset,
        });
    }

    let phaser = cfg.phaser.filter(|p| p.stages > 0).map(|ph| {
        let lfo = ph.sweep.then(|| {
            let rate = rng.jitter(ph.sweep_rate, 0.3);
            let depth = rng.jitter(ph.spread * 0.5, 0.3) * (1.0 - f);
            (rate, depth)
        });
        let stages = (0..ph.stages)
            .map(|s| {
                let freq = clamp(
                    rng.jitter((ph.base_freq)(a) * 1.55f64.powi(s as i32), 0.25),
                    60.0,
                    sr * 0.45,
                );
                let q = rng.jitter(0.7, 0.3);
                (freq, q)
            })
            .collect();
        PhaserPlan { lfo, stages }
    });

    let comb = cfg.comb.map(|c| {
        let delay = rng.jitter(c.delay_ms / 1000.0, 0.3);
        let feedback = rng.jitter(c.feedback, 0.3);
        let mix = (if c.invert { -1.0 } else { 1.0 }) * rng.jitter(c.mix, 0.3) * (1.0 - f);
        CombPlan {
            delay,
            feedback,
            mix,
        }
    });

    let sat = {
        let mut drive1 = sat_curve(rng.jitter(cfg.sat.drive1, 0.22), cfg.sat.asym1);
        let mut fold =
            (cfg.sat.fold_amt != 0.0).then(|| fold_curve(rng.jitter(cfg.sat.fold_amt, 0.25)));
        let mut drive2 = sat_curve(rng.jitter(cfg.sat.drive2, 0.22), cfg.sat.asym2);
        blend_identity(&mut drive1, f);
        if let Some(c) = fold.as_mut() {
            blend_identity(c, f);
        }
        blend_identity(&mut drive2, f);
        SatPlan {
            drive1,
            fold,
            drive2,
        }
    };

    let bass = cfg.bass_boost.map(|b| {
        let freq = rng.jitter(b.freq, 0.2);
        let gain_db = rng.jitter(b.db, 0.2) * (1.0 - f);
        FilterSpec {
            kind: FilterKind::Peaking,
            freq,
            q: 0.9,
            gain_db,
        }
    });

    let reverb = (t.rev_tail > 0.02).then(|| {
        let decay = rng.jitter(cfg.rev_decay, 0.25);
        let len = ((t.rev_tail * sr).floor() as usize).max(1);
        let impulse: Vec<f32> = (0..len)
            .map(|i| {
                ((rng.random() * 2.0 - 1.0) * (1.0 - i as f64 / len as f64).powf(decay)) as f32
            })
            .collect();
        let wet = rng.jitter(cfg.rev_wet, 0.3) * (1.0 - f);
        ReverbPlan { impulse, wet }
    });

    let out_gain = rng.jitter(cfg.out_gain, 0.15);
    let ratio = lerp(rng.jitter(9.0, 0.2), 1.0, f);
    let release = rng.jitter(0.09, 0.3);
    let master = MasterPlan {
        out_gain,
        ratio,
        release,
    };

    let crush = match cfg.crush {
        Some(c) if rng.chance(c.chance) && rng.chance(1.0) => {
            let bits = lerp(c.bits as f64, 16.0, f).round() as u32;
            let hold = lerp(c.hold as f64, 1.0, f).round() as u32;
            (bits < 16 || hold > 1).then_some((bits, hold))
        }
        _ => None,
    };

    PadPlan {
        timings: t,
        sources,
        phaser,
        comb,
        sat,
        bass,
        reverb,
        master,
        crush,
        peak: cfg.peak,
    }
}

#[inline]
fn read_buffer(buf: &[f32], pos: f64) -> f64 {
    let n = buf.len();
    if pos < 0.0 || n == 0 {
        return 0.0;
    }
    let i = pos.floor() as usize;
    if i >= n {
        return 0.0;
    }
    let frac = pos - i as f64;
    let a = buf[i] as f64;
    let b = buf[(i + 1).min(n - 1)] as f64;
    a + (b - a) * frac
}

/// Durée minimale d'un retard pris dans une boucle de feedback : un quantum
/// Web Audio (128 échantillons), comme dans le navigateur.
const CYCLE_MIN_DELAY: f64 = 128.0;
const MAX_DELAY_S: f64 = 0.05;

/// Exécute le graphe planifié. Mono, à la fréquence du sample.
pub fn render(plan: &PadPlan, src: &SourceSample) -> Vec<f32> {
    let sr = plan.timings.sr;
    let len = plan.timings.length;
    let mut mix = vec![0.0f64; len];

    for s in &plan.sources {
        let buf = if s.reverse { &src.reversed } else { &src.data };
        let mut filter = s
            .filter
            .as_ref()
            .map(|f| Biquad::new(f.kind, f.freq, f.q, f.gain_db, sr));
        let mut pos = s.offset * sr;
        let frames = buf.len() as f64;
        for (i, out) in mix.iter_mut().enumerate() {
            let t = i as f64 / sr;
            let env = curve_at(&s.env, s.env_duration, t);
            let v = if pos < frames {
                read_buffer(buf, pos)
            } else {
                0.0
            };
            pos += s.rate.at(t);
            let mut v = match filter.as_mut() {
                Some(f) => f.process(v),
                None => v,
            };
            if let Some(c) = &s.shaper {
                v = shape(c, v);
            }
            *out += v * env;
        }
    }

    let mut chain = mix;

    if let Some(ph) = &plan.phaser {
        let mut stages: Vec<Biquad> = ph
            .stages
            .iter()
            .map(|&(f, q)| Biquad::new(FilterKind::Allpass, f, q, 0.0, sr))
            .collect();
        for (i, v) in chain.iter_mut().enumerate() {
            if let Some((rate, depth)) = ph.lfo {
                let t = i as f64 / sr;
                let m = depth * (2.0 * std::f64::consts::PI * rate * t).sin();
                for (stage, &(f, q)) in stages.iter_mut().zip(&ph.stages) {
                    stage.set(FilterKind::Allpass, f + m, q, 0.0, sr);
                }
            }
            let mut x = *v;
            for stage in stages.iter_mut() {
                x = stage.process(x);
            }
            *v = x;
        }
    }

    if let Some(c) = &plan.comb {
        let delay_samples = (c.delay.min(MAX_DELAY_S) * sr).max(CYCLE_MIN_DELAY);
        let mut delay = Delay::new((MAX_DELAY_S * sr) as usize + CYCLE_MIN_DELAY as usize + 4);
        for v in chain.iter_mut() {
            let d = delay.read(delay_samples);
            delay.push(*v + c.feedback * d);
            *v += c.mix * d;
        }
    }

    for v in chain.iter_mut() {
        let mut x = shape(&plan.sat.drive1, *v);
        if let Some(f) = &plan.sat.fold {
            x = shape(f, x);
        }
        *v = shape(&plan.sat.drive2, x);
    }

    if let Some(b) = &plan.bass {
        let mut f = Biquad::new(b.kind, b.freq, b.q, b.gain_db, sr);
        for v in chain.iter_mut() {
            *v = f.process(*v);
        }
    }

    if let Some(r) = &plan.reverb {
        let impulse: Vec<f64> = r.impulse.iter().map(|v| *v as f64).collect();
        let wet = convolve(&chain, &impulse);
        for (v, w) in chain.iter_mut().zip(wet) {
            *v += w * r.wet;
        }
    }

    let mut comp = Compressor::new(
        -17.0,
        4.0,
        plan.master.ratio,
        0.0008,
        plan.master.release,
        sr,
    );
    let mut out: Vec<f32> = chain
        .iter()
        .map(|&v| (comp.process(v) * plan.master.out_gain) as f32)
        .collect();

    if let Some((bits, hold)) = plan.crush {
        bitcrush_in_place(&mut out, bits, hold);
    }
    normalize_peak_in_place(&mut out, plan.peak);
    out
}

/// Rend une percussion : la recette `cfg` appliquée au son, pour une graine.
pub fn render_pad(cfg: &Flavor, src: &SourceSample, seed: u32) -> Sample {
    render_pad_with(cfg, src, seed, &Shaping::default())
}

/// Comme `render_pad`, cadré par `shaping`.
pub fn render_pad_with(cfg: &Flavor, src: &SourceSample, seed: u32, shaping: &Shaping) -> Sample {
    let mut rng = Random::from_seed(seed);
    let p = plan_with(cfg, src, &mut rng, shaping);
    Sample {
        data: render(&p, src),
        channels: 1,
        sample_rate: src.sample_rate,
    }
}
