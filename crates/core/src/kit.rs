//! Un kit : jusqu'à 20 pads, chacun un échantillon et une note MIDI.
//!
//! Trois sources : le `makeitdrum_pack.zip` exporté par la page web (20 WAV,
//! sans manifeste), un `.kit.zip` portant un `kit.json` (schéma v1), ou un
//! dossier de WAV. Sans manifeste, les WAV sont triés par nom et le pad `i`
//! reçoit la note General MIDI `36 + i`.
use std::fmt;
use std::io::{Cursor, Read};
use std::path::Path;

use serde::Deserialize;

use crate::wav::{decode_wav, Sample};

pub const PAD_COUNT: usize = 20;
/// Note GM du premier pad (C1, kick).
pub const BASE_NOTE: u8 = 36;

#[derive(Debug, Clone, PartialEq)]
pub enum KitError {
    Zip(String),
    Wav(String),
    Manifest(String),
    MissingFile(String),
    Io(String),
    Empty,
}

impl fmt::Display for KitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KitError::Zip(m) => write!(f, "archive illisible : {m}"),
            KitError::Wav(m) => write!(f, "WAV illisible : {m}"),
            KitError::Manifest(m) => write!(f, "kit.json invalide : {m}"),
            KitError::MissingFile(m) => write!(f, "fichier absent de l'archive : {m}"),
            KitError::Io(m) => write!(f, "lecture impossible : {m}"),
            KitError::Empty => write!(f, "aucun WAV dans le kit"),
        }
    }
}

impl std::error::Error for KitError {}

#[derive(Debug, Clone, PartialEq)]
pub struct Pad {
    pub index: usize,
    pub label: String,
    pub note: u8,
    pub sample: Sample,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Kit {
    pub name: String,
    pub pads: Vec<Pad>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    schema_version: u32,
    name: Option<String>,
    pads: Vec<ManifestPad>,
}

#[derive(Deserialize)]
struct ManifestPad {
    index: usize,
    flavor: Option<String>,
    note: Option<u8>,
    file: String,
}

fn stem(name: &str) -> String {
    let base = name.rsplit('/').next().unwrap_or(name);
    let base = base.strip_suffix(".kit.zip").unwrap_or(base);
    match base.rfind('.') {
        Some(i) if i > 0 => base[..i].to_string(),
        _ => base.to_string(),
    }
}

fn is_wav(name: &str) -> bool {
    name.to_ascii_lowercase().ends_with(".wav") && !name.ends_with('/')
}

impl Kit {
    /// Lit un kit depuis les octets d'un ZIP ; `name` sert si le manifeste n'en donne pas.
    pub fn from_zip_bytes(name: &str, bytes: &[u8]) -> Result<Kit, KitError> {
        let mut archive =
            zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| KitError::Zip(e.to_string()))?;
        let read_entry = |archive: &mut zip::ZipArchive<Cursor<&[u8]>>,
                          entry: &str|
         -> Result<Vec<u8>, KitError> {
            let mut file = archive
                .by_name(entry)
                .map_err(|_| KitError::MissingFile(entry.to_string()))?;
            let mut buf = Vec::with_capacity(file.size() as usize);
            file.read_to_end(&mut buf)
                .map_err(|e| KitError::Zip(e.to_string()))?;
            Ok(buf)
        };
        let names: Vec<String> = archive
            .file_names()
            .filter_map(|n| n.ok().map(|c| c.into_owned()))
            .collect();
        if let Some(manifest_name) = names
            .iter()
            .find(|n| n.as_str() == "kit.json" || n.ends_with("/kit.json"))
        {
            let manifest_name = manifest_name.clone();
            let raw = read_entry(&mut archive, &manifest_name)?;
            let manifest: Manifest =
                serde_json::from_slice(&raw).map_err(|e| KitError::Manifest(e.to_string()))?;
            if manifest.schema_version != 1 {
                return Err(KitError::Manifest(format!(
                    "schemaVersion {} non pris en charge",
                    manifest.schema_version
                )));
            }
            let prefix = manifest_name.strip_suffix("kit.json").unwrap_or("");
            let mut pads = Vec::with_capacity(manifest.pads.len());
            for p in manifest.pads.into_iter().take(PAD_COUNT) {
                let entry = format!("{prefix}{}", p.file);
                let wav = read_entry(&mut archive, &entry).map_err(|e| match e {
                    KitError::MissingFile(_) => KitError::MissingFile(p.file.clone()),
                    other => other,
                })?;
                pads.push(Pad {
                    index: p.index,
                    label: p.flavor.unwrap_or_else(|| stem(&p.file)),
                    note: p.note.unwrap_or(BASE_NOTE.saturating_add(p.index as u8)),
                    sample: decode_wav(&wav)?,
                });
            }
            return Ok(Kit {
                name: manifest.name.unwrap_or_else(|| name.to_string()),
                pads,
            });
        }
        let mut wavs: Vec<String> = names.into_iter().filter(|n| is_wav(n)).collect();
        wavs.sort();
        let mut pads = Vec::new();
        for (index, entry) in wavs.iter().take(PAD_COUNT).enumerate() {
            let wav = read_entry(&mut archive, entry)?;
            pads.push(Pad {
                index,
                label: stem(entry),
                note: BASE_NOTE + index as u8,
                sample: decode_wav(&wav)?,
            });
        }
        if pads.is_empty() {
            return Err(KitError::Empty);
        }
        Ok(Kit {
            name: name.to_string(),
            pads,
        })
    }

    /// Lit les WAV d'un dossier (non récursif, triés par nom).
    pub fn from_dir(path: &Path) -> Result<Kit, KitError> {
        let mut files: Vec<_> = std::fs::read_dir(path)
            .map_err(|e| KitError::Io(e.to_string()))?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.is_file()
                    && p.file_name()
                        .map(|n| is_wav(&n.to_string_lossy()))
                        .unwrap_or(false)
            })
            .collect();
        files.sort();
        let mut pads = Vec::new();
        for (index, file) in files.iter().take(PAD_COUNT).enumerate() {
            let bytes = std::fs::read(file).map_err(|e| KitError::Io(e.to_string()))?;
            let label = stem(&file.file_name().unwrap_or_default().to_string_lossy());
            pads.push(Pad {
                index,
                label,
                note: BASE_NOTE + index as u8,
                sample: decode_wav(&bytes)?,
            });
        }
        if pads.is_empty() {
            return Err(KitError::Empty);
        }
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "kit".into());
        Ok(Kit { name, pads })
    }

    /// Lit un kit depuis un `.zip` ou un dossier.
    pub fn from_path(path: &Path) -> Result<Kit, KitError> {
        if path.is_dir() {
            return Kit::from_dir(path);
        }
        let bytes = std::fs::read(path).map_err(|e| KitError::Io(e.to_string()))?;
        let name = stem(&path.file_name().unwrap_or_default().to_string_lossy());
        Kit::from_zip_bytes(&name, &bytes)
    }

    /// Index du premier pad qui porte cette note.
    pub fn pad_for_note(&self, note: u8) -> Option<usize> {
        self.pads.iter().position(|p| p.note == note)
    }
}
