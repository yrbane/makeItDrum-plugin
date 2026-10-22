//! Une voix one-shot : lit un échantillon du début à la fin, à un ratio de
//! vitesse donné, par interpolation linéaire, et s'ajoute à la sortie.
use crate::wav::Sample;

#[derive(Debug, Clone, Default)]
pub struct Voice {
    position: f64,
    step: f64,
    gain: f32,
    active: bool,
}

impl Voice {
    pub fn new() -> Self {
        Self::default()
    }

    /// (Re)démarre la lecture au début. `ratio` = fréquence du sample / fréquence de l'hôte.
    pub fn trigger(&mut self, velocity: f32, ratio: f64) {
        self.position = 0.0;
        self.step = ratio;
        self.gain = velocity;
        self.active = true;
    }

    pub fn stop(&mut self) {
        self.active = false;
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Ajoute la lecture à `out_l`/`out_r` (même longueur). Sans allocation.
    pub fn render(&mut self, sample: &Sample, out_l: &mut [f32], out_r: &mut [f32]) {
        if !self.active {
            return;
        }
        let frames = sample.frames();
        let last = frames.saturating_sub(1);
        for (l, r) in out_l.iter_mut().zip(out_r.iter_mut()) {
            let i = self.position as usize;
            if i >= frames {
                self.active = false;
                return;
            }
            let frac = (self.position - i as f64) as f32;
            let (a_l, a_r) = sample.frame(i);
            let (b_l, b_r) = sample.frame((i + 1).min(last));
            *l += (a_l + (b_l - a_l) * frac) * self.gain;
            *r += (a_r + (b_r - a_r) * frac) * self.gain;
            self.position += self.step;
        }
        if self.position >= frames as f64 {
            self.active = false;
        }
    }
}
