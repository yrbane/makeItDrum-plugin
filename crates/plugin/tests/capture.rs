use make_it_drum::capture::{Recorder, MAX_SECONDS};

#[test]
fn records_the_requested_length_mixed_to_mono_then_hands_it_over_once() {
    let r = Recorder::new();
    assert!(!r.is_recording() && !r.is_done());
    r.push(&[&[1.0, 1.0]]);
    assert!(r.take().is_none(), "rien sans armement");

    r.arm(0.001, 8000.0); // 8 trames
    assert!(r.is_recording());
    let l = [0.2f32; 5];
    let rr = [0.4f32; 5];
    r.push(&[&l, &rr]);
    assert!((r.progress() - 5.0 / 8.0).abs() < 1e-6);
    assert!(r.is_recording());
    r.push(&[&l, &rr]);
    assert!(r.is_done(), "8 trames atteintes, le surplus est ignoré");
    assert!(!r.is_recording());
    r.push(&[&l, &rr]);
    let s = r.take().unwrap();
    assert_eq!(s.channels, 1);
    assert_eq!(s.sample_rate, 8000);
    assert_eq!(s.frames(), 8);
    assert!(
        s.data.iter().all(|v| (v - 0.3).abs() < 1e-6),
        "moyenne des canaux"
    );
    assert!(r.take().is_none(), "une seule fois");
    assert!(!r.is_done());
}

#[test]
fn arm_is_ignored_while_busy_and_cancel_resets() {
    let r = Recorder::new();
    r.arm(0.001, 8000.0);
    r.push(&[&[0.5f32; 3]]);
    r.arm(1.0, 8000.0);
    assert!(
        (r.progress() - 3.0 / 8.0).abs() < 1e-6,
        "réarmer en cours n'efface rien"
    );
    r.cancel();
    assert!(!r.is_recording() && !r.is_done());
    assert!(r.take().is_none());
    r.arm(100.0, 1000.0);
    assert!(r.is_recording());
    let block = [0.1f32; 1000];
    for _ in 0..(MAX_SECONDS as usize + 1) {
        r.push(&[&block]);
    }
    assert!(r.is_done(), "borné à MAX_SECONDS");
    assert_eq!(r.take().unwrap().frames(), (MAX_SECONDS * 1000.0) as usize);
}
