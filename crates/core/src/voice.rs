//! Une voix one-shot : lit un échantillon du début à la fin (ou à l'envers),
//! à un ratio de vitesse donné, par interpolation linéaire, et s'ajoute à la
//! sortie avec un gain par canal, un fondu de fin optionnel et un étouffement
//! rapide à la demande. Sans allocation.
use crate::wav::Sample;

/// Durée de l'étouffement (choke), en trames de l'hôte.
pub const CHOKE_FADE_FRAMES: u32 = 256;

#[derive(Debug, Clone, Default)]
pub struct Voice {
    position: f64,
    step: f64,
    gain_l: f32,
    gain_r: f32,
    active: bool,
    reverse: bool,
    /// Trame (dans le son) à partir de laquelle le fondu de fin commence, et sa longueur.
    fade_start: f64,
    fade_len: f64,
    /// Son joué : 0 = principal, puis variantes.
    variant: usize,
    /// Étouffement en cours : gain restant, de 1 vers 0.
    choke: Option<f32>,
}

/// Comment jouer : vitesse, gains, sens, fondu, variante.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Playback {
    /// Pas de lecture : fréquence du sample / fréquence de l'hôte × transposition.
    pub step: f64,
    pub gain_l: f32,
    pub gain_r: f32,
    pub reverse: bool,
    /// Part du son jouée (0,05 à 1) ; en dessous de 1, fondu sur le dernier cinquième.
    pub decay: f32,
    pub variant: usize,
}

impl Playback {
    /// Lecture simple : vitesse `ratio`, gain `velocity` des deux côtés.
    pub fn plain(velocity: f32, ratio: f64) -> Self {
        Self {
            step: ratio,
            gain_l: velocity,
            gain_r: velocity,
            reverse: false,
            decay: 1.0,
            variant: 0,
        }
    }
}

impl Voice {
    pub fn new() -> Self {
        Self::default()
    }

    /// (Re)démarre la lecture au début. `ratio` = fréquence du sample / fréquence de l'hôte.
    pub fn trigger(&mut self, velocity: f32, ratio: f64) {
        self.start(Playback::plain(velocity, ratio), 0);
    }

    /// (Re)démarre la lecture selon `p`, pour un son de `frames` trames.
    pub fn start(&mut self, p: Playback, frames: usize) {
        self.step = p.step.abs();
        self.gain_l = p.gain_l;
        self.gain_r = p.gain_r;
        self.reverse = p.reverse;
        self.variant = p.variant;
        self.choke = None;
        self.active = true;
        let decay = if p.decay.is_finite() {
            p.decay.clamp(0.05, 1.0) as f64
        } else {
            1.0
        };
        if decay < 1.0 {
            let played = frames as f64 * decay;
            self.fade_len = played * 0.2;
            self.fade_start = played - self.fade_len;
        } else {
            self.fade_len = 0.0;
            self.fade_start = f64::INFINITY;
        }
        self.position = if p.reverse {
            frames.saturating_sub(1) as f64
        } else {
            0.0
        };
    }

    pub fn stop(&mut self) {
        self.active = false;
    }

    /// Étouffe la voix : fondu de `CHOKE_FADE_FRAMES` trames puis arrêt.
    pub fn choke(&mut self) {
        if self.active && self.choke.is_none() {
            self.choke = Some(1.0);
        }
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Le son joué (0 = principal, puis variantes).
    pub fn variant(&self) -> usize {
        self.variant
    }

    /// Ajoute la lecture à `out_l`/`out_r` (même longueur). Sans allocation.
    pub fn render(&mut self, sample: &Sample, out_l: &mut [f32], out_r: &mut [f32]) {
        if !self.active {
            return;
        }
        let frames = sample.frames();
        if frames == 0 {
            self.active = false;
            return;
        }
        let last = frames - 1;
        let choke_step = 1.0 / CHOKE_FADE_FRAMES as f32;
        for (l, r) in out_l.iter_mut().zip(out_r.iter_mut()) {
            if self.position < 0.0 || self.position >= frames as f64 {
                self.active = false;
                return;
            }
            let i = self.position as usize;
            let frac = (self.position - i as f64) as f32;
            let (a_l, a_r) = sample.frame(i);
            let (b_l, b_r) = sample.frame((i + 1).min(last));
            // distance parcourue depuis le départ, pour le fondu de fin
            let travelled = if self.reverse {
                last as f64 - self.position
            } else {
                self.position
            };
            let mut env = 1.0f32;
            if travelled >= self.fade_start {
                let t = ((travelled - self.fade_start) / self.fade_len.max(1.0)) as f32;
                if t >= 1.0 {
                    self.active = false;
                    return;
                }
                env = 1.0 - t;
            }
            if let Some(c) = self.choke.as_mut() {
                *c -= choke_step;
                if *c <= 0.0 {
                    self.active = false;
                    return;
                }
                env *= *c;
            }
            *l += (a_l + (b_l - a_l) * frac) * self.gain_l * env;
            *r += (a_r + (b_r - a_r) * frac) * self.gain_r * env;
            self.position += if self.reverse { -self.step } else { self.step };
        }
        if self.position < 0.0 || self.position >= frames as f64 {
            self.active = false;
        }
    }
}
