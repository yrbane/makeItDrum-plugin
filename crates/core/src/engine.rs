//! Le moteur : 20 voix, une par pad, mixées dans une sortie stéréo. Chaque
//! frappe applique les réglages du pad (gain, transposition, panoramique,
//! chute, sens), choisit un son à tour de rôle parmi ses variantes, et coupe
//! les pads de son groupe d'étouffement. Mode mélodique : une seule recette
//! jouée sur tout le clavier, transposée autour du do médian.
use std::sync::Arc;

use crate::kit::{Kit, PAD_COUNT};
use crate::voice::{Playback, Voice};

/// Note MIDI jouée telle quelle en mode mélodique (do médian).
pub const MELODIC_ROOT: u8 = 60;

pub struct Engine {
    host_rate: f32,
    kit: Option<Arc<Kit>>,
    voices: [Voice; PAD_COUNT],
    round_robin: [u8; PAD_COUNT],
    /// `Some(pad)` : toutes les notes jouent ce pad, transposé.
    melodic: Option<usize>,
}

impl Engine {
    pub fn new(host_rate: f32) -> Self {
        Self {
            host_rate,
            kit: None,
            voices: std::array::from_fn(|_| Voice::new()),
            round_robin: [0; PAD_COUNT],
            melodic: None,
        }
    }

    pub fn set_sample_rate(&mut self, host_rate: f32) {
        self.host_rate = host_rate;
    }

    /// Remplace le kit ; les voix en cours sont coupées.
    pub fn set_kit(&mut self, kit: Option<Arc<Kit>>) {
        self.stop_all();
        self.kit = kit;
        self.round_robin = [0; PAD_COUNT];
    }

    pub fn kit(&self) -> Option<&Arc<Kit>> {
        self.kit.as_ref()
    }

    /// Mode mélodique : `Some(pad)` joue ce pad sur tout le clavier.
    pub fn set_melodic(&mut self, pad: Option<usize>) {
        self.melodic = pad.filter(|p| *p < PAD_COUNT);
    }

    pub fn melodic(&self) -> Option<usize> {
        self.melodic
    }

    pub fn stop_all(&mut self) {
        self.voices.iter_mut().for_each(Voice::stop);
    }

    /// Note MIDI → pad par le mapping du kit. Vélocité nulle = ignorée.
    pub fn note_on(&mut self, note: u8, velocity: f32) {
        if let Some(pad) = self.melodic {
            let semitones = note as f64 - MELODIC_ROOT as f64;
            self.play(pad, velocity, semitones);
            return;
        }
        if let Some(pad) = self.kit.as_ref().and_then(|k| k.pad_for_note(note)) {
            self.trigger_pad(pad, velocity);
        }
    }

    /// Déclenche un pad par son index (clic dans l'interface).
    pub fn trigger_pad(&mut self, pad: usize, velocity: f32) {
        self.play(pad, velocity, 0.0);
    }

    fn play(&mut self, pad: usize, velocity: f32, semitones: f64) {
        if velocity <= 0.0 || pad >= PAD_COUNT {
            return;
        }
        let Some(kit) = self.kit.as_ref() else { return };
        let Some(p) = kit.pads.get(pad).filter(|p| !p.is_empty()) else {
            return;
        };
        let s = p.settings;
        let variant = (self.round_robin[pad] as usize) % p.variant_count();
        self.round_robin[pad] = self.round_robin[pad].wrapping_add(1);
        let sample = p.sample_at(variant);
        let ratio = sample.sample_rate as f64 / self.host_rate as f64;
        let gain = velocity * s.gain();
        let (pan_l, pan_r) = s.pan_gains();
        let playback = Playback {
            step: ratio * s.rate() * 2f64.powf(semitones / 12.0),
            gain_l: gain * pan_l,
            gain_r: gain * pan_r,
            reverse: s.reverse,
            decay: s.decay,
            variant,
        };
        let frames = sample.frames();
        if s.choke != 0 {
            for (i, other) in kit.pads.iter().enumerate().take(PAD_COUNT) {
                if i != pad && other.settings.choke == s.choke {
                    self.voices[i].choke();
                }
            }
        }
        self.voices[pad].start(playback, frames);
    }

    /// Ajoute les voix actives à la sortie. Sans allocation.
    pub fn render(&mut self, out_l: &mut [f32], out_r: &mut [f32]) {
        let Some(kit) = self.kit.as_ref() else { return };
        for (voice, pad) in self.voices.iter_mut().zip(&kit.pads) {
            if voice.is_active() {
                voice.render(pad.sample_at(voice.variant()), out_l, out_r);
            }
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
