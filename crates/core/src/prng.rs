//! Aléatoire déterministe du moteur, port exact de la page : mulberry32 sur
//! une graine 32 bits, et les helpers `rnd` / `jitter` / `chance` /
//! `random_offset`. Même graine, même suite de tirages que le JS.

/// Générateur mulberry32 : une graine 32 bits reproduit exactement la même suite.
#[derive(Debug, Clone)]
pub struct Mulberry32 {
    a: u32,
}

impl Mulberry32 {
    pub fn new(seed: u32) -> Self {
        Self { a: seed }
    }

    /// Prochain tirage dans [0, 1[, comme `Math.random`.
    pub fn next_f64(&mut self) -> f64 {
        self.a = self.a.wrapping_add(0x6d2b_79f5);
        let mut t = self.a;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        f64::from(t ^ (t >> 14)) / 4_294_967_296.0
    }
}

/// Les helpers du moteur autour d'un générateur.
#[derive(Debug, Clone)]
pub struct Random {
    gen: Mulberry32,
}

impl Random {
    pub fn from_seed(seed: u32) -> Self {
        Self {
            gen: Mulberry32::new(seed),
        }
    }

    pub fn random(&mut self) -> f64 {
        self.gen.next_f64()
    }

    pub fn rnd(&mut self, a: f64, b: f64) -> f64 {
        a + self.random() * (b - a)
    }

    /// Écart relatif centré : base × (1 ± pct).
    pub fn jitter(&mut self, base: f64, pct: f64) -> f64 {
        base * (1.0 + self.rnd(-pct, pct))
    }

    pub fn chance(&mut self, p: f64) -> bool {
        self.random() < p
    }

    /// Point de départ aléatoire dans le son importé (secondes). Aucun tirage
    /// si la fenêtre est nulle, comme sur la page.
    pub fn random_offset(&mut self, duration: f64, max_frac: f64) -> f64 {
        if duration == 0.0 {
            return 0.0;
        }
        let span = (duration * max_frac - 0.01).max(0.0);
        if span > 0.0 {
            self.random() * span
        } else {
            0.0
        }
    }

    /// Graine 32 bits tirée de ce générateur (pour en dériver d'autres).
    pub fn random_seed(&mut self) -> u32 {
        (self.random() * 4_294_967_296.0).floor() as u32
    }
}

/// Graine 32 bits « fraîche » : horloge et compteur, pour un nouveau rendu.
pub fn fresh_seed() -> u32 {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut r =
        Mulberry32::new((nanos as u32) ^ ((nanos >> 32) as u32) ^ n.wrapping_mul(0x9e37_79b9));
    (r.next_f64() * 4_294_967_296.0).floor() as u32
}
