//! Point de partage entre l'interface, le thread de fond et le thread audio.
//!
//! Le thread audio ne doit ni allouer ni bloquer : il lit `generation()` à
//! chaque bloc et, si elle a changé, tente `try_current()` pour cloner l'Arc
//! du kit. Les derniers kits publiés sont gardés vivants ici, de sorte que
//! la libération d'un kit n'arrive jamais sur le thread audio.
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use make_it_drum_core::kit::{Kit, PAD_COUNT};

/// Nombre de kits précédents gardés vivants côté fond.
const KEEPALIVE: usize = 4;

#[derive(Default)]
pub struct KitSlot {
    current: Mutex<Option<Arc<Kit>>>,
    generation: AtomicU64,
    keepalive: Mutex<VecDeque<Arc<Kit>>>,
    status: Mutex<String>,
    pending_triggers: AtomicU32,
    playing: AtomicU32,
}

impl KitSlot {
    pub fn new() -> Self {
        Self::default()
    }

    /// Publie un nouveau kit (thread de fond ou GUI).
    pub fn publish(&self, kit: Arc<Kit>) {
        let previous = self
            .current
            .lock()
            .map(|mut c| c.replace(kit))
            .unwrap_or(None);
        if let Some(prev) = previous {
            if let Ok(mut keep) = self.keepalive.lock() {
                keep.push_back(prev);
                while keep.len() > KEEPALIVE {
                    keep.pop_front();
                }
            }
        }
        self.generation.fetch_add(1, Ordering::AcqRel);
    }

    pub fn clear(&self) {
        let previous = self.current.lock().map(|mut c| c.take()).unwrap_or(None);
        if let Some(prev) = previous {
            if let Ok(mut keep) = self.keepalive.lock() {
                keep.push_back(prev);
            }
        }
        self.generation.fetch_add(1, Ordering::AcqRel);
    }

    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Le kit courant, en attendant le verrou (thread de fond ou GUI, jamais audio).
    pub fn current(&self) -> Option<Arc<Kit>> {
        self.current.lock().ok().and_then(|c| c.clone())
    }

    /// Lecture non bloquante du kit courant (thread audio). `None` si le verrou est pris.
    pub fn try_current(&self) -> Option<Option<Arc<Kit>>> {
        self.current.try_lock().ok().map(|c| c.clone())
    }

    pub fn set_status(&self, status: impl Into<String>) {
        if let Ok(mut s) = self.status.lock() {
            *s = status.into();
        }
    }

    pub fn status(&self) -> String {
        self.status.lock().map(|s| s.clone()).unwrap_or_default()
    }

    /// Demande (GUI) le déclenchement d'un pad ; ignoré hors des 20 pads.
    pub fn request_trigger(&self, pad: usize) {
        if pad < PAD_COUNT {
            self.pending_triggers.fetch_or(1 << pad, Ordering::AcqRel);
        }
    }

    /// Récupère et vide le masque des pads à déclencher (thread audio).
    pub fn take_triggers(&self) -> u32 {
        self.pending_triggers.swap(0, Ordering::AcqRel)
    }

    pub fn set_playing(&self, mask: u32) {
        self.playing.store(mask, Ordering::Release);
    }

    pub fn playing(&self) -> u32 {
        self.playing.load(Ordering::Acquire)
    }
}
