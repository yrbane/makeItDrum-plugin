mod common;
use make_it_drum_core::kit::{Kit, KitError, BASE_NOTE, PAD_COUNT};

#[test]
fn web_pack_without_manifest_maps_20_pads_to_gm_notes() {
    let kit = Kit::from_zip_bytes("makeitdrum_pack", &common::web_pack(44100)).unwrap();
    assert_eq!(kit.name, "makeitdrum_pack");
    assert_eq!(kit.pads.len(), PAD_COUNT);
    assert_eq!(
        kit.pads[0].label, "BOOM",
        "pack web reconnu : nom de la recette"
    );
    assert_eq!(kit.pads[0].note, BASE_NOTE);
    assert_eq!(kit.pads[19].note, BASE_NOTE + 19);
    assert_eq!(kit.pads[19].index, 19);
    assert_eq!(kit.pads[3].sample.sample_rate, 44100);
}

#[test]
fn pack_with_fewer_wavs_leaves_missing_pads_out() {
    let a = common::sine_wav(100.0, 10, 44100, 1, 16);
    let b = common::sine_wav(200.0, 10, 44100, 1, 16);
    let zip = common::zip_store(&[("b.wav", &b), ("a.wav", &a), ("notes.txt", b"hello")]);
    let kit = Kit::from_zip_bytes("mini", &zip).unwrap();
    assert_eq!(kit.pads.len(), 2, "tri par nom, fichiers non WAV ignorés");
    assert_eq!(kit.pads[0].label, "a");
    assert_eq!(kit.pads[1].label, "b");
    assert_eq!(kit.pad_for_note(BASE_NOTE + 1), Some(1));
    assert_eq!(kit.pad_for_note(BASE_NOTE + 2), None);
}

#[test]
fn pack_with_more_than_20_wavs_keeps_the_first_20() {
    let w = common::sine_wav(100.0, 10, 44100, 1, 16);
    let names: Vec<String> = (0..25).map(|i| format!("{i:02}.wav")).collect();
    let entries: Vec<(&str, &[u8])> = names.iter().map(|n| (n.as_str(), w.as_slice())).collect();
    let kit = Kit::from_zip_bytes("big", &common::zip_store(&entries)).unwrap();
    assert_eq!(kit.pads.len(), PAD_COUNT);
    assert_eq!(kit.pads[19].label, "19");
}

#[test]
fn manifest_overrides_name_labels_and_notes() {
    let w = common::sine_wav(100.0, 10, 44100, 1, 16);
    let manifest = r#"{"schemaVersion":1,"app":"make-it-drum","name":"Casserole bleue",
      "pads":[{"index":0,"flavor":"BOOM","note":36,"file":"01_BOOM.wav"},
              {"index":1,"flavor":"SNAP","note":38,"file":"sub/02_SNAP.wav"},
              {"index":2,"file":"03.wav"}]}"#;
    let zip = common::zip_store(&[
        ("kit.json", manifest.as_bytes()),
        ("01_BOOM.wav", &w),
        ("sub/02_SNAP.wav", &w),
        ("03.wav", &w),
        ("zz_ignored.wav", &w),
    ]);
    let kit = Kit::from_zip_bytes("fichier", &zip).unwrap();
    assert_eq!(kit.name, "Casserole bleue");
    assert_eq!(kit.pads.len(), 3);
    assert_eq!(kit.pads[1].label, "SNAP");
    assert_eq!(kit.pads[1].note, 38);
    assert_eq!(kit.pads[2].label, "03", "sans flavor : nom de fichier");
    assert_eq!(kit.pads[2].note, 38, "sans note : 36 + index");
    assert_eq!(
        kit.pad_for_note(38),
        Some(1),
        "le premier pad qui porte la note gagne"
    );
    assert_eq!(kit.pad_for_note(37), None);
}

#[test]
fn manifest_pointing_to_a_missing_file_is_an_error() {
    let manifest = r#"{"schemaVersion":1,"pads":[{"index":0,"file":"nope.wav"}]}"#;
    let zip = common::zip_store(&[("kit.json", manifest.as_bytes())]);
    match Kit::from_zip_bytes("x", &zip) {
        Err(KitError::MissingFile(f)) => assert_eq!(f, "nope.wav"),
        other => panic!("attendu MissingFile, obtenu {other:?}"),
    }
}

#[test]
fn invalid_manifest_is_an_error() {
    let zip = common::zip_store(&[("kit.json", b"{not json")]);
    assert!(matches!(
        Kit::from_zip_bytes("x", &zip),
        Err(KitError::Manifest(_))
    ));
}

#[test]
fn unsupported_schema_version_is_an_error() {
    let zip = common::zip_store(&[("kit.json", br#"{"schemaVersion":2,"pads":[]}"#)]);
    assert!(matches!(
        Kit::from_zip_bytes("x", &zip),
        Err(KitError::Manifest(_))
    ));
}

#[test]
fn empty_zip_and_garbage_are_errors() {
    assert!(matches!(
        Kit::from_zip_bytes("x", &common::zip_store(&[])),
        Err(KitError::Empty)
    ));
    assert!(matches!(
        Kit::from_zip_bytes("x", b"garbage"),
        Err(KitError::Zip(_))
    ));
}

#[test]
fn from_path_loads_a_directory_of_wavs_and_a_zip_file() {
    let dir = std::env::temp_dir().join(format!("mid-kit-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("b.wav"), common::sine_wav(100.0, 10, 44100, 1, 16)).unwrap();
    std::fs::write(dir.join("a.wav"), common::sine_wav(100.0, 10, 44100, 1, 16)).unwrap();
    std::fs::write(dir.join("readme.txt"), b"x").unwrap();
    let kit = Kit::from_path(&dir).unwrap();
    assert_eq!(kit.name, dir.file_name().unwrap().to_string_lossy());
    assert_eq!(kit.pads.len(), 2);
    assert_eq!(kit.pads[0].label, "a");

    let zip_path = dir.join("Mon kit.kit.zip");
    std::fs::write(&zip_path, common::web_pack(44100)).unwrap();
    let kit = Kit::from_path(&zip_path).unwrap();
    assert_eq!(kit.name, "Mon kit", "nom = fichier sans .kit.zip");
    assert_eq!(kit.pads.len(), 20);

    assert!(matches!(
        Kit::from_path(&dir.join("absent.zip")),
        Err(KitError::Io(_))
    ));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn web_pack_pads_get_the_recipe_names_of_the_page() {
    let kit = Kit::from_zip_bytes("makeitdrum_pack", &common::web_pack(44100)).unwrap();
    let labels: Vec<&str> = kit.pads.iter().map(|p| p.label.as_str()).collect();
    assert_eq!(&labels[..4], &["BOOM", "KICK", "THUD", "SNAP"]);
    assert_eq!(labels[19], "CHAOS");
}

#[test]
fn other_wav_names_keep_their_file_stem() {
    let w = common::sine_wav(100.0, 10, 44100, 1, 16);
    let zip = common::zip_store(&[("makeitdrum_01.wav", &w), ("snare.wav", &w)]);
    let kit = Kit::from_zip_bytes("x", &zip).unwrap();
    assert_eq!(kit.pads[0].label, "BOOM", "numéro reconnu");
    assert_eq!(kit.pads[1].label, "snare");
}

#[test]
fn set_pad_on_an_empty_kit_fills_the_gap_with_silent_pads() {
    let mut kit = Kit::empty("kit perso");
    let s = make_it_drum_core::wav::decode_wav(&common::sine_wav(100.0, 10, 44100, 1, 16)).unwrap();
    kit.set_pad(5, "Kick 808", s.clone()).unwrap();
    assert_eq!(kit.pads.len(), 6);
    assert!(kit.pads[..5].iter().all(|p| p.is_empty()));
    assert_eq!(kit.pads[5].label, "Kick 808");
    assert_eq!(kit.pads[5].note, BASE_NOTE + 5);
    assert_eq!(kit.pads[5].index, 5);
    assert_eq!(kit.pad_for_note(BASE_NOTE + 5), Some(5));
    assert_eq!(
        kit.pad_for_note(BASE_NOTE + 2),
        None,
        "un pad vide ne répond pas à sa note"
    );
    assert_eq!(kit.first_free_pad(), Some(0));
    assert!(kit.set_pad(PAD_COUNT, "trop loin", s).is_err());
}

#[test]
fn set_pad_on_an_existing_pad_keeps_its_note_and_takes_the_new_name() {
    let mut kit = Kit::from_zip_bytes("pack", &common::web_pack(44100)).unwrap();
    let s = make_it_drum_core::wav::decode_wav(&common::sine_wav(900.0, 7, 22050, 1, 16)).unwrap();
    kit.pads[3].note = 60;
    kit.set_pad(3, "clap", s).unwrap();
    assert_eq!(kit.pads.len(), PAD_COUNT);
    assert_eq!(kit.pads[3].label, "clap");
    assert_eq!(kit.pads[3].note, 60);
    assert_eq!(kit.pads[3].sample.frames(), 7);
    assert_eq!(kit.pads[3].sample.sample_rate, 22050);
    assert_eq!(kit.first_free_pad(), None, "les 20 pads sont pris");
}

#[test]
fn kit_zip_round_trip_keeps_pads_notes_and_audio_and_skips_empty_pads() {
    let mut kit = Kit::empty("Casserole");
    let a = make_it_drum_core::wav::decode_wav(&common::sine_wav(100.0, 10, 44100, 1, 16)).unwrap();
    let b = make_it_drum_core::wav::decode_wav(&common::sine_wav(200.0, 10, 48000, 2, 32)).unwrap();
    kit.set_pad(1, "Kick 808 (dry)", a).unwrap();
    kit.set_pad(4, "snare", b).unwrap();
    kit.pads[4].note = 70;
    let zip = kit.to_kit_zip().unwrap();
    let back = Kit::from_zip_bytes("ignored", &zip).unwrap();
    assert_eq!(back.name, "Casserole");
    assert_eq!(back.pads.len(), 5, "pads 0..=4, les vides comblés");
    assert!(back.pads[0].is_empty() && back.pads[2].is_empty() && back.pads[3].is_empty());
    assert_eq!(back.pads[1].label, "Kick 808 (dry)");
    assert_eq!(back.pads[1].note, BASE_NOTE + 1);
    assert_eq!(back.pads[4].label, "snare");
    assert_eq!(back.pads[4].note, 70);
    assert_eq!(
        back.pads[4].sample, kit.pads[4].sample,
        "flottant 32 bits : sans perte"
    );
    assert_eq!(back.pads[1].sample, kit.pads[1].sample);
    assert_eq!(back.pad_for_note(70), Some(4));
}

#[test]
fn manifest_with_sparse_indices_puts_pads_at_their_index() {
    let w = common::sine_wav(100.0, 10, 44100, 1, 16);
    let manifest = r#"{"schemaVersion":1,"pads":[{"index":7,"flavor":"BELL","file":"b.wav"},
                                                  {"index":2,"flavor":"SNAP","note":38,"file":"a.wav"}]}"#;
    let zip = common::zip_store(&[
        ("kit.json", manifest.as_bytes()),
        ("a.wav", &w),
        ("b.wav", &w),
    ]);
    let kit = Kit::from_zip_bytes("x", &zip).unwrap();
    assert_eq!(kit.pads.len(), 8);
    assert_eq!(kit.pads[2].label, "SNAP");
    assert_eq!(kit.pads[7].label, "BELL");
    assert_eq!(kit.pads[7].note, BASE_NOTE + 7);
    assert!(kit.pads[0].is_empty());
    assert_eq!(
        kit.pad_for_note(38),
        Some(2),
        "le pad vide 2 ne masque pas… il est le bon"
    );
}

#[test]
fn generate_makes_20_pads_from_one_sound_with_seeds_and_keeps_the_source() {
    let wav = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/source.wav"
    ))
    .unwrap();
    let sound = make_it_drum_core::wav::decode_wav(&wav).unwrap();
    let seeds: Vec<u32> = (0..20).map(|i| 1000 + i).collect();
    let mut seen = Vec::new();
    let kit = Kit::generate("porte", &sound, Some(&seeds), |k, i| {
        seen.push((k.pads.len(), i))
    })
    .unwrap();
    assert_eq!(kit.name, "porte");
    assert_eq!(kit.pads.len(), PAD_COUNT);
    assert_eq!(seen.len(), PAD_COUNT, "un appel de progression par pad");
    assert_eq!(seen[0], (1, 0));
    assert_eq!(seen[19], (20, 19));
    assert!(kit.has_source());
    assert_eq!(kit.source.as_ref().unwrap().sample_rate, 22050);
    let labels: Vec<&str> = kit.pads.iter().map(|p| p.label.as_str()).collect();
    assert_eq!(labels, make_it_drum_core::kit::FLAVORS.to_vec());
    assert_eq!(kit.pads[0].note, BASE_NOTE);
    assert_eq!(kit.pads[19].note, BASE_NOTE + 19);
    assert_eq!(kit.pads[3].seed, Some(1003));
    assert!(kit
        .pads
        .iter()
        .all(|p| !p.is_empty() && p.sample.sample_rate == 22050));

    // même graines → mêmes sons ; aller-retour .kit.zip avec graines et source
    let again = Kit::generate("porte", &sound, Some(&seeds), |_, _| {}).unwrap();
    assert_eq!(again.pads[7].sample, kit.pads[7].sample);
    let zip = kit.to_kit_zip().unwrap();
    let back = Kit::from_zip_bytes("x", &zip).unwrap();
    assert_eq!(back.pads[3].seed, Some(1003));
    assert_eq!(back.source, kit.source, "le son source voyage avec le kit");
    assert_eq!(back.pads[7].sample, kit.pads[7].sample);
    assert_eq!(back.name, "porte");
}

#[test]
fn regenerate_pad_changes_only_that_pad_and_needs_a_source() {
    let wav = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/source.wav"
    ))
    .unwrap();
    let sound = make_it_drum_core::wav::decode_wav(&wav).unwrap();
    let seeds: Vec<u32> = (0..20).collect();
    let mut kit = Kit::generate("k", &sound, Some(&seeds), |_, _| {}).unwrap();
    let before = kit.clone();
    kit.pads[5].note = 61;
    kit.regenerate_pad(5, Some(99)).unwrap();
    assert_eq!(kit.pads[5].seed, Some(99));
    assert_eq!(kit.pads[5].note, 61, "la note du pad est conservée");
    assert_eq!(kit.pads[5].label, "BUZZ");
    assert_ne!(kit.pads[5].sample, before.pads[5].sample);
    assert_eq!(kit.pads[4].sample, before.pads[4].sample);
    assert_eq!(kit.pads[6].sample, before.pads[6].sample);
    assert!(kit.regenerate_pad(PAD_COUNT, None).is_err());

    // un pad remplacé par un son déposé perd sa graine
    kit.set_pad(5, "clap", sound.clone()).unwrap();
    assert_eq!(kit.pads[5].seed, None);

    let mut plain = Kit::from_zip_bytes("pack", &common::web_pack(44100)).unwrap();
    assert!(!plain.has_source());
    assert_eq!(plain.regenerate_pad(0, None), Err(KitError::NoSource));
    assert!(Kit::generate(
        "vide",
        &make_it_drum_core::wav::Sample::silent(),
        None,
        |_, _| {}
    )
    .is_err());
}

#[test]
fn recipe_names_and_pad_names_agree() {
    let names: Vec<&str> = make_it_drum_core::flavors::FLAVORS
        .iter()
        .map(|f| f.name)
        .collect();
    assert_eq!(names, make_it_drum_core::kit::FLAVORS.to_vec());
}

#[test]
fn reshape_rerenders_seeded_pads_only_and_travels_in_the_kit_zip() {
    use make_it_drum_core::render::Shaping;
    let wav = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/source.wav"
    ))
    .unwrap();
    let sound = make_it_drum_core::wav::decode_wav(&wav).unwrap();
    let seeds: Vec<u32> = (0..20).map(|i| 500 + i).collect();
    let mut kit = Kit::generate("k", &sound, Some(&seeds), |_, _| {}).unwrap();
    assert!(kit.shaping.is_default());
    let dropped =
        make_it_drum_core::wav::decode_wav(&common::sine_wav(300.0, 400, 22050, 1, 16)).unwrap();
    kit.set_pad(3, "main", dropped.clone()).unwrap();
    let before = kit.clone();

    let mut seen = Vec::new();
    kit.reshape(Shaping::new(0.8, 0.5), |_, i| seen.push(i))
        .unwrap();
    assert_eq!(seen.len(), 19, "19 pads générés rejoués, pas le pad déposé");
    assert!(!seen.contains(&3));
    assert_eq!(kit.shaping, Shaping::new(0.8, 0.5));
    assert_eq!(kit.pads[3].sample, dropped, "le pad déposé ne bouge pas");
    assert_eq!(kit.pads[3].seed, None);
    for i in (0..20).filter(|i| *i != 3) {
        assert_eq!(kit.pads[i].seed, before.pads[i].seed, "graine conservée");
        assert_ne!(kit.pads[i].sample, before.pads[i].sample, "pad {i} rejoué");
    }

    // même cadrage dès la génération → mêmes sons
    let direct =
        Kit::generate_with("k", &sound, Some(&seeds), Shaping::new(0.8, 0.5), |_, _| {}).unwrap();
    assert_eq!(direct.pads[7].sample, kit.pads[7].sample);

    // le cadrage voyage dans le .kit.zip, et regenerate_pad l'utilise
    let back = Kit::from_zip_bytes("x", &kit.to_kit_zip().unwrap()).unwrap();
    assert_eq!(back.shaping, Shaping::new(0.8, 0.5));
    assert_eq!(back.pads[7].sample, kit.pads[7].sample);
    let mut again = back.clone();
    again.regenerate_pad(7, Some(507)).unwrap();
    assert_eq!(again.pads[7].sample, kit.pads[7].sample);

    // sans source : erreur ; cadrage nul : pas de clé dans le manifeste
    let mut plain = Kit::empty("plain");
    assert_eq!(
        plain.reshape(Shaping::default(), |_, _| {}),
        Err(KitError::NoSource)
    );
    let zip = before.to_kit_zip().unwrap();
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(zip)).unwrap();
    let mut json = String::new();
    std::io::Read::read_to_string(&mut archive.by_name("kit.json").unwrap(), &mut json).unwrap();
    assert!(!json.contains("shaping"));
}
