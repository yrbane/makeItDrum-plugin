//! Un kit : jusqu'à 20 pads, chacun un échantillon et une note MIDI.
//!
//! Trois sources : le `makeitdrum_pack.zip` exporté par la page web (20 WAV,
//! sans manifeste), un `.kit.zip` portant un `kit.json` (schéma v1), ou un
//! dossier de WAV. Sans manifeste, les WAV sont triés par nom et le pad `i`
//! reçoit la note General MIDI `36 + i`.
use std::fmt;
use std::io::{Cursor, Read, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::flavors::{flavor_at, FLAVORS as RECIPES};
use crate::prng::fresh_seed;
use crate::render::{render_pad_with, Shaping, SourceSample};
use crate::wav::{decode_wav, Sample};

pub const PAD_COUNT: usize = 20;
/// Note GM du premier pad (C1, kick).
pub const BASE_NOTE: u8 = 36;

/// Les 20 recettes de la page make it drum, dans l'ordre d'export du pack.
pub const FLAVORS: [&str; PAD_COUNT] = [
    "BOOM", "KICK", "THUD", "SNAP", "CRACK", "BUZZ", "RING", "TICK", "GLASS", "WASH", "BEND",
    "SWELL", "STUTTER", "CRUSH", "PHASE", "GROWL", "PERC", "BELL", "DARK", "CHAOS",
];

/// `makeitdrum_NN` (NN de 01 à 20) → nom de la recette ; sinon le nom tel quel.
fn label_for(stem: &str) -> String {
    stem.strip_prefix("makeitdrum_")
        .and_then(|n| n.parse::<usize>().ok())
        .filter(|n| (1..=PAD_COUNT).contains(n))
        .map(|n| FLAVORS[n - 1].to_string())
        .unwrap_or_else(|| stem.to_string())
}

#[derive(Debug, Clone, PartialEq)]
pub enum KitError {
    Zip(String),
    Wav(String),
    Manifest(String),
    MissingFile(String),
    Io(String),
    Empty,
    /// Le kit n'a pas de son source : impossible de régénérer.
    NoSource,
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
            KitError::NoSource => write!(f, "ce kit n'a pas de son source à régénérer"),
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
    /// Graine du rendu, si le pad a été généré à partir du son source.
    pub seed: Option<u32>,
}

impl Pad {
    /// Un pad sans son, qui tient la place `index` dans la grille.
    pub fn empty(index: usize) -> Pad {
        Pad {
            index,
            label: String::new(),
            note: BASE_NOTE.saturating_add(index as u8),
            sample: Sample::silent(),
            seed: None,
        }
    }

    /// `true` si le pad n'a aucun son.
    pub fn is_empty(&self) -> bool {
        self.sample.frames() == 0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Kit {
    pub name: String,
    pub pads: Vec<Pad>,
    /// Le son importé dont les pads sont tirés, mono ; permet de régénérer.
    pub source: Option<Sample>,
    /// Le cadrage (fidélité, durée) avec lequel les pads générés ont été rendus.
    pub shaping: Shaping,
}

/// Nom d'entrée du son source dans un `.kit.zip` écrit par le plugin.
pub const SOURCE_FILE: &str = "source.wav";

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    schema_version: u32,
    #[serde(skip_deserializing)]
    app: &'static str,
    name: Option<String>,
    source: Option<ManifestSource>,
    /// Cadrage de la génération (plugin) ; absent dans les packs de la page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    shaping: Option<ManifestShaping>,
    pads: Vec<ManifestPad>,
}

#[derive(Serialize, Deserialize, Default)]
struct ManifestShaping {
    fidelity: f64,
    length: f64,
}

/// Le son source : les métadonnées de la page, plus `file` quand le plugin
/// l'embarque dans l'archive.
#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct ManifestSource {
    file_name: Option<String>,
    sample_rate: Option<u32>,
    duration: Option<f64>,
    file: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct ManifestPad {
    index: usize,
    flavor: Option<String>,
    note: Option<u8>,
    seed: Option<u32>,
    file: String,
}

/// Les pads à leur place : triés par `index`, les trous comblés par des pads
/// vides, au plus `PAD_COUNT`. Le moteur et l'interface lisent la position.
fn normalize(mut pads: Vec<Pad>) -> Vec<Pad> {
    pads.sort_by_key(|p| p.index);
    let mut out: Vec<Pad> = Vec::with_capacity(PAD_COUNT);
    for pad in pads {
        if pad.index >= PAD_COUNT {
            break;
        }
        while out.len() < pad.index {
            out.push(Pad::empty(out.len()));
        }
        if out.len() == pad.index {
            out.push(pad);
        }
    }
    out
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
                    seed: p.seed,
                });
            }
            let source = match manifest.source.and_then(|s| s.file) {
                Some(file) => Some(decode_wav(&read_entry(
                    &mut archive,
                    &format!("{prefix}{file}"),
                )?)?),
                None => None,
            };
            let shaping = manifest
                .shaping
                .map(|s| Shaping::new(s.fidelity, s.length))
                .unwrap_or_default();
            return Ok(Kit {
                name: manifest.name.unwrap_or_else(|| name.to_string()),
                pads: normalize(pads),
                source,
                shaping,
            });
        }
        let mut wavs: Vec<String> = names.into_iter().filter(|n| is_wav(n)).collect();
        wavs.sort();
        let mut pads = Vec::new();
        for (index, entry) in wavs.iter().take(PAD_COUNT).enumerate() {
            let wav = read_entry(&mut archive, entry)?;
            pads.push(Pad {
                index,
                label: label_for(&stem(entry)),
                note: BASE_NOTE + index as u8,
                sample: decode_wav(&wav)?,
                seed: None,
            });
        }
        if pads.is_empty() {
            return Err(KitError::Empty);
        }
        Ok(Kit {
            name: name.to_string(),
            pads,
            source: None,
            shaping: Shaping::default(),
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
            let label = label_for(&stem(
                &file.file_name().unwrap_or_default().to_string_lossy(),
            ));
            pads.push(Pad {
                index,
                label,
                note: BASE_NOTE + index as u8,
                sample: decode_wav(&bytes)?,
                seed: None,
            });
        }
        if pads.is_empty() {
            return Err(KitError::Empty);
        }
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "kit".into());
        Ok(Kit {
            name,
            pads,
            source: None,
            shaping: Shaping::default(),
        })
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

    /// Index du premier pad sonore qui porte cette note.
    pub fn pad_for_note(&self, note: u8) -> Option<usize> {
        self.pads
            .iter()
            .position(|p| p.note == note && !p.is_empty())
    }

    /// Un kit sans aucun pad.
    pub fn empty(name: &str) -> Kit {
        Kit {
            name: name.to_string(),
            pads: Vec::new(),
            source: None,
            shaping: Shaping::default(),
        }
    }

    /// Fabrique les 20 percussions à partir d'un son, une recette par pad :
    /// ce que fait la page. `seeds` fixe les graines (sinon tirées), et
    /// `progress` est appelé après chaque pad rendu, avec le kit en cours.
    pub fn generate(
        name: &str,
        source: &Sample,
        seeds: Option<&[u32]>,
        progress: impl FnMut(&Kit, usize),
    ) -> Result<Kit, KitError> {
        Self::generate_with(name, source, seeds, Shaping::default(), progress)
    }

    /// Comme `generate`, cadré par `shaping` (fidélité au son, durée).
    pub fn generate_with(
        name: &str,
        source: &Sample,
        seeds: Option<&[u32]>,
        shaping: Shaping,
        mut progress: impl FnMut(&Kit, usize),
    ) -> Result<Kit, KitError> {
        if source.frames() == 0 {
            return Err(KitError::Empty);
        }
        let src = SourceSample::prepare(source);
        let mut kit = Kit {
            name: name.to_string(),
            pads: Vec::with_capacity(PAD_COUNT),
            source: Some(src.to_sample()),
            shaping,
        };
        for (index, flavor) in RECIPES.iter().enumerate() {
            let seed = seeds
                .and_then(|s| s.get(index).copied())
                .unwrap_or_else(fresh_seed);
            kit.pads.push(Pad {
                index,
                label: flavor.name.to_string(),
                note: BASE_NOTE + index as u8,
                sample: render_pad_with(flavor, &src, seed, &shaping),
                seed: Some(seed),
            });
            progress(&kit, index);
        }
        Ok(kit)
    }

    /// Rejoue la recette du pad `index` avec une nouvelle graine (ou `seed`),
    /// à partir du son source du kit. Le nom, la note du pad sont conservés.
    pub fn regenerate_pad(&mut self, index: usize, seed: Option<u32>) -> Result<(), KitError> {
        let source = self.source.as_ref().ok_or(KitError::NoSource)?;
        if index >= PAD_COUNT {
            return Err(KitError::Manifest(format!("pad {index} hors de la grille")));
        }
        let src = SourceSample::prepare(source);
        let seed = seed.unwrap_or_else(fresh_seed);
        let flavor = flavor_at(index);
        let sample = render_pad_with(flavor, &src, seed, &self.shaping);
        while self.pads.len() <= index {
            self.pads.push(Pad::empty(self.pads.len()));
        }
        let pad = &mut self.pads[index];
        if pad.is_empty() {
            pad.label = flavor.name.to_string();
        }
        pad.sample = sample;
        pad.seed = Some(seed);
        Ok(())
    }

    /// Change le cadrage et rejoue, avec leur graine, tous les pads générés :
    /// même « idée » de son, plus ou moins proche de l'original. Les pads
    /// déposés à la main (sans graine) ne bougent pas. `progress` est appelé
    /// après chaque pad rendu.
    pub fn reshape(
        &mut self,
        shaping: Shaping,
        mut progress: impl FnMut(&Kit, usize),
    ) -> Result<(), KitError> {
        let source = self.source.as_ref().ok_or(KitError::NoSource)?;
        let src = SourceSample::prepare(source);
        self.shaping = shaping;
        for index in 0..self.pads.len() {
            let Some(seed) = self.pads[index].seed else {
                continue;
            };
            self.pads[index].sample = render_pad_with(flavor_at(index), &src, seed, &shaping);
            progress(self, index);
        }
        Ok(())
    }

    /// `true` si le kit peut être régénéré (il garde son son source).
    pub fn has_source(&self) -> bool {
        self.source.is_some()
    }

    /// Met `sample` sur le pad `index` (0 à 19). Un pad existant garde sa
    /// note ; un pad nouveau prend la note `36 + index`, et les places
    /// intermédiaires sont comblées par des pads vides.
    pub fn set_pad(&mut self, index: usize, label: &str, sample: Sample) -> Result<(), KitError> {
        if index >= PAD_COUNT {
            return Err(KitError::Manifest(format!(
                "pad {index} hors de la grille (0 à {})",
                PAD_COUNT - 1
            )));
        }
        while self.pads.len() <= index {
            self.pads.push(Pad::empty(self.pads.len()));
        }
        let pad = &mut self.pads[index];
        pad.label = label.to_string();
        pad.sample = sample;
        pad.seed = None;
        Ok(())
    }

    /// Index du premier pad vide ou absent, s'il reste une place.
    pub fn first_free_pad(&self) -> Option<usize> {
        self.pads
            .iter()
            .position(Pad::is_empty)
            .or_else(|| (self.pads.len() < PAD_COUNT).then_some(self.pads.len()))
    }

    /// Sérialise le kit en `.kit.zip` (schéma v1) : `kit.json` et un WAV
    /// flottant 32 bits par pad sonore. Ce qu'un preset embarque après une
    /// modification.
    pub fn to_kit_zip(&self) -> Result<Vec<u8>, KitError> {
        let zip_err = |e: zip::result::ZipError| KitError::Zip(e.to_string());
        let io_err = |e: std::io::Error| KitError::Io(e.to_string());
        let mut cursor = Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(&mut cursor);
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored)
            .large_file(true);
        let mut manifest_pads = Vec::new();
        for pad in self.pads.iter().filter(|p| !p.is_empty()) {
            let file = format!("{:02}_{}.wav", pad.index + 1, safe_stem(&pad.label));
            let spec = hound::WavSpec {
                channels: pad.sample.channels,
                sample_rate: pad.sample.sample_rate,
                bits_per_sample: 32,
                sample_format: hound::SampleFormat::Float,
            };
            let mut wav = Cursor::new(Vec::new());
            {
                let mut w = hound::WavWriter::new(&mut wav, spec)
                    .map_err(|e| KitError::Wav(e.to_string()))?;
                for v in &pad.sample.data {
                    w.write_sample(*v)
                        .map_err(|e| KitError::Wav(e.to_string()))?;
                }
                w.finalize().map_err(|e| KitError::Wav(e.to_string()))?;
            }
            writer.start_file(&file, opts).map_err(zip_err)?;
            writer.write_all(&wav.into_inner()).map_err(io_err)?;
            manifest_pads.push(ManifestPad {
                index: pad.index,
                flavor: Some(pad.label.clone()),
                note: Some(pad.note),
                seed: pad.seed,
                file,
            });
        }
        let source = match &self.source {
            Some(src) if src.frames() > 0 => {
                writer.start_file(SOURCE_FILE, opts).map_err(zip_err)?;
                writer.write_all(&encode_wav(src)?).map_err(io_err)?;
                Some(ManifestSource {
                    file_name: None,
                    sample_rate: Some(src.sample_rate),
                    duration: Some(src.frames() as f64 / src.sample_rate as f64),
                    file: Some(SOURCE_FILE.to_string()),
                })
            }
            _ => None,
        };
        let manifest = Manifest {
            schema_version: 1,
            app: "make-it-drum",
            name: Some(self.name.clone()),
            source,
            shaping: (!self.shaping.is_default()).then_some(ManifestShaping {
                fidelity: self.shaping.fidelity,
                length: self.shaping.length,
            }),
            pads: manifest_pads,
        };
        let json =
            serde_json::to_vec_pretty(&manifest).map_err(|e| KitError::Manifest(e.to_string()))?;
        writer.start_file("kit.json", opts).map_err(zip_err)?;
        writer.write_all(&json).map_err(io_err)?;
        writer.finish().map_err(zip_err)?;
        Ok(cursor.into_inner())
    }
}

/// WAV flottant 32 bits (sans perte) d'un échantillon.
fn encode_wav(sample: &Sample) -> Result<Vec<u8>, KitError> {
    let spec = hound::WavSpec {
        channels: sample.channels,
        sample_rate: sample.sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let wav_err = |e: hound::Error| KitError::Wav(e.to_string());
    let mut wav = Cursor::new(Vec::new());
    {
        let mut w = hound::WavWriter::new(&mut wav, spec).map_err(wav_err)?;
        for v in &sample.data {
            w.write_sample(*v).map_err(wav_err)?;
        }
        w.finalize().map_err(wav_err)?;
    }
    Ok(wav.into_inner())
}

/// Nom de fichier sûr pour une entrée d'archive : ASCII alphanumérique, `-` et `_`.
fn safe_stem(label: &str) -> String {
    let s: String = label
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if s.is_empty() {
        "pad".to_string()
    } else {
        s
    }
}
