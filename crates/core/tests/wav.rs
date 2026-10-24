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

#[test]
fn decode_audio_reads_an_aiff_like_the_same_wav() {
    use make_it_drum_core::wav::decode_audio;
    let wav = decode_wav(&common::sine_wav(440.0, 200, 44100, 2, 16)).unwrap();
    let aiff = decode_audio(&common::sine_aiff(440.0, 200, 44100, 2), Some("aif")).unwrap();
    assert_eq!(aiff.channels, 2);
    assert_eq!(aiff.sample_rate, 44100);
    assert_eq!(aiff.frames(), 200);
    for (a, w) in aiff.data.iter().zip(&wav.data) {
        assert!((a - w).abs() < 1e-4, "{a} vs {w}");
    }
}

#[test]
fn decode_audio_still_reads_wav_and_rejects_garbage() {
    use make_it_drum_core::wav::{decode_audio, is_audio_file};
    let s = decode_audio(&common::sine_wav(100.0, 10, 48000, 1, 24), Some("wav")).unwrap();
    assert_eq!(s.frames(), 10);
    assert!(decode_audio(b"pas du son du tout", Some("mp3")).is_err());
    assert!(decode_audio(b"pas du son du tout", None).is_err());
    assert!(is_audio_file("Kick 808.aif"));
    assert!(is_audio_file("snare.WAV"));
    assert!(is_audio_file("loop.flac"));
    assert!(!is_audio_file("makeitdrum_pack.zip"));
    assert!(!is_audio_file("notes.txt"));
}

#[test]
fn decode_audio_reads_the_m4a_that_live_records() {
    use make_it_drum_core::wav::{decode_audio, decode_wav, is_audio_file};
    let wav = decode_wav(include_bytes!("fixtures/source.wav")).unwrap();
    let m4a = decode_audio(include_bytes!("fixtures/source.m4a"), Some("m4a")).unwrap();
    assert_eq!(m4a.channels, wav.channels);
    assert_eq!(m4a.sample_rate, wav.sample_rate);
    // L'AAC ajoute un délai d'encodage et arrondit à la trame : on tolère un écart modéré.
    let diff = (m4a.frames() as i64 - wav.frames() as i64).abs();
    assert!(
        diff < 4096,
        "trames : {} contre {}",
        m4a.frames(),
        wav.frames()
    );
    let peak = m4a.data.iter().fold(0f32, |m, v| m.max(v.abs()));
    assert!(peak > 0.05, "le m4a décodé est silencieux ({peak})");
    assert!(is_audio_file("Aveu robin.m4a"));
    assert!(is_audio_file("clip.MP4"));
}

#[test]
fn decode_audio_reads_a_flac_sample_for_sample() {
    use make_it_drum_core::wav::{decode_audio, decode_wav};
    let wav = decode_wav(include_bytes!("fixtures/source.wav")).unwrap();
    let flac = decode_audio(include_bytes!("fixtures/source.flac"), Some("flac")).unwrap();
    assert_eq!(flac.channels, wav.channels);
    assert_eq!(flac.sample_rate, wav.sample_rate);
    assert_eq!(flac.frames(), wav.frames());
    // Sans perte : même signal à la quantification 16 bits près.
    for (a, b) in flac.data.iter().zip(&wav.data) {
        assert!((a - b).abs() < 1.0 / 32768.0, "{a} contre {b}");
    }
}
