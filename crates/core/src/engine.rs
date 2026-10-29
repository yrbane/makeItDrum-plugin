//! Le moteur : une réserve de voix mixées dans une sortie stéréo. Chaque
//! frappe applique les réglages du pad (gain, transposition, panoramique,
//! chute, sens), choisit un son à tour de rôle parmi ses variantes, et coupe
//! ce qui doit l'être : le pad lui-même (« cut itself »), les pads qui
//! demandent à être coupés par tous (« cut by all »), ceux de son groupe
//! d'étouffement. Le relâchement de la note arrête le son (fondu court) quand
//! la durée de la note est suivie. Mode mélodique : une seule recette jouée
//! sur tout le clavier, transposée autour du do médian.
use std::sync::Arc;

use crate::kit::{Kit, PAD_COUNT};
use crate::voice::{Playback, Voice};

/// Note MIDI jouée telle quelle en mode mélodique (do médian).
pub const MELODIC_ROOT: u8 = 60;

/// Voix simultanées ; au-delà, la plus ancienne est reprise.
pub const MAX_VOICES: usize = 32;

#[derive(Clone, Default)]
struct Slot {
    voice: Voice,
    pad: usize,
    note: u8,
    /// Ordre de départ, pour reprendre la plus ancienne.
    serial: u64,
}

pub struct Engine {
    host_rate: f32,
    kit: Option<Arc<Kit>>,
    slots: Vec<Slot>,
    round_robin: [u8; PAD_COUNT],
    /// `Some(pad)` : toutes les notes jouent ce pad, transposé.
    melodic: Option<usize>,
    /// Le relâchement de la note arrête le son.
    gate: bool,
    serial: u64,
}

impl Engine {
    pub fn new(host_rate: f32) -> Self {
        Self {
            host_rate,
            kit: None,
            slots: vec![Slot::default(); MAX_VOICES],
            round_robin: [0; PAD_COUNT],
            melodic: None,
            gate: true,
            serial: 0,
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

    /// Suivre la durée de la note : le relâchement arrête le son.
    pub fn set_gate(&mut self, gate: bool) {
        self.gate = gate;
    }

    pub fn gate(&self) -> bool {
        self.gate
    }

    pub fn stop_all(&mut self) {
        self.slots.iter_mut().for_each(|s| s.voice.stop());
    }

    /// Note MIDI → pad par le mapping du kit. Vélocité nulle = ignorée.
    pub fn note_on(&mut self, note: u8, velocity: f32) {
        if let Some(pad) = self.melodic {
            let semitones = note as f64 - MELODIC_ROOT as f64;
            self.play(pad, velocity, semitones, note);
            return;
        }
        if let Some(pad) = self.kit.as_ref().and_then(|k| k.pad_for_note(note)) {
            self.play(pad, velocity, 0.0, note);
        }
    }

    /// Relâchement d'une note : ses voix s'éteignent en fondu court, si la
    /// durée de la note est suivie.
    pub fn note_off(&mut self, note: u8) {
        if !self.gate {
            return;
        }
        for s in self.slots.iter_mut() {
            if s.voice.is_active() && s.note == note {
                s.voice.choke();
            }
        }
    }

    /// Déclenche un pad par son index (clic dans l'interface) ; sans
    /// relâchement, il joue jusqu'au bout.
    pub fn trigger_pad(&mut self, pad: usize, velocity: f32) {
        let note = self
            .kit
            .as_ref()
            .and_then(|k| k.pads.get(pad))
            .map(|p| p.note)
            .unwrap_or(0);
        self.play(pad, velocity, 0.0, note);
    }

    fn play(&mut self, pad: usize, velocity: f32, semitones: f64, note: u8) {
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

        // ce que cette frappe coupe
        let mut free: Option<usize> = None;
        for (i, slot) in self.slots.iter_mut().enumerate() {
            if !slot.voice.is_active() {
                free.get_or_insert(i);
                continue;
            }
            if slot.pad == pad {
                if s.cut_self {
                    // redéclenchement dur, comme une voix unique par pad
                    slot.voice.stop();
                    free.get_or_insert(i);
                }
                continue;
            }
            let other = kit.pads[slot.pad].settings;
            if other.cut_by_all || (s.choke != 0 && other.choke == s.choke) {
                slot.voice.choke();
            }
        }
        // une voix libre, sinon la plus ancienne
        let i = free.unwrap_or_else(|| {
            self.slots
                .iter()
                .enumerate()
                .min_by_key(|(_, s)| s.serial)
                .map(|(i, _)| i)
                .unwrap_or(0)
        });
        self.serial = self.serial.wrapping_add(1);
        let slot = &mut self.slots[i];
        slot.pad = pad;
        slot.note = note;
        slot.serial = self.serial;
        slot.voice.start(playback, frames);
    }

    /// Ajoute les voix actives à la sortie, pad par pad. Sans allocation.
    pub fn render(&mut self, out_l: &mut [f32], out_r: &mut [f32]) {
        let Some(kit) = self.kit.as_ref() else { return };
        for (pad_index, pad) in kit.pads.iter().enumerate().take(PAD_COUNT) {
            for slot in self.slots.iter_mut() {
                if slot.pad == pad_index && slot.voice.is_active() {
                    slot.voice
                        .render(pad.sample_at(slot.voice.variant()), out_l, out_r);
                }
            }
        }
    }

    /// Bit `i` levé si le pad `i` est en cours de lecture.
    pub fn playing_mask(&self) -> u32 {
        self.slots.iter().fold(0, |m, s| {
            if s.voice.is_active() && s.pad < PAD_COUNT {
                m | (1 << s.pad)
            } else {
                m
            }
        })
    }

    /// Nombre de voix en cours.
    pub fn active_voices(&self) -> usize {
        self.slots.iter().filter(|s| s.voice.is_active()).count()
    }
}
