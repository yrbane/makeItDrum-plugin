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
