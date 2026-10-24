// Les valeurs de référence sont celles du JS de la page, recopiées telles quelles.
#![allow(clippy::excessive_precision)]
// Les briques pures du moteur contre les valeurs calculées par le JS de la page.
mod common;
use make_it_drum_core::analysis::{analyze, fft_in_place};
use make_it_drum_core::curves::{curve_at, env_curve, fold_curve, sat_curve, shape};
use make_it_drum_core::prng::{Mulberry32, Random};

fn close(a: f64, b: f64, rel: f64) -> bool {
    (a - b).abs() <= rel * b.abs().max(1e-9)
}

#[test]
fn mulberry32_reproduces_the_page_exactly() {
    let mut r = Mulberry32::new(42);
    let got: Vec<f64> = (0..8).map(|_| r.next_f64()).collect();
    let expected = [
        0.6011037519201636,
        0.44829055899754167,
        0.8524657934904099,
        0.6697340414393693,
        0.17481389874592423,
        0.5265925421845168,
        0.2732279943302274,
        0.6247446539346129,
    ];
    assert_eq!(got, expected);
    let mut r = Mulberry32::new(123456789);
    assert_eq!(r.next_f64(), 0.2577907438389957);
    assert_eq!(r.next_f64(), 0.9707721115555614);
}

#[test]
fn random_helpers_follow_the_page() {
    let mut r = Random::from_seed(42);
    // jitter(2.0, 0.25) avec r = 0.6011… → 2.0 × (1 + (−0.25 + 0.5 × 0.6011…))
    assert!(close(
        r.jitter(2.0, 0.25),
        2.0 * (1.0 + (-0.25 + 0.5 * 0.6011037519201636)),
        1e-12
    ));
    assert!(r.chance(1.0));
    assert_eq!(r.random_offset(0.0, 0.7), 0.0);
    assert_eq!(
        r.random_offset(0.01, 0.7),
        0.0,
        "fenêtre nulle : aucun tirage"
    );
    let before = r.clone();
    let off = r.random_offset(0.5, 0.6);
    let mut again = before;
    assert_eq!(off, again.random() * (0.5 * 0.6 - 0.01));
}

#[test]
fn analysis_matches_the_page_on_the_synthetic_and_fixture_sounds() {
    let n = 22050;
    let data: Vec<f32> = (0..n)
        .map(|i| ((i as f64 * 0.01).sin() * 0.8) as f32)
        .collect();
    let a = analyze(&data, 44100);
    assert!(close(a.centroid, 70.18850019464111, 1e-3), "{a:?}");
    assert!(close(a.fundamental, 69.98291015625, 1e-6), "{a:?}");
    assert!(close(a.mid_peak, 619.07958984375, 1e-6), "{a:?}");
    assert!(a.brightness < 1e-9, "{a:?}");
    assert!(close(a.crest, 1.415706484529097, 1e-5), "{a:?}");

    let wav = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/source.wav"
    ))
    .unwrap();
    let s = make_it_drum_core::wav::decode_wav(&wav).unwrap();
    assert_eq!((s.channels, s.sample_rate), (1, 22050));
    let a = analyze(&s.data, s.sample_rate);
    assert!(close(a.centroid, 3212.9777983199583, 1e-3), "{a:?}");
    assert!(close(a.fundamental, 180.340576171875, 1e-6), "{a:?}");
    assert!(close(a.mid_peak, 732.12890625, 1e-6), "{a:?}");
    assert!(close(a.brightness, 0.0013230787667494355, 1e-3), "{a:?}");
    assert_eq!(a.crest, 6.0);
}

#[test]
fn fft_of_a_pure_tone_peaks_in_its_bin() {
    let n = 1024;
    let mut re: Vec<f32> = (0..n)
        .map(|i| (2.0 * std::f64::consts::PI * 10.0 * i as f64 / n as f64).sin() as f32)
        .collect();
    let mut im = vec![0.0f32; n];
    fft_in_place(&mut re, &mut im);
    let mag: Vec<f64> = (0..n / 2)
        .map(|i| (re[i] as f64).hypot(im[i] as f64))
        .collect();
    let best = (0..n / 2)
        .max_by(|&a, &b| mag[a].partial_cmp(&mag[b]).unwrap())
        .unwrap();
    assert_eq!(best, 10);
    assert!(close(mag[10], n as f64 / 2.0, 1e-5));
}

#[test]
fn curves_match_the_page() {
    let sat = sat_curve(2.4, 0.16);
    for (i, v) in [
        (0, -0.9811582565307617f32),
        (100, -0.9486410617828369),
        (511, -0.002384598832577467),
        (900, 0.9830827116966248),
        (1023, 1.0088626146316528),
    ] {
        assert!((sat[i] - v).abs() < 1e-6, "sat[{i}] = {} ≠ {v}", sat[i]);
    }
    let fold = fold_curve(0.35);
    for (i, v) in [
        (0, 0.8571428656578064f32),
        (100, 0.2985616624355316),
        (900, -0.17008797824382782),
        (1023, -0.8571428656578064),
    ] {
        assert!((fold[i] - v).abs() < 1e-6, "fold[{i}] = {} ≠ {v}", fold[i]);
    }
    let env = env_curve(0.0008, 0.02, 0.5, 1.8);
    for (i, v) in [
        (0, 0.0f32),
        (1, 1.0),
        (20, 1.0),
        (100, 0.7271422743797302),
        (300, 0.21898408234119415),
        (511, 0.0001),
    ] {
        assert!((env[i] - v).abs() < 1e-6, "env[{i}] = {} ≠ {v}", env[i]);
    }
}

#[test]
fn shaper_and_automation_lookups_interpolate_and_clamp() {
    let curve: Vec<f32> = vec![-1.0, 0.0, 1.0];
    assert_eq!(shape(&curve, -2.0), -1.0);
    assert_eq!(shape(&curve, 2.0), 1.0);
    assert!((shape(&curve, 0.5) - 0.5).abs() < 1e-9);
    assert!((shape(&curve, -0.25) + 0.25).abs() < 1e-9);
    let env: Vec<f32> = vec![0.0, 1.0, 0.5];
    assert_eq!(curve_at(&env, 1.0, 0.0), 0.0);
    assert!((curve_at(&env, 1.0, 0.25) - 0.5).abs() < 1e-9);
    assert!((curve_at(&env, 1.0, 0.75) - 0.75).abs() < 1e-9);
    assert_eq!(
        curve_at(&env, 1.0, 3.0),
        0.5,
        "après la fin : dernière valeur"
    );
}
