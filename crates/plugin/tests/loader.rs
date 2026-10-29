use make_it_drum::loader::{kits_dir, load_from_path, load_from_state, scan_kits, KitState};
use make_it_drum_core::kit::KitError;
use std::io::Write;

fn pack() -> Vec<u8> {
    let wav = {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 44100,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut c = std::io::Cursor::new(Vec::new());
        let mut w = hound::WavWriter::new(&mut c, spec).unwrap();
        for i in 0..10 {
            w.write_sample((i * 100) as i16).unwrap();
        }
        w.finalize().unwrap();
        c.into_inner()
    };
    let mut c = std::io::Cursor::new(Vec::new());
    let mut z = zip::ZipWriter::new(&mut c);
    let opts =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    for n in ["makeitdrum_01.wav", "makeitdrum_02.wav"] {
        z.start_file(n, opts).unwrap();
        z.write_all(&wav).unwrap();
    }
    z.finish().unwrap();
    c.into_inner()
}

fn tmp() -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!(
        "mid-loader-{}-{}",
        std::process::id(),
        rand_suffix()
    ));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn rand_suffix() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos()
}

#[test]
fn loading_a_zip_embeds_its_bytes_in_the_state() {
    let d = tmp();
    let p = d.join("Mon kit.zip");
    std::fs::write(&p, pack()).unwrap();
    let (kit, state) = load_from_path(&p).unwrap();
    assert_eq!(kit.name, "Mon kit");
    assert_eq!(kit.pads.len(), 2);
    assert_eq!(state.path.as_deref(), Some(p.to_str().unwrap()));
    assert_eq!(state.name.as_deref(), Some("Mon kit"));
    assert!(state.zip_base64.is_some());

    // le fichier disparaît : l'état suffit
    std::fs::remove_dir_all(&d).unwrap();
    let (again, _) = load_from_state(&state).unwrap();
    assert_eq!(again, kit);
}

#[test]
fn loading_a_directory_keeps_only_the_path() {
    let d = tmp();
    std::fs::write(d.join("a.wav"), {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 44100,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut c = std::io::Cursor::new(Vec::new());
        let mut w = hound::WavWriter::new(&mut c, spec).unwrap();
        w.write_sample(0i16).unwrap();
        w.finalize().unwrap();
        c.into_inner()
    })
    .unwrap();
    let (kit, state) = load_from_path(&d).unwrap();
    assert_eq!(kit.pads.len(), 1);
    assert!(state.zip_base64.is_none());
    assert_eq!(state.path.as_deref(), Some(d.to_str().unwrap()));
    let (again, _) = load_from_state(&state).unwrap();
    assert_eq!(again.pads.len(), 1);
    std::fs::remove_dir_all(&d).unwrap();
}

#[test]
fn corrupted_base64_falls_back_to_the_path_then_errors() {
    let state = KitState {
        path: Some("/nonexistent/kit.zip".into()),
        name: Some("x".into()),
        zip_base64: Some("!!!not base64!!!".into()),
    };
    assert!(matches!(load_from_state(&state), Err(KitError::Io(_))));
    let empty = KitState::default();
    assert!(matches!(load_from_state(&empty), Err(KitError::Empty)));
}

#[test]
fn scan_lists_zip_files_sorted_and_tolerates_a_missing_dir() {
    let d = tmp();
    std::fs::write(d.join("b.zip"), b"x").unwrap();
    std::fs::write(d.join("a.kit.zip"), b"x").unwrap();
    std::fs::write(d.join("notes.txt"), b"x").unwrap();
    std::fs::create_dir(d.join("sub")).unwrap();
    let names: Vec<String> = scan_kits(&d)
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, vec!["a.kit.zip", "b.zip"]);
    assert!(scan_kits(&d.join("absent")).is_empty());
    std::fs::remove_dir_all(&d).unwrap();
}

#[test]
fn kits_dir_honours_the_environment_override() {
    std::env::set_var("MAKEITDRUM_KITS", "/tmp/kits-override");
    assert_eq!(kits_dir(), std::path::PathBuf::from("/tmp/kits-override"));
    std::env::remove_var("MAKEITDRUM_KITS");
    assert!(kits_dir().ends_with("makeitdrum/kits"));
}

#[test]
fn first_kit_is_the_first_zip_of_the_folder_or_none() {
    use make_it_drum::loader::first_kit;
    let d = tmp();
    assert_eq!(first_kit(&d), None);
    std::fs::write(d.join("b.zip"), b"x").unwrap();
    std::fs::write(d.join("a.zip"), b"x").unwrap();
    assert_eq!(first_kit(&d), Some(d.join("a.zip")));
    std::fs::remove_dir_all(&d).unwrap();
}

#[test]
fn replace_pad_builds_a_kit_from_scratch_and_embeds_it_in_the_state() {
    use make_it_drum::loader::{replace_pad, SCRATCH_KIT_NAME};
    let dir = tmp();
    let wav = dir.join("Kick 808.wav");
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 44100,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(&wav, spec).unwrap();
    for i in 0..10 {
        w.write_sample((i * 100) as i16).unwrap();
    }
    w.finalize().unwrap();

    let (kit, state) = replace_pad(None, 3, &wav).unwrap();
    assert_eq!(kit.name, SCRATCH_KIT_NAME);
    assert_eq!(kit.pads.len(), 4);
    assert_eq!(kit.pads[3].label, "Kick 808");
    assert_eq!(state.path, None);
    assert_eq!(state.name.as_deref(), Some(SCRATCH_KIT_NAME));
    let (restored, _) = load_from_state(&state).unwrap();
    assert_eq!(
        restored.pads.len(),
        4,
        "le projet se rouvre sans le fichier"
    );
    assert_eq!(restored.pads[3].sample, kit.pads[3].sample);

    // sur un kit existant : les autres pads restent, le nom du kit aussi
    let (kit2, state2) = replace_pad(Some(&kit), 0, &wav).unwrap();
    assert_eq!(kit2.pads[0].label, "Kick 808");
    assert_eq!(kit2.pads[3].label, "Kick 808");
    assert_eq!(state2.name, state.name);
    assert!(replace_pad(Some(&kit), 0, &dir.join("absent.wav")).is_err());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn generate_from_path_builds_a_full_kit_embedded_in_the_state() {
    use make_it_drum::loader::{generate_from_path, regenerate};
    use make_it_drum_core::render::Shaping;
    let dir = tmp();
    let wav = dir.join("Porte qui claque.wav");
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: 44100,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(&wav, spec).unwrap();
    for i in 0..4410 {
        let v = ((i as f32 * 0.05).sin() * 12000.0 * (1.0 - i as f32 / 4410.0)) as i16;
        w.write_sample(v).unwrap();
        w.write_sample(-v).unwrap();
    }
    w.finalize().unwrap();

    let mut calls = 0;
    let (kit, state) = generate_from_path(&wav, Shaping::default(), |_, _| calls += 1).unwrap();
    assert_eq!(calls, 20);
    assert_eq!(kit.name, "Porte qui claque");
    assert_eq!(kit.pads.len(), 20);
    assert!(kit.has_source());
    assert_eq!(state.name.as_deref(), Some("Porte qui claque"));
    let (restored, _) = load_from_state(&state).unwrap();
    assert_eq!(restored.pads[11].seed, kit.pads[11].seed);
    assert_eq!(restored.pads[11].sample, kit.pads[11].sample);
    assert!(restored.has_source(), "le son source est dans le preset");

    let (varied, _) = regenerate(&restored, Some(2), Shaping::default()).unwrap();
    assert_ne!(varied.pads[2].seed, restored.pads[2].seed);
    assert_eq!(varied.pads[3].sample, restored.pads[3].sample);
    let (all, state2) = regenerate(&varied, None, Shaping::default()).unwrap();
    assert!(all
        .pads
        .iter()
        .zip(&varied.pads)
        .all(|(a, b)| a.seed != b.seed));
    assert!(state2.zip_base64.is_some());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn reshape_keeps_seeds_and_embeds_the_shaping_in_the_state() {
    use make_it_drum::loader::{generate_from_path, load_from_state, regenerate, reshape};
    use make_it_drum_core::render::Shaping;
    let dir = tmp();
    std::fs::create_dir_all(&dir).unwrap();
    let wav = dir.join("clap.wav");
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 22050,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(&wav, spec).unwrap();
    for i in 0..11025 {
        let v = ((i as f32 * 0.05).sin() * 12000.0 * (1.0 - i as f32 / 11025.0)) as i16;
        w.write_sample(v).unwrap();
    }
    w.finalize().unwrap();

    let (kit, _) = generate_from_path(&wav, Shaping::new(0.3, 0.0), |_, _| {}).unwrap();
    assert_eq!(kit.shaping, Shaping::new(0.3, 0.0));
    let mut calls = 0;
    let (shaped, state) = reshape(&kit, Shaping::new(1.0, 0.5), |_, _| calls += 1).unwrap();
    assert_eq!(calls, 20);
    assert_eq!(shaped.shaping, Shaping::new(1.0, 0.5));
    assert!(shaped
        .pads
        .iter()
        .zip(&kit.pads)
        .all(|(a, b)| a.seed == b.seed && a.sample != b.sample));
    let (restored, _) = load_from_state(&state).unwrap();
    assert_eq!(restored.shaping, Shaping::new(1.0, 0.5));
    assert_eq!(restored.pads[4].sample, shaped.pads[4].sample);

    // regenerate adopte le cadrage demandé
    let (varied, _) = regenerate(&restored, Some(1), Shaping::new(0.0, 0.0)).unwrap();
    assert_eq!(varied.shaping, Shaping::default());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn regenerate_with_seeds_brings_a_previous_series_back_bit_for_bit() {
    use make_it_drum::loader::{generate_from_path, regenerate, regenerate_with_seeds, seeds_of};
    use make_it_drum_core::render::Shaping;
    let dir = tmp();
    std::fs::create_dir_all(&dir).unwrap();
    let wav = dir.join("snap.wav");
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 22050,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(&wav, spec).unwrap();
    for i in 0..6000 {
        let v = ((i as f32 * 0.07).sin() * 10000.0 * (1.0 - i as f32 / 6000.0)) as i16;
        w.write_sample(v).unwrap();
    }
    w.finalize().unwrap();

    let (first, _) = generate_from_path(&wav, Shaping::default(), |_, _| {}).unwrap();
    let first_seeds = seeds_of(&first);
    assert_eq!(first_seeds.len(), 20);
    assert!(first_seeds.iter().all(|s| s.is_some()));

    let (second, _) = regenerate(&first, None, Shaping::default()).unwrap();
    assert_ne!(seeds_of(&second), first_seeds);

    let mut calls = 0;
    let (back, state) =
        regenerate_with_seeds(&second, &first_seeds, Shaping::default(), |_, _| calls += 1)
            .unwrap();
    assert_eq!(calls, 20);
    assert_eq!(seeds_of(&back), first_seeds);
    for (a, b) in back.pads.iter().zip(&first.pads) {
        assert_eq!(
            a.sample, b.sample,
            "pad {} identique à la première série",
            a.index
        );
    }
    assert!(state.zip_base64.is_some());

    // un pad déposé à la main (sans graine) n'est pas touché par un retour
    let (dropped, _) = make_it_drum::loader::replace_pad(Some(&second), 4, &wav).unwrap();
    assert_eq!(dropped.pads[4].seed, None);
    let (back2, _) =
        regenerate_with_seeds(&dropped, &first_seeds, Shaping::default(), |_, _| {}).unwrap();
    assert_eq!(back2.pads[4].seed, None);
    assert_eq!(back2.pads[4].sample, dropped.pads[4].sample);
    assert_eq!(back2.pads[5].sample, first.pads[5].sample);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn set_cut_all_changes_only_the_requested_flags_on_every_pad() {
    use make_it_drum::loader::set_cut_all;
    use make_it_drum_core::kit::{Kit, PadSettings};
    let dir = tmp();
    std::fs::create_dir_all(&dir).unwrap();
    let zip = dir.join("p.zip");
    std::fs::write(&zip, pack()).unwrap();
    let (kit, _) = load_from_path(&zip).unwrap();
    let mut kit: Kit = (*kit).clone();
    kit.set_settings(
        1,
        PadSettings {
            gain_db: -6.0,
            ..Default::default()
        },
    )
    .unwrap();
    let (all, state) = set_cut_all(&kit, None, Some(true)).unwrap();
    assert!(all
        .pads
        .iter()
        .all(|p| p.settings.cut_by_all && p.settings.cut_self));
    assert_eq!(
        all.pads[1].settings.gain_db, -6.0,
        "les autres réglages restent"
    );
    assert!(state.zip_base64.is_some());
    let (none, _) = set_cut_all(&all, Some(false), Some(false)).unwrap();
    assert!(none
        .pads
        .iter()
        .all(|p| !p.settings.cut_by_all && !p.settings.cut_self));
    std::fs::remove_dir_all(&dir).ok();
}
