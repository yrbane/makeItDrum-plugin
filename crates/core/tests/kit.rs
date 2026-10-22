mod common;
use make_it_drum_core::kit::{Kit, KitError, BASE_NOTE, PAD_COUNT};

#[test]
fn web_pack_without_manifest_maps_20_pads_to_gm_notes() {
    let kit = Kit::from_zip_bytes("makeitdrum_pack", &common::web_pack(44100)).unwrap();
    assert_eq!(kit.name, "makeitdrum_pack");
    assert_eq!(kit.pads.len(), PAD_COUNT);
    assert_eq!(kit.pads[0].label, "makeitdrum_01");
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
