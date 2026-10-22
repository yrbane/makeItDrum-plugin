mod common;
use make_it_drum_core::wav::decode_wav;

#[test]
fn decode_mono_16_bits() {
    let s = decode_wav(&common::sine_wav(440.0, 100, 44100, 1, 16)).unwrap();
    assert_eq!(s.channels, 1);
    assert_eq!(s.sample_rate, 44100);
    assert_eq!(s.frames(), 100);
    assert_eq!(s.data[0], 0.0);
    let expected = 0.5 * (440.0 * std::f32::consts::TAU / 44100.0f32).sin();
    assert!(
        (s.data[1] - expected).abs() < 1e-4,
        "{} vs {expected}",
        s.data[1]
    );
}

#[test]
fn decode_stereo_24_bits_keeps_interleaving() {
    let s = decode_wav(&common::sine_wav(440.0, 10, 48000, 2, 24)).unwrap();
    assert_eq!(s.channels, 2);
    assert_eq!(s.frames(), 10);
    assert_eq!(s.data.len(), 20);
    let (l, r) = s.frame(1);
    assert!((l + r).abs() < 1e-6, "canal droit = opposé du gauche");
    assert!(l > 0.0);
}

#[test]
fn decode_float_32_bits() {
    let s = decode_wav(&common::sine_wav(440.0, 10, 44100, 1, 32)).unwrap();
    assert!((s.data[1] - 0.5 * (440.0 * std::f32::consts::TAU / 44100.0f32).sin()).abs() < 1e-7);
}

#[test]
fn garbage_is_an_error_not_a_panic() {
    assert!(decode_wav(b"not a wav at all").is_err());
}

#[test]
fn frame_of_mono_sample_duplicates_the_channel() {
    let s = decode_wav(&common::sine_wav(440.0, 4, 44100, 1, 16)).unwrap();
    let (l, r) = s.frame(1);
    assert_eq!(l, r);
}
