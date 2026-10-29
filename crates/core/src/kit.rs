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
use crate::wav::{decode_wav, encode_wav, Sample};

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

/// Réglages de lecture d'un pad, appliqués par le moteur à chaque frappe.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PadSettings {
    /// Gain en dB (−24 à +12).
    pub gain_db: f32,
    /// Transposition en demi-tons (−24 à +24).
    pub pitch: f32,
    /// Panoramique, −1 (gauche) à +1 (droite) ; 0 = sans atténuation.
    pub pan: f32,
    /// Part du son jouée, 0,05 à 1 ; en dessous de 1, fondu de sortie.
    pub decay: f32,
    /// Lecture à l'envers.
    pub reverse: bool,
    /// Groupe d'étouffement, 0 = aucun : une frappe coupe les autres pads du groupe.
    pub choke: u8,
    /// Verrouillé : variations, cadrage et variantes de frappe le laissent tel quel.
    pub locked: bool,
    /// « Cut itself » : une nouvelle frappe de ce pad coupe la précédente.
    pub cut_self: bool,
    /// « Cut by all » : n'importe quel autre pad frappé coupe celui-ci.
    pub cut_by_all: bool,
}

impl Default for PadSettings {
    fn default() -> Self {
        Self {
            gain_db: 0.0,
            pitch: 0.0,
            pan: 0.0,
            decay: 1.0,
            reverse: false,
            choke: 0,
            locked: false,
            cut_self: true,
            cut_by_all: false,
        }
    }
}

impl PadSettings {
    pub const MAX_CHOKE: u8 = 4;

    /// Ramène chaque réglage dans sa plage.
    pub fn clamped(self) -> Self {
        Self {
            gain_db: self.gain_db.clamp(-24.0, 12.0),
            pitch: self.pitch.clamp(-24.0, 24.0),
            pan: self.pan.clamp(-1.0, 1.0),
            decay: if self.decay.is_finite() {
                self.decay.clamp(0.05, 1.0)
            } else {
                1.0
            },
            reverse: self.reverse,
            choke: self.choke.min(Self::MAX_CHOKE),
            locked: self.locked,
            cut_self: self.cut_self,
            cut_by_all: self.cut_by_all,
        }
    }

    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// Facteur de gain linéaire.
    pub fn gain(&self) -> f32 {
        10f32.powf(self.gain_db / 20.0)
    }

    /// Multiplicateur de vitesse de lecture pour la transposition.
    pub fn rate(&self) -> f64 {
        2f64.powf(self.pitch as f64 / 12.0)
    }

    /// Gains (gauche, droite) du panoramique : au centre, 1 et 1.
    pub fn pan_gains(&self) -> (f32, f32) {
        ((1.0 - self.pan).min(1.0), (1.0 + self.pan).min(1.0))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Pad {
    pub index: usize,
    pub label: String,
    pub note: u8,
    pub sample: Sample,
    /// Graine du rendu, si le pad a été généré à partir du son source.
    pub seed: Option<u32>,
    /// Réglages de lecture.
    pub settings: PadSettings,
    /// Variantes de frappe (graine, son) jouées à tour de rôle avec `sample`.
    pub alternates: Vec<(u32, Sample)>,
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
            settings: PadSettings::default(),
            alternates: Vec::new(),
        }
    }

    /// Nombre de sons jouables à tour de rôle : le principal plus les variantes.
    pub fn variant_count(&self) -> usize {
        1 + self.alternates.len()
    }

    /// Le son `k` (0 = principal, puis les variantes) ; au-delà, modulo.
    pub fn sample_at(&self, k: usize) -> &Sample {
        let n = self.variant_count();
        match k % n {
            0 => &self.sample,
            i => &self.alternates[i - 1].1,
        }
    }

    /// Graine de la variante `k` (1 et plus), dérivée de la graine du pad.
    pub fn alternate_seed(seed: u32, k: usize) -> u32 {
        seed.wrapping_add(0x9E37_79B9u32.wrapping_mul(k as u32))
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

/// Nombre maximal de sons à tour de rôle par pad.
pub const MAX_ROUND_ROBIN: usize = 4;

/// Rend `jobs` (pad, graine) sur tous les cœurs, et livre chaque son à `done`
/// sur le thread appelant, dans l'ordre d'achèvement. Les rendus sont
/// déterministes par graine : l'ordre ne change pas le résultat.
fn render_parallel(
    jobs: &[(usize, u32)],
    src: &SourceSample,
    shaping: &Shaping,
    mut done: impl FnMut(usize, u32, Sample),
) {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc;
    if jobs.is_empty() {
        return;
    }
    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .clamp(1, jobs.len());
    let next = AtomicUsize::new(0);
    let (tx, rx) = mpsc::channel();
    std::thread::scope(|scope| {
        for _ in 0..threads {
            let tx = tx.clone();
            let next = &next;
            scope.spawn(move || loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                let Some(&(index, seed)) = jobs.get(i) else {
                    break;
                };
                let sample = render_pad_with(flavor_at(index), src, seed, shaping);
                if tx.send((index, seed, sample)).is_err() {
                    break;
                }
            });
        }
        drop(tx);
        for (index, seed, sample) in rx.iter().take(jobs.len()) {
            done(index, seed, sample);
        }
    });
}

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
    #[serde(default)]
    start: f64,
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
    /// Réglages de lecture (plugin) ; absents quand ils sont par défaut.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    settings: Option<PadSettings>,
    /// Variantes de frappe (plugin).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    alternates: Vec<ManifestAlternate>,
}

#[derive(Serialize, Deserialize)]
struct ManifestAlternate {
    seed: u32,
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
                let mut alternates = Vec::with_capacity(p.alternates.len());
                for alt in &p.alternates {
                    let bytes = read_entry(&mut archive, &format!("{prefix}{}", alt.file))?;
                    alternates.push((alt.seed, decode_wav(&bytes)?));
                }
                pads.push(Pad {
                    index: p.index,
                    label: p.flavor.unwrap_or_else(|| stem(&p.file)),
                    note: p.note.unwrap_or(BASE_NOTE.saturating_add(p.index as u8)),
                    sample: decode_wav(&wav)?,
                    seed: p.seed,
                    settings: p.settings.unwrap_or_default().clamped(),
                    alternates,
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
                .map(|s| Shaping::new(s.fidelity, s.length).with_start(s.start))
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
                settings: PadSettings::default(),
                alternates: Vec::new(),
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
                settings: PadSettings::default(),
                alternates: Vec::new(),
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
        let src = SourceSample::prepare_from(source, shaping.start);
        let mut kit = Kit {
            name: name.to_string(),
            pads: (0..PAD_COUNT).map(Pad::empty).collect(),
            // le son entier, pour pouvoir déplacer le départ ensuite
            source: Some(SourceSample::prepare(source).to_sample()),
            shaping,
        };
        let jobs: Vec<(usize, u32)> = (0..PAD_COUNT)
            .map(|index| {
                let seed = seeds
                    .and_then(|s| s.get(index).copied())
                    .unwrap_or_else(fresh_seed);
                (index, seed)
            })
            .collect();
        render_parallel(&jobs, &src, &shaping, |index, seed, sample| {
            kit.pads[index] = Pad {
                index,
                label: RECIPES[index].name.to_string(),
                note: BASE_NOTE + index as u8,
                sample,
                seed: Some(seed),
                settings: PadSettings::default(),
                alternates: Vec::new(),
            };
            progress(&kit, index);
        });
        Ok(kit)
    }

    /// Rejoue la recette du pad `index` avec une nouvelle graine (ou `seed`),
    /// à partir du son source du kit. Le nom, la note du pad sont conservés.
    pub fn regenerate_pad(&mut self, index: usize, seed: Option<u32>) -> Result<(), KitError> {
        let source = self.source.as_ref().ok_or(KitError::NoSource)?;
        if index >= PAD_COUNT {
            return Err(KitError::Manifest(format!("pad {index} hors de la grille")));
        }
        let src = SourceSample::prepare_from(source, self.shaping.start);
        let seed = seed.unwrap_or_else(fresh_seed);
        let flavor = flavor_at(index);
        let sample = render_pad_with(flavor, &src, seed, &self.shaping);
        while self.pads.len() <= index {
            self.pads.push(Pad::empty(self.pads.len()));
        }
        let shaping = self.shaping;
        let pad = &mut self.pads[index];
        if pad.is_empty() {
            pad.label = flavor.name.to_string();
        }
        pad.sample = sample;
        pad.seed = Some(seed);
        for (k, alt) in pad.alternates.iter_mut().enumerate() {
            alt.0 = Pad::alternate_seed(seed, k + 1);
            alt.1 = render_pad_with(flavor, &src, alt.0, &shaping);
        }
        Ok(())
    }

    /// Donne à chaque pad généré (et non verrouillé) `count` sons à tour de
    /// rôle : le principal plus `count − 1` variantes, rendues avec des
    /// graines dérivées de la sienne. `count` = 1 retire les variantes.
    /// `progress` est appelé après chaque pad traité.
    pub fn set_round_robin(
        &mut self,
        count: usize,
        mut progress: impl FnMut(&Kit, usize),
    ) -> Result<(), KitError> {
        let count = count.clamp(1, MAX_ROUND_ROBIN);
        let src = match &self.source {
            Some(s) => Some(SourceSample::prepare_from(s, self.shaping.start)),
            None if count > 1 => return Err(KitError::NoSource),
            None => None,
        };
        let shaping = self.shaping;
        for index in 0..self.pads.len() {
            let pad = &self.pads[index];
            let Some(seed) = pad.seed else { continue };
            if pad.settings.locked {
                continue;
            }
            let wanted = count - 1;
            if pad.alternates.len() == wanted {
                continue;
            }
            let flavor = flavor_at(index);
            let mut alternates = std::mem::take(&mut self.pads[index].alternates);
            alternates.truncate(wanted);
            if let Some(src) = &src {
                while alternates.len() < wanted {
                    let k = alternates.len() + 1;
                    let alt_seed = Pad::alternate_seed(seed, k);
                    alternates.push((alt_seed, render_pad_with(flavor, src, alt_seed, &shaping)));
                }
            }
            self.pads[index].alternates = alternates;
            progress(self, index);
        }
        Ok(())
    }

    /// Nombre de sons à tour de rôle du kit : le maximum sur les pads.
    pub fn round_robin(&self) -> usize {
        self.pads.iter().map(Pad::variant_count).max().unwrap_or(1)
    }

    /// Change les réglages de lecture du pad `index`.
    pub fn set_settings(&mut self, index: usize, settings: PadSettings) -> Result<(), KitError> {
        let pad = self
            .pads
            .get_mut(index)
            .ok_or_else(|| KitError::Manifest(format!("pad {index} hors de la grille")))?;
        pad.settings = settings.clamped();
        Ok(())
    }

    /// Renomme le pad `index`.
    pub fn rename_pad(&mut self, index: usize, label: &str) -> Result<(), KitError> {
        let pad = self
            .pads
            .get_mut(index)
            .ok_or_else(|| KitError::Manifest(format!("pad {index} hors de la grille")))?;
        pad.label = label.trim().to_string();
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
        let src = SourceSample::prepare_from(source, shaping.start);
        self.shaping = shaping;
        // un travail par son à rejouer : (pad, graine), le principal puis ses variantes
        let mut jobs: Vec<(usize, u32)> = Vec::new();
        for (index, pad) in self.pads.iter().enumerate() {
            let Some(seed) = pad.seed else { continue };
            if pad.settings.locked {
                continue;
            }
            jobs.push((index, seed));
            jobs.extend(pad.alternates.iter().map(|(s, _)| (index, *s)));
        }
        let mut remaining: Vec<usize> = self.pads.iter().map(|p| p.variant_count()).collect();
        render_parallel(&jobs, &src, &shaping, |index, seed, sample| {
            let pad = &mut self.pads[index];
            if pad.seed == Some(seed) {
                pad.sample = sample;
            } else if let Some(alt) = pad.alternates.iter_mut().find(|a| a.0 == seed) {
                alt.1 = sample;
            }
            remaining[index] -= 1;
            if remaining[index] == 0 {
                progress(self, index);
            }
        });
        Ok(())
    }

    /// Position du transitoire le plus fort du son source, en fraction de sa
    /// durée ; `None` sans source.
    pub fn source_onset(&self) -> Option<f64> {
        let src = self.source.as_ref()?;
        let frames = src.frames();
        if frames == 0 {
            return None;
        }
        let mono = SourceSample::prepare(src);
        Some(crate::analysis::onset(&mono.data, src.sample_rate) as f64 / frames as f64)
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
        pad.alternates.clear();
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
            let stem = format!("{:02}_{}", pad.index + 1, safe_stem(&pad.label));
            let file = format!("{stem}.wav");
            writer.start_file(&file, opts).map_err(zip_err)?;
            writer
                .write_all(&encode_wav(&pad.sample)?)
                .map_err(io_err)?;
            let mut alternates = Vec::with_capacity(pad.alternates.len());
            for (k, (seed, sample)) in pad.alternates.iter().enumerate() {
                let alt_file = format!("{stem}_v{}.wav", k + 2);
                writer.start_file(&alt_file, opts).map_err(zip_err)?;
                writer.write_all(&encode_wav(sample)?).map_err(io_err)?;
                alternates.push(ManifestAlternate {
                    seed: *seed,
                    file: alt_file,
                });
            }
            manifest_pads.push(ManifestPad {
                index: pad.index,
                flavor: Some(pad.label.clone()),
                note: Some(pad.note),
                seed: pad.seed,
                file,
                settings: (!pad.settings.is_default()).then_some(pad.settings),
                alternates,
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
                start: self.shaping.start,
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
