//! Le moteur : 20 voix, une par pad, mixées dans une sortie stéréo.
use std::sync::Arc;

use crate::kit::{Kit, PAD_COUNT};
use crate::voice::Voice;

pub struct Engine {
    host_rate: f32,
    kit: Option<Arc<Kit>>,
    voices: [Voice; PAD_COUNT],
}

impl Engine {
    pub fn new(host_rate: f32) -> Self {
        Self {
            host_rate,
            kit: None,
            voices: std::array::from_fn(|_| Voice::new()),
        }
    }

    pub fn set_sample_rate(&mut self, host_rate: f32) {
        self.host_rate = host_rate;
    }

    /// Remplace le kit ; les voix en cours sont coupées.
    pub fn set_kit(&mut self, kit: Option<Arc<Kit>>) {
        self.stop_all();
        self.kit = kit;
    }

    pub fn kit(&self) -> Option<&Arc<Kit>> {
        self.kit.as_ref()
    }

    pub fn stop_all(&mut self) {
        self.voices.iter_mut().for_each(Voice::stop);
    }

    /// Note MIDI → pad par le mapping du kit. Vélocité nulle = ignorée.
    pub fn note_on(&mut self, note: u8, velocity: f32) {
        if let Some(pad) = self.kit.as_ref().and_then(|k| k.pad_for_note(note)) {
            self.trigger_pad(pad, velocity);
        }
    }

    /// Déclenche un pad par son index (clic dans l'interface).
    pub fn trigger_pad(&mut self, pad: usize, velocity: f32) {
        if velocity <= 0.0 || pad >= PAD_COUNT {
            return;
        }
        let Some(kit) = self.kit.as_ref() else { return };
        let Some(p) = kit.pads.get(pad).filter(|p| !p.is_empty()) else {
            return;
        };
        let ratio = p.sample.sample_rate as f64 / self.host_rate as f64;
        self.voices[pad].trigger(velocity, ratio);
    }

    /// Ajoute les voix actives à la sortie. Sans allocation.
    pub fn render(&mut self, out_l: &mut [f32], out_r: &mut [f32]) {
        let Some(kit) = self.kit.as_ref() else { return };
        for (voice, pad) in self.voices.iter_mut().zip(&kit.pads) {
            voice.render(&pad.sample, out_l, out_r);
        }
    }

    /// Bit `i` levé si le pad `i` est en cours de lecture.
    pub fn playing_mask(&self) -> u32 {
        self.voices
            .iter()
            .enumerate()
            .fold(0, |m, (i, v)| if v.is_active() { m | (1 << i) } else { m })
    }
}
