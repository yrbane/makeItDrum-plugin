//! Analyse spectrale du son importé, port de la page : fondamentale grave,
//! pic médium, centroïde, brillance (énergie > 3 kHz) et facteur de crête.
//! Les recettes s'appuient sur ce profil plutôt que sur des réglages fixes.

/// Profil spectral d'un son.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Analysis {
    pub centroid: f64,
    pub fundamental: f64,
    pub mid_peak: f64,
    pub brightness: f64,
    pub crest: f64,
}

pub fn clamp(v: f64, lo: f64, hi: f64) -> f64 {
    v.max(lo).min(hi)
}

/// FFT radix-2 en place (Cooley-Tukey) ; longueur puissance de deux. Les
/// tableaux sont en `f32` et le calcul en `f64`, comme les `Float32Array` de
/// la page : mêmes arrondis, mêmes résultats.
pub fn fft_in_place(re: &mut [f32], im: &mut [f32]) {
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
    let mut len = 2;
    while len <= n {
        let ang = -2.0 * std::f64::consts::PI / len as f64;
        let (wi0, wr0) = ang.sin_cos();
        let half = len / 2;
        let mut i = 0;
        while i < n {
            let (mut cur_wr, mut cur_wi) = (1.0, 0.0);
            for k in 0..half {
                let (ur, ui) = (re[i + k] as f64, im[i + k] as f64);
                let (xr, xi) = (re[i + k + half] as f64, im[i + k + half] as f64);
                let vr = xr * cur_wr - xi * cur_wi;
                let vi = xr * cur_wi + xi * cur_wr;
                re[i + k] = (ur + vr) as f32;
                im[i + k] = (ui + vi) as f32;
                re[i + k + half] = (ur - vr) as f32;
                im[i + k + half] = (ui - vi) as f32;
                let nwr = cur_wr * wr0 - cur_wi * wi0;
                let nwi = cur_wr * wi0 + cur_wi * wr0;
                cur_wr = nwr;
                cur_wi = nwi;
            }
            i += len;
        }
        len <<= 1;
    }
}

fn peak_bin(mag: &[f64], lo_hz: f64, hi_hz: f64, freq_res: f64, half: usize) -> Option<usize> {
    let lo = ((lo_hz / freq_res).floor() as usize).max(1);
    let hi = ((hi_hz / freq_res).ceil() as usize).min(half - 1);
    let mut best: Option<(usize, f64)> = None;
    for (i, &m) in mag.iter().enumerate().take(hi + 1).skip(lo) {
        if best.is_none_or(|(_, v)| m > v) {
            best = Some((i, m));
        }
    }
    best.map(|(i, _)| i)
}

/// Analyse un signal mono. Fenêtre de Hann sur les `N` premiers échantillons,
/// `N` puissance de deux entre 1024 et 16384.
pub fn analyze(data: &[f32], sample_rate: u32) -> Analysis {
    let mut n = 1024usize;
    while n * 2 <= data.len() && n < 16384 {
        n *= 2;
    }
    let mut re = vec![0.0f32; n];
    let mut im = vec![0.0f32; n];
    let len = n.min(data.len());
    for i in 0..len {
        let w = 0.5
            - 0.5 * ((2.0 * std::f64::consts::PI * i as f64) / (len as f64 - 1.0).max(1.0)).cos();
        re[i] = (data[i] as f64 * w) as f32;
    }
    fft_in_place(&mut re, &mut im);
    let half = n / 2;
    let mag: Vec<f64> = (0..half)
        .map(|i| ((re[i] as f64).hypot(im[i] as f64) as f32) as f64)
        .collect();
    let freq_res = sample_rate as f64 / n as f64;

    let (mut num, mut den) = (0.0, 0.0);
    for (i, &m) in mag.iter().enumerate().skip(1) {
        num += i as f64 * freq_res * m;
        den += m;
    }
    let centroid = if den > 0.0 { num / den } else { 2000.0 };
    let fundamental = peak_bin(&mag, 30.0, 300.0, freq_res, half)
        .map(|b| b as f64 * freq_res)
        .unwrap_or(60.0);
    let mid_peak = peak_bin(&mag, 600.0, 3000.0, freq_res, half)
        .map(|b| b as f64 * freq_res)
        .unwrap_or(1200.0);

    let (mut hi_e, mut tot_e) = (0.0, 0.0);
    for (i, &m) in mag.iter().enumerate().skip(1) {
        let e = m * m;
        tot_e += e;
        if i as f64 * freq_res > 3000.0 {
            hi_e += e;
        }
    }
    let brightness = if tot_e > 0.0 {
        clamp(hi_e / tot_e, 0.0, 1.0)
    } else {
        0.3
    };

    let (mut peak, mut sum_sq) = (0.0f64, 0.0f64);
    for &v in data {
        let v = (v as f64).abs();
        peak = peak.max(v);
        sum_sq += v * v;
    }
    let mut rms = (sum_sq / data.len().max(1) as f64).sqrt();
    if rms == 0.0 || rms.is_nan() {
        rms = 0.001;
    }
    let crest = clamp(peak / rms, 1.0, 6.0);

    Analysis {
        centroid,
        fundamental,
        mid_peak,
        brightness,
        crest,
    }
}
