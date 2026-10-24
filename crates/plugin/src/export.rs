//! Enregistrer et exporter un kit : `.kit.zip` dans le dossier des kits,
//! dossier de WAV, et Drum Rack de Live (`.adg`, XML gzippé) qui pointe
//! vers ces WAV, un Simpler par pad, notes et groupes d'étouffement repris.
use std::io::Write;
use std::path::{Path, PathBuf};

use make_it_drum_core::kit::{Kit, KitError, Pad};
use make_it_drum_core::wav::encode_wav;

const RACK: &str = include_str!("../templates/drum_rack.xml");
const BRANCH: &str = include_str!("../templates/drum_branch.xml");
const PART: &str = include_str!("../templates/sample_part.xml");

fn io(e: std::io::Error) -> KitError {
    KitError::Io(e.to_string())
}

/// Nom de fichier sûr : lettres, chiffres, espaces, tirets ; jamais vide.
pub fn safe_name(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let s = s.trim().trim_end_matches('.').to_string();
    if s.is_empty() {
        "kit".to_string()
    } else {
        s
    }
}

/// Écrit le kit en `<dir>/<name>.kit.zip` et renvoie le chemin.
pub fn save_kit(kit: &Kit, dir: &Path, name: &str) -> Result<PathBuf, KitError> {
    std::fs::create_dir_all(dir).map_err(io)?;
    let path = dir.join(format!("{}.kit.zip", safe_name(name)));
    std::fs::write(&path, kit.to_kit_zip()?).map_err(io)?;
    Ok(path)
}

/// Nom de fichier WAV d'un pad : `03 THUD.wav`.
pub fn pad_file_name(pad: &Pad) -> String {
    format!("{:02} {}.wav", pad.index + 1, safe_name(&pad.label))
}

/// Écrit un WAV par pad sonore dans `<dir>/<name>/` et renvoie le dossier et
/// les fichiers écrits, dans l'ordre des pads.
pub fn export_wavs(kit: &Kit, dir: &Path, name: &str) -> Result<(PathBuf, Vec<PathBuf>), KitError> {
    let folder = dir.join(safe_name(name));
    std::fs::create_dir_all(&folder).map_err(io)?;
    let mut files = Vec::new();
    for pad in kit.pads.iter().filter(|p| !p.is_empty()) {
        let path = folder.join(pad_file_name(pad));
        std::fs::write(&path, encode_wav(&pad.sample)?).map_err(io)?;
        files.push(path);
    }
    if files.is_empty() {
        return Err(KitError::Empty);
    }
    Ok((folder, files))
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Le XML (non compressé) d'un Drum Rack de Live pour `kit`, dont les WAV
/// sont `files` (un par pad sonore, dans l'ordre des pads sonores).
pub fn drum_rack_xml(kit: &Kit, files: &[PathBuf]) -> String {
    let mut branches = String::new();
    for (k, (pad, file)) in kit
        .pads
        .iter()
        .filter(|p| !p.is_empty())
        .zip(files)
        .enumerate()
    {
        let frames = pad.sample.frames();
        let path = file.to_string_lossy();
        let file_name = file
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let size = std::fs::metadata(file).map(|m| m.len()).unwrap_or(0);
        let part = PART
            .replace("{{PART_ID}}", &k.to_string())
            .replace("{{FILE_NAME}}", &xml_escape(&file_name))
            .replace("{{PATH}}", &xml_escape(&path))
            .replace("{{LAST_FRAME}}", &frames.saturating_sub(1).to_string())
            .replace("{{FRAMES}}", &frames.to_string())
            .replace("{{SAMPLE_RATE}}", &pad.sample.sample_rate.to_string())
            .replace("{{FILE_SIZE}}", &size.to_string())
            .replace("{{MOD_DATE}}", "0");
        let parts = format!("<SampleParts>\n{part}</SampleParts>");
        let branch = BRANCH
            .replace("{{BRANCH_ID}}", &k.to_string())
            .replace("{{PAD_NAME}}", &xml_escape(&pad.label))
            .replace("{{SAMPLE_PARTS}}", &parts)
            // Live range les pads par 128 − note MIDI
            .replace("{{RECEIVING_NOTE}}", &(128 - pad.note as i32).to_string())
            .replace("{{CHOKE_GROUP}}", &pad.settings.choke.to_string());
        branches.push_str(&branch);
    }
    RACK.replace("{{KIT_NAME}}", &xml_escape(&kit.name))
        .replace("{{BRANCHES}}", branches.trim_end_matches('\n'))
}

/// Exporte les WAV puis écrit `<dir>/<name>/<name>.adg`, un Drum Rack de
/// Live qui les joue. Renvoie le chemin du `.adg`.
pub fn export_drum_rack(kit: &Kit, dir: &Path, name: &str) -> Result<PathBuf, KitError> {
    let (folder, files) = export_wavs(kit, dir, name)?;
    let xml = drum_rack_xml(kit, &files);
    let path = folder.join(format!("{}.adg", safe_name(name)));
    let file = std::fs::File::create(&path).map_err(io)?;
    let mut gz = flate2::write::GzEncoder::new(file, flate2::Compression::default());
    gz.write_all(xml.as_bytes()).map_err(io)?;
    gz.finish().map_err(io)?;
    Ok(path)
}
