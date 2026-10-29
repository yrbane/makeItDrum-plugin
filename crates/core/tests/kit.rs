mod common;
use make_it_drum_core::kit::{Kit, KitError, Pad, BASE_NOTE, PAD_COUNT};

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
        assert_eq!(k.pads.len(), PAD_COUNT, "la grille est pleine dès le début");
        assert!(!k.pads[i].is_empty(), "le pad livré est rendu");
        seen.push(i)
    })
    .unwrap();
    assert_eq!(kit.name, "porte");
    assert_eq!(kit.pads.len(), PAD_COUNT);
    assert_eq!(seen.len(), PAD_COUNT, "un appel de progression par pad");
    seen.sort();
    assert_eq!(
        seen,
        (0..PAD_COUNT).collect::<Vec<_>>(),
        "chaque pad une fois"
    );
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

// ---- réglages par pad, variantes de frappe, verrou, renommage

#[test]
fn pad_settings_are_clamped_and_travel_in_the_kit_zip_only_when_set() {
    use make_it_drum_core::kit::PadSettings;
    let mut kit = Kit::from_zip_bytes("pack", &common::web_pack(44100)).unwrap();
    assert!(kit.pads.iter().all(|p| p.settings.is_default()));
    kit.set_settings(
        2,
        PadSettings {
            gain_db: -40.0,
            pitch: 30.0,
            pan: 2.0,
            decay: 0.0,
            reverse: true,
            choke: 9,
            locked: true,
            cut_self: false,
            cut_by_all: true,
        },
    )
    .unwrap();
    let s = kit.pads[2].settings;
    assert_eq!(s.gain_db, -24.0);
    assert_eq!(s.pitch, 24.0);
    assert_eq!(s.pan, 1.0);
    assert_eq!(s.decay, 0.05);
    assert_eq!(s.choke, PadSettings::MAX_CHOKE);
    assert!(s.reverse && s.locked);
    assert!(kit.set_settings(PAD_COUNT, PadSettings::default()).is_err());
    kit.rename_pad(2, "  caisse  ").unwrap();
    assert_eq!(kit.pads[2].label, "caisse");

    let zip = kit.to_kit_zip().unwrap();
    let back = Kit::from_zip_bytes("x", &zip).unwrap();
    assert_eq!(back.pads[2].settings, s);
    assert_eq!(back.pads[2].label, "caisse");
    assert!(back.pads[3].settings.is_default());
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(zip)).unwrap();
    let mut json = String::new();
    std::io::Read::read_to_string(&mut archive.by_name("kit.json").unwrap(), &mut json).unwrap();
    assert_eq!(
        json.matches("\"settings\"").count(),
        1,
        "seul le pad réglé porte des réglages"
    );
    assert!(json.contains("\"gainDb\": -24.0"));
}

#[test]
fn round_robin_renders_alternates_for_seeded_unlocked_pads_and_persists_them() {
    use make_it_drum_core::kit::{PadSettings, MAX_ROUND_ROBIN};
    let wav = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/source.wav"
    ))
    .unwrap();
    let sound = make_it_drum_core::wav::decode_wav(&wav).unwrap();
    let seeds: Vec<u32> = (0..20).map(|i| 900 + i).collect();
    let mut kit = Kit::generate("k", &sound, Some(&seeds), |_, _| {}).unwrap();
    assert_eq!(kit.round_robin(), 1);
    let dropped =
        make_it_drum_core::wav::decode_wav(&common::sine_wav(300.0, 400, 22050, 1, 16)).unwrap();
    kit.set_pad(4, "main", dropped).unwrap();
    kit.set_settings(
        6,
        PadSettings {
            locked: true,
            ..Default::default()
        },
    )
    .unwrap();

    let mut seen = Vec::new();
    kit.set_round_robin(3, |_, i| seen.push(i)).unwrap();
    assert_eq!(seen.len(), 18, "20 pads moins le déposé et le verrouillé");
    assert_eq!(kit.round_robin(), 3);
    assert_eq!(kit.pads[0].alternates.len(), 2);
    assert_eq!(kit.pads[4].alternates.len(), 0);
    assert_eq!(kit.pads[6].alternates.len(), 0);
    assert_eq!(kit.pads[0].alternates[0].0, Pad::alternate_seed(900, 1));
    assert_ne!(kit.pads[0].alternates[0].1, kit.pads[0].sample);
    assert_ne!(kit.pads[0].alternates[0].1, kit.pads[0].alternates[1].1);
    assert_eq!(kit.pads[0].sample_at(1), &kit.pads[0].alternates[0].1);
    assert_eq!(kit.pads[0].sample_at(3), &kit.pads[0].sample, "modulo");

    // déjà au bon compte : rien à refaire ; réduire retire ; au-delà du max, borné
    let mut seen = Vec::new();
    kit.set_round_robin(3, |_, i| seen.push(i)).unwrap();
    assert!(seen.is_empty());
    let back = Kit::from_zip_bytes("x", &kit.to_kit_zip().unwrap()).unwrap();
    assert_eq!(back.pads[0].alternates, kit.pads[0].alternates);
    assert_eq!(back.round_robin(), 3);
    kit.set_round_robin(99, |_, _| {}).unwrap();
    assert_eq!(kit.round_robin(), MAX_ROUND_ROBIN);
    kit.set_round_robin(1, |_, _| {}).unwrap();
    assert_eq!(kit.round_robin(), 1);

    // sans source : impossible d'en demander, mais 1 passe
    let mut plain = Kit::from_zip_bytes("pack", &common::web_pack(44100)).unwrap();
    assert_eq!(plain.set_round_robin(2, |_, _| {}), Err(KitError::NoSource));
    assert!(plain.set_round_robin(1, |_, _| {}).is_ok());
}

#[test]
fn locked_pads_survive_reshape_and_variations_and_alternates_follow_their_pad() {
    use make_it_drum_core::kit::PadSettings;
    use make_it_drum_core::render::Shaping;
    let wav = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/source.wav"
    ))
    .unwrap();
    let sound = make_it_drum_core::wav::decode_wav(&wav).unwrap();
    let seeds: Vec<u32> = (0..20).map(|i| 300 + i).collect();
    let mut kit = Kit::generate("k", &sound, Some(&seeds), |_, _| {}).unwrap();
    kit.set_round_robin(2, |_, _| {}).unwrap();
    kit.set_settings(
        1,
        PadSettings {
            locked: true,
            ..Default::default()
        },
    )
    .unwrap();
    let before = kit.clone();
    kit.reshape(Shaping::new(1.0, 0.0), |_, _| {}).unwrap();
    assert_eq!(kit.pads[1], before.pads[1], "verrouillé : intact");
    assert_ne!(kit.pads[0].sample, before.pads[0].sample);
    assert_ne!(
        kit.pads[0].alternates[0].1, before.pads[0].alternates[0].1,
        "variante rejouée"
    );
    assert_eq!(
        kit.pads[0].alternates[0].0, before.pads[0].alternates[0].0,
        "même graine"
    );

    // nouvelle graine explicite : les variantes suivent la nouvelle graine
    kit.regenerate_pad(0, Some(4242)).unwrap();
    assert_eq!(kit.pads[0].alternates[0].0, Pad::alternate_seed(4242, 1));
    // un son déposé sur un pad efface ses variantes
    let dropped =
        make_it_drum_core::wav::decode_wav(&common::sine_wav(300.0, 400, 22050, 1, 16)).unwrap();
    kit.set_pad(0, "main", dropped).unwrap();
    assert!(kit.pads[0].alternates.is_empty());
}

#[test]
fn start_moves_the_source_window_keeps_the_whole_source_and_travels_in_the_zip() {
    use make_it_drum_core::render::Shaping;
    let wav = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/source.wav"
    ))
    .unwrap();
    let sound = make_it_drum_core::wav::decode_wav(&wav).unwrap();
    let seeds: Vec<u32> = (0..20).map(|i| 100 + i).collect();
    let plain = Kit::generate("k", &sound, Some(&seeds), |_, _| {}).unwrap();
    let late = Kit::generate_with(
        "k",
        &sound,
        Some(&seeds),
        Shaping::default().with_start(0.5),
        |_, _| {},
    )
    .unwrap();
    assert_eq!(late.source, plain.source, "le son entier reste dans le kit");
    assert_ne!(late.pads[0].sample, plain.pads[0].sample);
    let onset = plain.source_onset().unwrap();
    assert!((0.0..1.0).contains(&onset));
    assert_eq!(Kit::empty("x").source_onset(), None);

    let back = Kit::from_zip_bytes("x", &late.to_kit_zip().unwrap()).unwrap();
    assert_eq!(back.shaping.start, 0.5);
    let mut moved = plain.clone();
    moved
        .reshape(Shaping::default().with_start(0.5), |_, _| {})
        .unwrap();
    assert_eq!(moved.pads[0].sample, late.pads[0].sample);
}

#[test]
fn cut_itself_and_cut_by_all_travel_in_the_kit_json_in_camel_case() {
    use make_it_drum_core::kit::PadSettings;
    let mut kit = Kit::from_zip_bytes("pack", &common::web_pack(22050)).unwrap();
    assert!(kit.pads[0].settings.cut_self, "cut itself est le défaut");
    assert!(!kit.pads[0].settings.cut_by_all);
    kit.set_settings(
        6,
        PadSettings {
            cut_self: false,
            cut_by_all: true,
            ..Default::default()
        },
    )
    .unwrap();
    let zip = kit.to_kit_zip().unwrap();
    let back = Kit::from_zip_bytes("pack", &zip).unwrap();
    assert!(!back.pads[6].settings.cut_self);
    assert!(back.pads[6].settings.cut_by_all);
    assert!(back.pads[7].settings.cut_self);
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(zip)).unwrap();
    let mut json = String::new();
    std::io::Read::read_to_string(&mut archive.by_name("kit.json").unwrap(), &mut json).unwrap();
    assert!(json.contains("\"cutSelf\": false"), "{json}");
    assert!(json.contains("\"cutByAll\": true"), "{json}");
}
