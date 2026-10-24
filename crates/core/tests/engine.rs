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

// ---- réglages par pad, étouffement, variantes, mode mélodique

fn kit_with(f: impl FnOnce(&mut Kit)) -> Arc<Kit> {
    let mut k = Kit::from_zip_bytes("pack", &common::web_pack(44100)).unwrap();
    f(&mut k);
    Arc::new(k)
}

#[test]
fn pad_gain_and_velocity_multiply_and_pan_attenuates_one_side_only() {
    use make_it_drum_core::kit::PadSettings;
    let mut e = Engine::new(44100.0);
    e.set_kit(Some(kit_with(|k| {
        k.set_settings(
            0,
            PadSettings {
                gain_db: -6.0,
                pan: -0.5,
                ..Default::default()
            },
        )
        .unwrap();
    })));
    e.trigger_pad(0, 0.5);
    let (l, r) = render(&mut e, 8);
    let k = e.kit().unwrap();
    let g = 0.5 * 10f32.powf(-6.0 / 20.0);
    for i in 0..8 {
        let v = k.pads[0].sample.data[i];
        assert!((l[i] - v * g).abs() < 1e-6, "gauche entière");
        assert!((r[i] - v * g * 0.5).abs() < 1e-6, "droite à moitié");
    }
}

#[test]
fn pitch_changes_the_playback_step() {
    use make_it_drum_core::kit::PadSettings;
    let mut e = Engine::new(44100.0);
    e.set_kit(Some(kit_with(|k| {
        k.set_settings(
            0,
            PadSettings {
                pitch: 12.0,
                ..Default::default()
            },
        )
        .unwrap();
    })));
    e.trigger_pad(0, 1.0);
    let (l, _) = render(&mut e, 4);
    let d = &e.kit().unwrap().pads[0].sample.data;
    for i in 0..4 {
        assert!(
            (l[i] - d[2 * i]).abs() < 1e-6,
            "une octave au-dessus : une trame sur deux"
        );
    }
}

#[test]
fn decay_fades_out_before_the_end_and_reverse_reads_backwards() {
    use make_it_drum_core::kit::PadSettings;
    let mut e = Engine::new(44100.0);
    e.set_kit(Some(kit_with(|k| {
        k.set_settings(
            0,
            PadSettings {
                decay: 0.5,
                ..Default::default()
            },
        )
        .unwrap();
        k.set_settings(
            1,
            PadSettings {
                reverse: true,
                ..Default::default()
            },
        )
        .unwrap();
    })));
    let frames = e.kit().unwrap().pads[0].sample.frames();
    e.trigger_pad(0, 1.0);
    let (_, _) = render(&mut e, frames / 2 + 2);
    assert_eq!(e.playing_mask(), 0, "fini à la moitié du son");
    e.trigger_pad(1, 1.0);
    let (l, _) = render(&mut e, 4);
    let d = &e.kit().unwrap().pads[1].sample.data;
    let n = d.len();
    for i in 0..4 {
        assert!((l[i] - d[n - 1 - i]).abs() < 1e-6, "lecture depuis la fin");
    }
}

#[test]
fn choke_group_cuts_the_other_pads_of_the_group_with_a_short_fade() {
    use make_it_drum_core::kit::PadSettings;
    use make_it_drum_core::voice::CHOKE_FADE_FRAMES;
    let mut e = Engine::new(44100.0);
    e.set_kit(Some(kit_with(|k| {
        for i in [2, 3] {
            k.set_settings(
                i,
                PadSettings {
                    choke: 1,
                    ..Default::default()
                },
            )
            .unwrap();
        }
        k.set_settings(
            4,
            PadSettings {
                choke: 2,
                ..Default::default()
            },
        )
        .unwrap();
    })));
    e.trigger_pad(2, 1.0);
    e.trigger_pad(4, 1.0);
    e.trigger_pad(0, 1.0);
    e.trigger_pad(3, 1.0);
    assert_eq!(
        e.playing_mask(),
        0b11101,
        "le pad 2 s'éteint en fondu, encore actif"
    );
    render(&mut e, CHOKE_FADE_FRAMES as usize + 1);
    assert_eq!(e.playing_mask() & (1 << 2), 0, "pad 2 étouffé");
    assert_ne!(e.playing_mask() & (1 << 3), 0, "pad 3 joue");
    assert_ne!(e.playing_mask() & (1 << 4), 0, "autre groupe, intact");
    assert_ne!(e.playing_mask() & 1, 0, "sans groupe, intact");
}

#[test]
fn round_robin_cycles_through_the_alternates() {
    let mut e = Engine::new(44100.0);
    e.set_kit(Some(kit_with(|k| {
        let alt = k.pads[1].sample.clone();
        k.pads[0].alternates.push((7, alt));
    })));
    let k = e.kit().unwrap().clone();
    e.trigger_pad(0, 1.0);
    let (a, _) = render(&mut e, 4);
    assert_eq!(a, k.pads[0].sample.data[..4]);
    e.trigger_pad(0, 1.0);
    let (b, _) = render(&mut e, 4);
    assert_eq!(
        b,
        k.pads[1].sample.data[..4],
        "deuxième frappe : la variante"
    );
    e.trigger_pad(0, 1.0);
    let (c, _) = render(&mut e, 4);
    assert_eq!(
        c,
        k.pads[0].sample.data[..4],
        "troisième : retour au principal"
    );
}

#[test]
fn melodic_mode_plays_one_pad_on_the_whole_keyboard_transposed() {
    use make_it_drum_core::engine::MELODIC_ROOT;
    let mut e = Engine::new(44100.0);
    e.set_kit(Some(kit()));
    e.set_melodic(Some(5));
    e.note_on(MELODIC_ROOT, 1.0);
    assert_eq!(
        e.playing_mask(),
        1 << 5,
        "do médian : le pad choisi, tel quel"
    );
    let (l, _) = render(&mut e, 4);
    let d = e.kit().unwrap().pads[5].sample.data.clone();
    assert_eq!(l, d[..4]);
    e.set_kit(Some(kit()));
    e.set_melodic(Some(5));
    e.note_on(MELODIC_ROOT + 12, 1.0);
    let (l, _) = render(&mut e, 4);
    for i in 0..4 {
        assert!(
            (l[i] - d[2 * i]).abs() < 1e-6,
            "octave : deux fois plus vite"
        );
    }
    e.note_on(36, 1.0);
    assert_eq!(e.playing_mask(), 1 << 5, "la note 36 ne joue plus le pad 1");
    e.set_melodic(None);
    e.note_on(36, 1.0);
    assert_ne!(e.playing_mask() & 1, 0, "retour au mode pads");
}
