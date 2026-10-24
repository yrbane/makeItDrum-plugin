use make_it_drum::editor::{note_name, waveform_columns};
use make_it_drum_core::wav::Sample;

#[test]
fn note_names_follow_live_convention_c3_is_middle_c() {
    assert_eq!(note_name(36), "C1");
    assert_eq!(note_name(37), "C#1");
    assert_eq!(note_name(55), "G2");
    assert_eq!(note_name(60), "C3");
    assert_eq!(note_name(0), "C-2");
    assert_eq!(note_name(127), "G8");
}

#[test]
fn waveform_columns_keep_min_and_max_per_column_and_handle_stereo_and_empty() {
    let s = Sample {
        data: (0..100)
            .map(|i| if i % 2 == 0 { 0.5 } else { -0.25 })
            .collect(),
        channels: 1,
        sample_rate: 44100,
    };
    let w = waveform_columns(&s, 10);
    assert_eq!(w.len(), 10);
    assert!(w.iter().all(|&(lo, hi)| lo == -0.25 && hi == 0.5));
    let stereo = Sample {
        data: vec![0.9, -0.9, 0.1, -0.1],
        channels: 2,
        sample_rate: 44100,
    };
    assert_eq!(
        waveform_columns(&stereo, 2),
        vec![(0.9, 0.9), (0.1, 0.1)],
        "canal gauche"
    );
    assert!(waveform_columns(&Sample::silent(), 10).is_empty());
    assert!(waveform_columns(&s, 0).is_empty());
    let loud = Sample {
        data: vec![3.0, -3.0],
        channels: 1,
        sample_rate: 44100,
    };
    assert_eq!(waveform_columns(&loud, 1), vec![(-1.0, 1.0)], "borné à ±1");
}
