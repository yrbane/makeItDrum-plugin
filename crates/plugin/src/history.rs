//! Historique des kits pour « Annuler » : les derniers kits publiés, avec
//! l'état persistant qui va avec, du plus ancien au plus récent.
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use make_it_drum_core::kit::Kit;

use crate::loader::KitState;

/// Nombre de kits précédents gardés.
pub const DEPTH: usize = 8;

#[derive(Default)]
pub struct History {
    past: Mutex<VecDeque<(Arc<Kit>, KitState)>>,
}

impl History {
    pub fn new() -> Self {
        Self::default()
    }

    /// Mémorise le kit courant avant qu'il soit remplacé.
    pub fn push(&self, kit: Arc<Kit>, state: KitState) {
        if let Ok(mut p) = self.past.lock() {
            p.push_back((kit, state));
            while p.len() > DEPTH {
                p.pop_front();
            }
        }
    }

    /// Reprend le kit précédent, s'il y en a un.
    pub fn pop(&self) -> Option<(Arc<Kit>, KitState)> {
        self.past.lock().ok().and_then(|mut p| p.pop_back())
    }

    pub fn len(&self) -> usize {
        self.past.lock().map(|p| p.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn clear(&self) {
        if let Ok(mut p) = self.past.lock() {
            p.clear();
        }
    }
}
