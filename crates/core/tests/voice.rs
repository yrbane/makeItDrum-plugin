mod common;
use make_it_drum_core::voice::Voice;
use make_it_drum_core::wav::{decode_wav, Sample};

fn ramp(frames: usize, channels: u16) -> Sample {
    let mut data = Vec::new();
    for i in 0..frames {
        for ch in 0..channels {
            data.push(if ch == 0 { i as f32 } else { -(i as f32) });
        }
    }
    Sample {
        data,
        channels,
        sample_rate: 44100,
    }
}

#[test]
fn new_voice_is_silent_and_inactive() {
    let mut v = Voice::new();
    assert!(!v.is_active());
    let (mut l, mut r) = (vec![1.0f32; 4], vec![1.0f32; 4]);
    v.render(&ramp(8, 1), &mut l, &mut r);
    assert_eq!(l, vec![1.0; 4], "rien n'est ajouté");
}

#[test]
fn trigger_plays_at_ratio_one_with_velocity_gain_and_adds_to_output() {
    let mut v = Voice::new();
    v.trigger(0.5, 1.0);
    let (mut l, mut r) = (vec![1.0f32; 4], vec![0.0f32; 4]);
    v.render(&ramp(8, 1), &mut l, &mut r);
    assert_eq!(l, vec![1.0, 1.5, 2.0, 2.5]);
    assert_eq!(r, vec![0.0, 0.5, 1.0, 1.5], "mono dupliqué à droite");
    assert!(v.is_active());
}

#[test]
fn stereo_sample_goes_to_both_channels_separately() {
    let mut v = Voice::new();
    v.trigger(1.0, 1.0);
    let (mut l, mut r) = (vec![0.0f32; 3], vec![0.0f32; 3]);
    v.render(&ramp(8, 2), &mut l, &mut r);
    assert_eq!(l, vec![0.0, 1.0, 2.0]);
    assert_eq!(r, vec![0.0, -1.0, -2.0]);
}

#[test]
fn ratio_above_one_interpolates_linearly_and_ends_sooner() {
    let mut v = Voice::new();
    v.trigger(1.0, 1.5); // 48 kHz joué à 32 kHz, par exemple
    let (mut l, mut r) = (vec![0.0f32; 8], vec![0.0f32; 8]);
    v.render(&ramp(6, 1), &mut l, &mut r);
    assert_eq!(&l[..4], &[0.0, 1.5, 3.0, 4.5]);
    assert_eq!(&l[4..], &[0.0; 4], "au-delà de la fin : silence");
    assert!(!v.is_active(), "voix libérée à la fin du sample");
}

#[test]
fn last_frame_is_held_against_the_end_not_against_zero() {
    let mut v = Voice::new();
    v.trigger(1.0, 0.5);
    let (mut l, mut r) = (vec![0.0f32; 5], vec![0.0f32; 5]);
    v.render(&ramp(3, 1), &mut l, &mut r);
    // positions 0, 0.5, 1, 1.5, 2 → 0, 0.5, 1, 1.5, 2 ; puis fin
    assert_eq!(l, vec![0.0, 0.5, 1.0, 1.5, 2.0]);
}

#[test]
fn retrigger_restarts_from_the_beginning() {
    let mut v = Voice::new();
    v.trigger(1.0, 1.0);
    let (mut l, mut r) = (vec![0.0f32; 2], vec![0.0f32; 2]);
    v.render(&ramp(8, 1), &mut l, &mut r);
    v.trigger(1.0, 1.0);
    let (mut l, mut r) = (vec![0.0f32; 2], vec![0.0f32; 2]);
    v.render(&ramp(8, 1), &mut l, &mut r);
    assert_eq!(l, vec![0.0, 1.0]);
}

#[test]
fn render_spans_several_blocks() {
    let mut v = Voice::new();
    v.trigger(1.0, 1.0);
    let s = ramp(5, 1);
    let (mut l, mut r) = (vec![0.0f32; 3], vec![0.0f32; 3]);
    v.render(&s, &mut l, &mut r);
    let (mut l2, mut r2) = (vec![0.0f32; 3], vec![0.0f32; 3]);
    v.render(&s, &mut l2, &mut r2);
    assert_eq!(l, vec![0.0, 1.0, 2.0]);
    assert_eq!(l2, vec![3.0, 4.0, 0.0]);
    assert!(!v.is_active());
}

#[test]
fn stop_silences_immediately() {
    let mut v = Voice::new();
    v.trigger(1.0, 1.0);
    v.stop();
    assert!(!v.is_active());
}

#[test]
fn works_with_a_decoded_wav() {
    let s = decode_wav(&common::sine_wav(440.0, 50, 44100, 1, 16)).unwrap();
    let mut v = Voice::new();
    v.trigger(1.0, 1.0);
    let (mut l, mut r) = (vec![0.0f32; 50], vec![0.0f32; 50]);
    v.render(&s, &mut l, &mut r);
    assert_eq!(l, s.data);
}
