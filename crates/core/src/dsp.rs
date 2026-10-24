//! Les nœuds audio du moteur, équivalents hors ligne des nœuds Web Audio
//! que la page utilise : biquad (formules de la spécification Web Audio),
//! ligne à retard, compresseur, convolution par FFT.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterKind {
    Lowpass,
    Highpass,
    Bandpass,
    Allpass,
    Peaking,
}

/// Filtre biquad aux coefficients de la spécification Web Audio : pour les
/// passe-bas et passe-haut, `q` est en dB ; pour les autres, un Q classique.
#[derive(Debug, Clone, Default)]
pub struct Biquad {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    x1: f64,
    x2: f64,
    y1: f64,
    y2: f64,
}

impl Biquad {
    pub fn new(kind: FilterKind, freq: f64, q: f64, gain_db: f64, sr: f64) -> Self {
        let mut f = Self::default();
        f.set(kind, freq, q, gain_db, sr);
        f
    }

    /// Recalcule les coefficients ; l'état est conservé (modulation continue).
    pub fn set(&mut self, kind: FilterKind, freq: f64, q: f64, gain_db: f64, sr: f64) {
        let nyquist = sr * 0.5;
        let freq = freq.max(1.0).min(nyquist * 0.999);
        let w0 = 2.0 * std::f64::consts::PI * freq / sr;
        let (sin, cos) = w0.sin_cos();
        let (b0, b1, b2, a0, a1, a2) = match kind {
            FilterKind::Lowpass => {
                let alpha = sin / (2.0 * 10f64.powf(q / 20.0));
                (
                    (1.0 - cos) / 2.0,
                    1.0 - cos,
                    (1.0 - cos) / 2.0,
                    1.0 + alpha,
                    -2.0 * cos,
                    1.0 - alpha,
                )
            }
            FilterKind::Highpass => {
                let alpha = sin / (2.0 * 10f64.powf(q / 20.0));
                (
                    (1.0 + cos) / 2.0,
                    -(1.0 + cos),
                    (1.0 + cos) / 2.0,
                    1.0 + alpha,
                    -2.0 * cos,
                    1.0 - alpha,
                )
            }
            FilterKind::Bandpass => {
                let alpha = sin / (2.0 * q.max(1e-4));
                (alpha, 0.0, -alpha, 1.0 + alpha, -2.0 * cos, 1.0 - alpha)
            }
            FilterKind::Allpass => {
                let alpha = sin / (2.0 * q.max(1e-4));
                (
                    1.0 - alpha,
                    -2.0 * cos,
                    1.0 + alpha,
                    1.0 + alpha,
                    -2.0 * cos,
                    1.0 - alpha,
                )
            }
            FilterKind::Peaking => {
                let a = 10f64.powf(gain_db / 40.0);
                let alpha = sin / (2.0 * q.max(1e-4));
                (
                    1.0 + alpha * a,
                    -2.0 * cos,
                    1.0 - alpha * a,
                    1.0 + alpha / a,
                    -2.0 * cos,
                    1.0 - alpha / a,
                )
            }
        };
        self.b0 = b0 / a0;
        self.b1 = b1 / a0;
        self.b2 = b2 / a0;
        self.a1 = a1 / a0;
        self.a2 = a2 / a0;
    }

    #[inline]
    pub fn process(&mut self, x: f64) -> f64 {
        let y = self.b0 * x + self.b1 * self.x1 + self.b2 * self.x2
            - self.a1 * self.y1
            - self.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }
}

/// Ligne à retard à lecture fractionnaire (interpolation linéaire).
#[derive(Debug, Clone)]
pub struct Delay {
    buf: Vec<f64>,
    write: usize,
}

impl Delay {
    pub fn new(max_samples: usize) -> Self {
        Self {
            buf: vec![0.0; max_samples.max(2)],
            write: 0,
        }
    }

    /// Valeur retardée de `delay` échantillons (borné à la taille du tampon).
    #[inline]
    pub fn read(&self, delay: f64) -> f64 {
        let n = self.buf.len();
        let d = delay.max(0.0).min((n - 1) as f64);
        let i = d.floor();
        let frac = d - i;
        let idx = (self.write + n - i as usize) % n;
        let a = self.buf[(idx + n - 1) % n];
        let b = self.buf[(idx + n - 2) % n];
        a + (b - a) * frac
    }

    #[inline]
    pub fn push(&mut self, x: f64) {
        self.buf[self.write] = x;
        self.write = (self.write + 1) % self.buf.len();
    }
}

/// Compresseur à genou doux, réaction en dB lissée par attaque et relâchement.
#[derive(Debug, Clone)]
pub struct Compressor {
    threshold: f64,
    knee: f64,
    ratio: f64,
    attack: f64,
    release: f64,
    reduction_db: f64,
}

impl Compressor {
    pub fn new(
        threshold_db: f64,
        knee_db: f64,
        ratio: f64,
        attack_s: f64,
        release_s: f64,
        sr: f64,
    ) -> Self {
        Self {
            threshold: threshold_db,
            knee: knee_db,
            ratio: ratio.max(1.0),
            attack: 1.0 - (-1.0 / (attack_s.max(1e-5) * sr)).exp(),
            release: 1.0 - (-1.0 / (release_s.max(1e-5) * sr)).exp(),
            reduction_db: 0.0,
        }
    }

    fn gain_for(&self, level_db: f64) -> f64 {
        let (t, w, r) = (self.threshold, self.knee, self.ratio);
        if level_db < t - w / 2.0 {
            0.0
        } else if level_db <= t + w / 2.0 {
            let d = level_db - t + w / 2.0;
            (1.0 / r - 1.0) * d * d / (2.0 * w)
        } else {
            (level_db - t) * (1.0 / r - 1.0)
        }
    }

    #[inline]
    pub fn process(&mut self, x: f64) -> f64 {
        let level_db = 20.0 * (x.abs() + 1e-9).log10();
        let wanted = self.gain_for(level_db);
        let coef = if wanted < self.reduction_db {
            self.attack
        } else {
            self.release
        };
        self.reduction_db += (wanted - self.reduction_db) * coef;
        x * 10f64.powf(self.reduction_db / 20.0)
    }
}

fn fft_complex(re: &mut [f64], im: &mut [f64], inverse: bool) {
    let n = re.len();
    let mut j = 0usize;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j ^= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let sign = if inverse { 2.0 } else { -2.0 };
    let mut len = 2;
    while len <= n {
        let ang = sign * std::f64::consts::PI / len as f64;
        let (wi0, wr0) = ang.sin_cos();
        let half = len / 2;
        let mut i = 0;
        while i < n {
            let (mut wr, mut wi) = (1.0, 0.0);
            for k in 0..half {
                let (ur, ui) = (re[i + k], im[i + k]);
                let (xr, xi) = (re[i + k + half], im[i + k + half]);
                let vr = xr * wr - xi * wi;
                let vi = xr * wi + xi * wr;
                re[i + k] = ur + vr;
                im[i + k] = ui + vi;
                re[i + k + half] = ur - vr;
                im[i + k + half] = ui - vi;
                let nwr = wr * wr0 - wi * wi0;
                wi = wr * wi0 + wi * wr0;
                wr = nwr;
            }
            i += len;
        }
        len <<= 1;
    }
    if inverse {
        let inv = 1.0 / n as f64;
        for v in re.iter_mut() {
            *v *= inv;
        }
        for v in im.iter_mut() {
            *v *= inv;
        }
    }
}

/// Convolution linéaire par FFT, tronquée à la longueur de `x` (le contexte
/// hors ligne s'arrête là, comme sur la page).
pub fn convolve(x: &[f64], h: &[f64]) -> Vec<f64> {
    if x.is_empty() || h.is_empty() {
        return vec![0.0; x.len()];
    }
    let n = (x.len() + h.len() - 1).next_power_of_two();
    let mut xr = vec![0.0; n];
    let mut xi = vec![0.0; n];
    let mut hr = vec![0.0; n];
    let mut hi = vec![0.0; n];
    xr[..x.len()].copy_from_slice(x);
    hr[..h.len()].copy_from_slice(h);
    fft_complex(&mut xr, &mut xi, false);
    fft_complex(&mut hr, &mut hi, false);
    for i in 0..n {
        let (a, b) = (xr[i], xi[i]);
        xr[i] = a * hr[i] - b * hi[i];
        xi[i] = a * hi[i] + b * hr[i];
    }
    fft_complex(&mut xr, &mut xi, true);
    xr.truncate(x.len());
    xr
}

/// Bitcrush (réduction de bits + sample & hold), en place.
pub fn bitcrush_in_place(data: &mut [f32], bit_depth: u32, hold_samples: u32) {
    let steps = 2f64.powi(bit_depth.clamp(3, 16) as i32);
    let hold = hold_samples.max(1) as usize;
    let mut held = 0.0f64;
    for (i, v) in data.iter_mut().enumerate() {
        if i % hold == 0 {
            held = (*v as f64 * steps).round() / steps;
        }
        *v = held as f32;
    }
}

/// Normalisation de crête, en place ; le silence (< 0,0001) reste intact.
pub fn normalize_peak_in_place(data: &mut [f32], target: f64) {
    let peak = data.iter().fold(0.0f64, |m, v| m.max((*v as f64).abs()));
    if peak > 0.0001 {
        let g = target / peak;
        for v in data.iter_mut() {
            *v = (*v as f64 * g) as f32;
        }
    }
}
