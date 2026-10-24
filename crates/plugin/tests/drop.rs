// Ce que devient un fichier déposé sur la fenêtre : kit, son sur un pad,
// génération d'un kit, ou refus.
use make_it_drum::editor::{classify_drop, Drop};
use std::path::Path;

#[test]
fn zip_and_directories_load_a_kit_wherever_they_land() {
    assert_eq!(
        classify_drop(Path::new("/x/makeitdrum_pack.zip"), Some(4)),
        Drop::Kit("/x/makeitdrum_pack.zip".into())
    );
    assert_eq!(
        classify_drop(Path::new("/x/Mon.KIT.ZIP"), None),
        Drop::Kit("/x/Mon.KIT.ZIP".into())
    );
    let dir = std::env::temp_dir();
    assert_eq!(classify_drop(&dir, Some(0)), Drop::Kit(dir.clone()));
}

#[test]
fn audio_on_a_pad_replaces_it_and_elsewhere_generates_a_kit() {
    let p = Path::new("/lib/Kick 808.aif");
    assert_eq!(
        classify_drop(p, Some(7)),
        Drop::Sample {
            path: p.into(),
            pad: 7
        }
    );
    assert_eq!(classify_drop(p, None), Drop::Generate(p.into()));
}

#[test]
fn anything_else_is_unsupported() {
    let p = Path::new("/x/notes.txt");
    assert_eq!(classify_drop(p, Some(1)), Drop::Unsupported(p.into()));
}
