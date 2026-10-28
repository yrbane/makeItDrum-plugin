//! Les séries de variations d'un kit, pour y revenir : chaque « Nouvelles
//! variations » ajoute la liste des graines des 20 pads ; ◀ et ▶ se déplacent
//! dans cette liste et le kit est rejoué avec les graines retrouvées.
use std::sync::Mutex;

/// Nombre de séries gardées ; au-delà, la plus ancienne part.
pub const DEPTH: usize = 64;

/// Les graines d'un kit, pad par pad (`None` : pad vide ou son déposé).
pub type Seeds = Vec<Option<u32>>;

#[derive(Default)]
struct Inner {
    sets: Vec<Seeds>,
    cursor: usize,
}

#[derive(Default)]
pub struct Variations {
    inner: Mutex<Inner>,
}

impl Variations {
    pub fn new() -> Self {
        Self::default()
    }

    /// Repart d'une seule série (nouveau son, kit chargé).
    pub fn reset(&self, seeds: Seeds) {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        g.sets = vec![seeds];
        g.cursor = 0;
    }

    /// Ajoute une série après la série courante ; ce qui suivait est oublié.
    /// Une série identique à la courante n'est pas ajoutée.
    pub fn record(&self, seeds: Seeds) {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if g.sets.get(g.cursor) == Some(&seeds) {
            return;
        }
        let keep = (g.cursor + 1).min(g.sets.len());
        g.sets.truncate(keep);
        g.sets.push(seeds);
        if g.sets.len() > DEPTH {
            g.sets.remove(0);
        }
        g.cursor = g.sets.len() - 1;
    }

    /// Recule d'une série et la rend, s'il y en a une.
    pub fn back(&self) -> Option<Seeds> {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if g.cursor == 0 || g.sets.is_empty() {
            return None;
        }
        g.cursor -= 1;
        g.sets.get(g.cursor).cloned()
    }

    /// Avance d'une série et la rend, s'il y en a une.
    pub fn forward(&self) -> Option<Seeds> {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if g.cursor + 1 >= g.sets.len() {
            return None;
        }
        g.cursor += 1;
        g.sets.get(g.cursor).cloned()
    }

    /// (série courante à partir de 1, nombre de séries) ; (0, 0) sans série.
    pub fn position(&self) -> (usize, usize) {
        let g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if g.sets.is_empty() {
            (0, 0)
        } else {
            (g.cursor + 1, g.sets.len())
        }
    }

    pub fn can_back(&self) -> bool {
        self.position().0 > 1
    }

    pub fn can_forward(&self) -> bool {
        let (c, n) = self.position();
        c < n
    }
}
