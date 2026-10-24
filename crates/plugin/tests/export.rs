use make_it_drum::export::{
    drum_rack_xml, export_drum_rack, export_wavs, pad_file_name, safe_name, save_kit,
};
use make_it_drum_core::kit::{Kit, PadSettings};
use make_it_drum_core::wav::{decode_wav, Sample};
use std::io::Read;

fn tmp() -> std::path::PathBuf {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("mid-export-{n}"))
}

fn kit() -> Kit {
    let mut k = Kit::empty("Porte / claque");
    for i in [0usize, 2, 5] {
        let sample = Sample {
            data: (0..100).map(|j| (j as f32 / 100.0).sin()).collect(),
            channels: 1,
            sample_rate: 44100,
        };
        k.set_pad(i, &format!("pad {i}"), sample).unwrap();
    }
    k.set_settings(
        2,
        PadSettings {
            choke: 3,
            ..Default::default()
        },
    )
    .unwrap();
    k
}

#[test]
fn names_are_made_safe() {
    assert_eq!(safe_name("Porte / claque"), "Porte _ claque");
    assert_eq!(safe_name("  ..  "), "kit");
    assert_eq!(safe_name("ok-1_2.x"), "ok-1_2.x");
    assert_eq!(pad_file_name(&kit().pads[2]), "03 pad 2.wav");
}

#[test]
fn save_kit_writes_a_loadable_kit_zip() {
    let dir = tmp();
    let path = save_kit(&kit(), &dir, "Porte / claque").unwrap();
    assert_eq!(path.file_name().unwrap(), "Porte _ claque.kit.zip");
    let back = Kit::from_path(&path).unwrap();
    assert_eq!(back.pads[2].settings.choke, 3);
    assert_eq!(back.pads[5].label, "pad 5");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn export_wavs_writes_one_file_per_sounding_pad() {
    let dir = tmp();
    let (folder, files) = export_wavs(&kit(), &dir, "x").unwrap();
    assert_eq!(folder, dir.join("x"));
    let names: Vec<String> = files
        .iter()
        .map(|f| f.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["01 pad 0.wav", "03 pad 2.wav", "06 pad 5.wav"]);
    let s = decode_wav(&std::fs::read(&files[1]).unwrap()).unwrap();
    assert_eq!(s.frames(), 100);
    assert!(export_wavs(&Kit::empty("vide"), &dir, "v").is_err());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn drum_rack_has_one_branch_per_pad_with_live_note_numbers_and_choke_groups() {
    let dir = tmp();
    let (_, files) = export_wavs(&kit(), &dir, "x").unwrap();
    let xml = drum_rack_xml(&kit(), &files);
    assert!(xml.starts_with("<?xml"));
    assert_eq!(xml.matches("<DrumBranchPreset Id=").count(), 3);
    assert_eq!(xml.matches("<MultiSamplePart Id=").count(), 3);
    assert!(xml.contains("<UserName Value=\"Porte / claque\" />"));
    // pad 1 = note 36 → 92, pad 3 = note 38 → 90, pad 6 = 41 → 87
    for n in [92, 90, 87] {
        assert!(
            xml.contains(&format!("<ReceivingNote Value=\"{n}\" />")),
            "{n}"
        );
    }
    assert_eq!(xml.matches("<ChokeGroup Value=\"3\" />").count(), 1);
    assert_eq!(xml.matches("<ChokeGroup Value=\"0\" />").count(), 2);
    assert!(xml.contains("<DefaultDuration Value=\"100\" />"));
    assert!(xml.contains("<SampleEnd Value=\"99\" />"));
    assert!(xml.contains("<DefaultSampleRate Value=\"44100\" />"));
    assert!(xml.contains(&format!("<Path Value=\"{}\" />", files[0].display())));
    assert!(!xml.contains("{{"), "tous les trous remplis");
    // bien formé
    let mut depth = 0i32;
    for tag in xml.split('<').skip(1) {
        if tag.starts_with('?') || tag.ends_with("/>") || tag.trim_end().ends_with("/>") {
            continue;
        }
        if tag.starts_with('/') {
            depth -= 1;
        } else {
            depth += 1;
        }
        assert!(depth >= 0);
    }
    assert_eq!(depth, 0, "balises équilibrées");

    let adg = export_drum_rack(&kit(), &dir, "x").unwrap();
    assert_eq!(adg, dir.join("x").join("x.adg"));
    let mut gz = flate2::read::GzDecoder::new(std::fs::File::open(&adg).unwrap());
    let mut back = String::new();
    gz.read_to_string(&mut back).unwrap();
    assert_eq!(back, drum_rack_xml(&kit(), &files));
    std::fs::remove_dir_all(&dir).ok();
}
