//! Tables pour les WaveShaper et enveloppes d'amplitude, port de la page.
use crate::analysis::clamp;

/// Saturation tanh, avec asymétrie optionnelle (harmoniques paires). 1024 points.
pub fn sat_curve(drive: f64, asym: f64) -> Vec<f32> {
    let k = drive.max(0.3);
    let n = 1024;
    let norm = k.tanh();
    (0..n)
        .map(|i| {
            let x = (i as f64 / (n - 1) as f64) * 2.0 - 1.0;
            ((k * (x + asym * x * x)).tanh() / norm) as f32
        })
        .collect()
}

/// Enveloppe : attaque, palier, puis chute en courbe de puissance. 512 points.
pub fn env_curve(attack: f64, hold: f64, decay: f64, curve_pow: f64) -> Vec<f32> {
    let curve_pow = if curve_pow == 0.0 { 2.0 } else { curve_pow };
    let n = 512;
    let total = attack + hold + decay;
    let mut curve: Vec<f32> = (0..n)
        .map(|i| {
            let t = (i as f64 / (n - 1) as f64) * total;
            let v = if t < attack {
                if attack > 0.0 {
                    t / attack
                } else {
                    1.0
                }
            } else if t < attack + hold {
                1.0
            } else {
                let dt = if decay > 0.0 {
                    (t - attack - hold) / decay
                } else {
                    1.0
                };
                (1.0 - dt.min(1.0)).powf(curve_pow)
            };
            v as f32
        })
        .collect();
    curve[n - 1] = 0.0001;
    curve
}

/// Enveloppe dont le pic est mis à l'échelle `peak`.
pub fn scaled_env(attack: f64, hold: f64, decay: f64, curve_pow: f64, peak: f64) -> Vec<f32> {
    let mut c = env_curve(attack, hold, decay, curve_pow);
    if peak != 1.0 {
        for v in &mut c {
            *v = (*v as f64 * peak) as f32;
        }
    }
    c
}

/// Wavefolder : distorsion par repliement. 1024 points.
pub fn fold_curve(threshold: f64) -> Vec<f32> {
    let th = clamp(threshold, 0.08, 0.9);
    let n = 1024;
    (0..n)
        .map(|i| {
            let x = (i as f64 / (n - 1) as f64) * 2.0 - 1.0;
            let mut y = x;
            let mut guard = 0;
            while y.abs() > th && guard < 8 {
                y = if y > th { 2.0 * th - y } else { -2.0 * th - y };
                guard += 1;
            }
            (y / th) as f32
        })
        .collect()
}

/// Lecture d'une table WaveShaper comme Web Audio : `x` dans [-1, 1] sur les
/// `n` points, interpolation linéaire, saturation aux bornes.
#[inline]
pub fn shape(curve: &[f32], x: f64) -> f64 {
    let n = curve.len();
    if n == 0 {
        return x;
    }
    if n == 1 {
        return curve[0] as f64;
    }
    let v = (x + 1.0) * 0.5 * (n - 1) as f64;
    if v <= 0.0 {
        return curve[0] as f64;
    }
    if v >= (n - 1) as f64 {
        return curve[n - 1] as f64;
    }
    let i = v.floor() as usize;
    let frac = v - i as f64;
    curve[i] as f64 + (curve[i + 1] as f64 - curve[i] as f64) * frac
}

/// Valeur d'une automation `setValueCurveAtTime(curve, 0, duration)` à `t` :
/// interpolation linéaire entre les points, dernière valeur ensuite.
#[inline]
pub fn curve_at(curve: &[f32], duration: f64, t: f64) -> f64 {
    let n = curve.len();
    if n == 0 {
        return 1.0;
    }
    if t <= 0.0 || duration <= 0.0 {
        return curve[0] as f64;
    }
    let k = (n - 1) as f64 * t / duration;
    if k >= (n - 1) as f64 {
        return curve[n - 1] as f64;
    }
    let i = k.floor() as usize;
    let frac = k - i as f64;
    curve[i] as f64 + (curve[i + 1] as f64 - curve[i] as f64) * frac
}
