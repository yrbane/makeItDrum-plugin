mod common;
use make_it_drum_core::engine::Engine;
use make_it_drum_core::kit::{Kit, BASE_NOTE};
use std::sync::Arc;

fn kit() -> Arc<Kit> {
    Arc::new(Kit::from_zip_bytes("pack", &common::web_pack(44100)).unwrap())
}

fn render(e: &mut Engine, n: usize) -> (Vec<f32>, Vec<f32>) {
    let (mut l, mut r) = (vec![0.0f32; n], vec![0.0f32; n]);
    e.render(&mut l, &mut r);
    (l, r)
}

#[test]
fn without_kit_everything_is_silent_and_nothing_plays() {
    let mut e = Engine::new(44100.0);
    e.note_on(BASE_NOTE, 1.0);
    let (l, _) = render(&mut e, 16);
    assert!(l.iter().all(|&x| x == 0.0));
    assert_eq!(e.playing_mask(), 0);
}

#[test]
fn note_on_plays_the_mapped_pad_and_lights_it() {
    let mut e = Engine::new(44100.0);
    e.set_kit(Some(kit()));
    e.note_on(BASE_NOTE + 2, 1.0);
    assert_eq!(e.playing_mask(), 1 << 2);
    let (l, r) = render(&mut e, 16);
    let expected = &e.kit().unwrap().pads[2].sample.data[..16];
    assert_eq!(l, expected);
    assert_eq!(r, expected);
}

#[test]
fn unmapped_notes_and_zero_velocity_are_ignored() {
    let mut e = Engine::new(44100.0);
    e.set_kit(Some(kit()));
    e.note_on(BASE_NOTE + 20, 1.0);
    e.note_on(35, 1.0);
    e.note_on(BASE_NOTE, 0.0);
    assert_eq!(e.playing_mask(), 0);
}

#[test]
fn pads_are_polyphonic_and_mix_additively() {
    let mut e = Engine::new(44100.0);
    e.set_kit(Some(kit()));
    e.note_on(BASE_NOTE, 1.0);
    e.note_on(BASE_NOTE + 1, 0.5);
    let (l, _) = render(&mut e, 8);
    let k = e.kit().unwrap();
    for (i, got) in l.iter().enumerate() {
        let want = k.pads[0].sample.data[i] + 0.5 * k.pads[1].sample.data[i];
        assert!((got - want).abs() < 1e-6);
    }
}

#[test]
fn host_rate_changes_the_playback_ratio() {
    let mut e = Engine::new(22050.0);
    e.set_kit(Some(kit()));
    e.note_on(BASE_NOTE, 1.0);
    let (l, _) = render(&mut e, 4);
    let d = &e.kit().unwrap().pads[0].sample.data;
    assert_eq!(
        l,
        vec![d[0], d[2], d[4], d[6]],
        "44,1 kHz joué à 22,05 kHz : un échantillon sur deux"
    );
}

#[test]
fn voices_end_and_the_mask_clears() {
    let mut e = Engine::new(44100.0);
    e.set_kit(Some(kit()));
    e.note_on(BASE_NOTE, 1.0);
    render(&mut e, 441);
    assert_eq!(e.playing_mask(), 0);
}

#[test]
fn set_kit_cuts_running_voices() {
    let mut e = Engine::new(44100.0);
    e.set_kit(Some(kit()));
    e.note_on(BASE_NOTE, 1.0);
    e.set_kit(Some(kit()));
    assert_eq!(e.playing_mask(), 0);
    e.set_kit(None);
    assert!(e.kit().is_none());
}

#[test]
fn trigger_pad_bypasses_the_note_mapping() {
    let mut e = Engine::new(44100.0);
    e.set_kit(Some(kit()));
    e.trigger_pad(19, 1.0);
    e.trigger_pad(25, 1.0);
    assert_eq!(e.playing_mask(), 1 << 19);
}

// Golden : séquence de notes sur le pack synthétique, comparée à l'enregistrement de référence.
#[test]
fn golden_sequence() {
    let mut e = Engine::new(48000.0);
    e.set_kit(Some(kit()));
    let mut out_l = Vec::new();
    let mut out_r = Vec::new();
    let events: &[(usize, u8, f32)] =
        &[(0, 36, 1.0), (100, 40, 0.5), (200, 36, 0.8), (300, 55, 1.0)];
    let mut cursor = 0;
    for &(at, note, vel) in events.iter().chain([(1000usize, 0u8, 0.0f32)].iter()) {
        if at > cursor {
            let (l, r) = render(&mut e, at - cursor);
            out_l.extend(l);
            out_r.extend(r);
            cursor = at;
        }
        if note > 0 {
            e.note_on(note, vel);
        }
    }
    let mut bytes = Vec::with_capacity(out_l.len() * 8);
    for (l, r) in out_l.iter().zip(&out_r) {
        bytes.extend_from_slice(&l.to_le_bytes());
        bytes.extend_from_slice(&r.to_le_bytes());
    }
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/engine_sequence.bin");
    if std::env::var("UPDATE_GOLDEN").is_ok() || !path.exists() {
        std::fs::write(&path, &bytes).unwrap();
        panic!("golden écrit ({} octets) — relancer", bytes.len());
    }
    let reference = std::fs::read(&path).unwrap();
    assert_eq!(reference.len(), bytes.len());
    for (i, (a, b)) in reference.chunks(4).zip(bytes.chunks(4)).enumerate() {
        let (a, b) = (
            f32::from_le_bytes(a.try_into().unwrap()),
            f32::from_le_bytes(b.try_into().unwrap()),
        );
        assert!(
            (a - b).abs() <= 1e-6,
            "divergence à l'échantillon {i} : {a} vs {b}"
        );
    }
}
