//! Capture du son depuis l'entrée audio du plugin, pour fabriquer un kit
//! sans passer par un fichier. Le thread audio écrit dans un tampon alloué
//! d'avance, sans jamais bloquer : s'il ne peut pas prendre le verrou, il
//! saute le bloc. Le thread de fond récupère l'enregistrement une fois fini.
use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};
use std::sync::Mutex;

use make_it_drum_core::wav::Sample;

/// Durée maximale d'une capture, en secondes.
pub const MAX_SECONDS: f64 = 10.0;
/// Fréquence maximale prise en compte pour dimensionner le tampon.
const MAX_RATE: f64 = 192_000.0;

const IDLE: u8 = 0;
const RECORDING: u8 = 1;
const DONE: u8 = 2;

pub struct Recorder {
    buffer: Mutex<Vec<f32>>,
    state: AtomicU8,
    wanted: AtomicUsize,
    written: AtomicUsize,
    sample_rate: AtomicUsize,
}

impl Default for Recorder {
    fn default() -> Self {
        Self::new()
    }
}

impl Recorder {
    pub fn new() -> Self {
        Self {
            buffer: Mutex::new(Vec::with_capacity((MAX_SECONDS * MAX_RATE) as usize)),
            state: AtomicU8::new(IDLE),
            wanted: AtomicUsize::new(0),
            written: AtomicUsize::new(0),
            sample_rate: AtomicUsize::new(44_100),
        }
    }

    /// Lance une capture de `seconds` secondes (interface). Ignoré si une
    /// capture est en cours ou attend d'être lue.
    pub fn arm(&self, seconds: f64, sample_rate: f32) {
        if self.state.load(Ordering::Acquire) != IDLE {
            return;
        }
        let seconds = seconds.clamp(0.001, MAX_SECONDS);
        let frames = (seconds * sample_rate as f64) as usize;
        if let Ok(mut b) = self.buffer.lock() {
            b.clear();
        }
        self.sample_rate
            .store(sample_rate as usize, Ordering::Release);
        self.wanted.store(frames, Ordering::Release);
        self.written.store(0, Ordering::Release);
        self.state.store(RECORDING, Ordering::Release);
    }

    pub fn is_recording(&self) -> bool {
        self.state.load(Ordering::Acquire) == RECORDING
    }

    pub fn is_done(&self) -> bool {
        self.state.load(Ordering::Acquire) == DONE
    }

    /// Avancement de la capture, 0 à 1.
    pub fn progress(&self) -> f32 {
        let wanted = self.wanted.load(Ordering::Acquire).max(1);
        (self.written.load(Ordering::Acquire) as f32 / wanted as f32).min(1.0)
    }

    /// Ajoute un bloc (thread audio) : moyenne des canaux, sans allocation,
    /// sans blocage. Rien n'est écrit si le verrou n'est pas libre.
    pub fn push<T: AsRef<[f32]>>(&self, channels: &[T]) {
        if self.state.load(Ordering::Acquire) != RECORDING || channels.is_empty() {
            return;
        }
        let wanted = self.wanted.load(Ordering::Acquire);
        let mut written = self.written.load(Ordering::Acquire);
        if let Ok(mut b) = self.buffer.try_lock() {
            let n = channels[0].as_ref().len();
            let inv = 1.0 / channels.len() as f32;
            for i in 0..n {
                if written >= wanted || b.len() >= b.capacity() {
                    break;
                }
                let mut v = 0.0;
                for ch in channels {
                    v += ch.as_ref().get(i).copied().unwrap_or(0.0);
                }
                b.push(v * inv);
                written += 1;
            }
            self.written.store(written, Ordering::Release);
        }
        if written >= wanted {
            self.state.store(DONE, Ordering::Release);
        }
    }

    /// Récupère l'enregistrement terminé (thread de fond) et libère le tampon.
    pub fn take(&self) -> Option<Sample> {
        if self.state.load(Ordering::Acquire) != DONE {
            return None;
        }
        let data = self
            .buffer
            .lock()
            .ok()
            .map(|mut b| std::mem::take(&mut *b))?;
        self.state.store(IDLE, Ordering::Release);
        if let Ok(mut b) = self.buffer.lock() {
            b.reserve_exact((MAX_SECONDS * MAX_RATE) as usize);
        }
        Some(Sample {
            data,
            channels: 1,
            sample_rate: self.sample_rate.load(Ordering::Acquire) as u32,
        })
    }

    /// Abandonne une capture en cours ou en attente.
    pub fn cancel(&self) {
        self.state.store(IDLE, Ordering::Release);
        if let Ok(mut b) = self.buffer.lock() {
            b.clear();
        }
    }
}
