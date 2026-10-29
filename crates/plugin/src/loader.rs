//! Chargement des kits et état persistant : le chemin du kit et, pour un
//! fichier ZIP, ses octets en base64 pour qu'un projet se rouvre sans lui.
use std::path::{Path, PathBuf};
use std::sync::Arc;

use base64::Engine as _;
use make_it_drum_core::kit::{Kit, KitError, PadSettings};
use make_it_drum_core::render::Shaping;
use make_it_drum_core::wav::decode_audio;
use make_it_drum_core::wav::Sample;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct KitState {
    pub path: Option<String>,
    pub name: Option<String>,
    pub zip_base64: Option<String>,
}

/// Charge un kit depuis un `.zip` ou un dossier et construit l'état à persister.
pub fn load_from_path(path: &Path) -> Result<(Arc<Kit>, KitState), KitError> {
    let kit = Kit::from_path(path)?;
    let zip_base64 = if path.is_file() {
        std::fs::read(path)
            .ok()
            .map(|bytes| base64::engine::general_purpose::STANDARD.encode(bytes))
    } else {
        None
    };
    let state = KitState {
        path: Some(path.to_string_lossy().into_owned()),
        name: Some(kit.name.clone()),
        zip_base64,
    };
    Ok((Arc::new(kit), state))
}

/// Nom de kit d'une instance qui n'en avait pas quand on y dépose un premier son.
pub const SCRATCH_KIT_NAME: &str = "kit perso";

/// L'état à persister pour un kit fabriqué ou modifié dans le plugin : le kit
/// entier, embarqué en `.kit.zip` (graines et son source compris).
pub fn state_for(kit: &Kit) -> Result<KitState, KitError> {
    let zip = kit.to_kit_zip()?;
    Ok(KitState {
        path: None,
        name: Some(kit.name.clone()),
        zip_base64: Some(base64::engine::general_purpose::STANDARD.encode(zip)),
    })
}

/// Lit un fichier audio quel que soit son format.
pub fn read_audio(path: &Path) -> Result<make_it_drum_core::wav::Sample, KitError> {
    let bytes = std::fs::read(path).map_err(|e| KitError::Io(e.to_string()))?;
    let extension = path.extension().map(|e| e.to_string_lossy().into_owned());
    decode_audio(&bytes, extension.as_deref())
}

/// Fabrique un kit de 20 percussions à partir du fichier audio `path`, comme
/// la page, cadré par `shaping`. `progress` reçoit le kit après chaque pad rendu.
pub fn generate_from_path(
    path: &Path,
    shaping: Shaping,
    progress: impl FnMut(&Kit, usize),
) -> Result<(Arc<Kit>, KitState), KitError> {
    let sample = read_audio(path)?;
    let name = path
        .file_stem()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "kit".to_string());
    let kit = Kit::generate_with(&name, &sample, None, shaping, progress)?;
    let state = state_for(&kit)?;
    Ok((Arc::new(kit), state))
}

/// Fabrique un kit de 20 percussions à partir d'un son déjà décodé (capture).
pub fn generate_from_sample(
    name: &str,
    sample: &Sample,
    shaping: Shaping,
    progress: impl FnMut(&Kit, usize),
) -> Result<(Arc<Kit>, KitState), KitError> {
    let kit = Kit::generate_with(name, sample, None, shaping, progress)?;
    let state = state_for(&kit)?;
    Ok((Arc::new(kit), state))
}

/// Change les réglages de lecture d'un pad du kit courant.
pub fn set_settings(
    current: &Kit,
    pad: usize,
    settings: PadSettings,
) -> Result<(Arc<Kit>, KitState), KitError> {
    let mut kit = current.clone();
    kit.set_settings(pad, settings)?;
    let state = state_for(&kit)?;
    Ok((Arc::new(kit), state))
}

/// Renomme un pad du kit courant.
pub fn rename_pad(
    current: &Kit,
    pad: usize,
    label: &str,
) -> Result<(Arc<Kit>, KitState), KitError> {
    let mut kit = current.clone();
    kit.rename_pad(pad, label)?;
    let state = state_for(&kit)?;
    Ok((Arc::new(kit), state))
}

/// Donne `count` sons à tour de rôle à chaque pad généré du kit courant.
pub fn set_round_robin(
    current: &Kit,
    count: usize,
    progress: impl FnMut(&Kit, usize),
) -> Result<(Arc<Kit>, KitState), KitError> {
    let mut kit = current.clone();
    kit.set_round_robin(count, progress)?;
    let state = state_for(&kit)?;
    Ok((Arc::new(kit), state))
}

/// Dossier des exports : `MAKEITDRUM_EXPORTS` sinon `~/Documents/makeitdrum/exports`.
pub fn exports_dir() -> PathBuf {
    if let Some(p) = std::env::var_os("MAKEITDRUM_EXPORTS") {
        return PathBuf::from(p);
    }
    dirs::document_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("makeitdrum")
        .join("exports")
}

/// Rejoue les pads générés du kit courant avec leurs graines et le cadrage
/// `shaping` : même idée de son, plus ou moins proche de l'original.
pub fn reshape(
    current: &Kit,
    shaping: Shaping,
    progress: impl FnMut(&Kit, usize),
) -> Result<(Arc<Kit>, KitState), KitError> {
    let mut kit = current.clone();
    kit.reshape(shaping, progress)?;
    let state = state_for(&kit)?;
    Ok((Arc::new(kit), state))
}

/// Nouvelle variation d'un pad (ou de tous si `pad` est `None`) du kit courant,
/// avec le cadrage `shaping`.
pub fn regenerate(
    current: &Kit,
    pad: Option<usize>,
    shaping: Shaping,
) -> Result<(Arc<Kit>, KitState), KitError> {
    let mut kit = current.clone();
    kit.shaping = shaping;
    match pad {
        Some(i) => kit.regenerate_pad(i, None)?,
        None => {
            for i in 0..kit.pads.len().max(if kit.has_source() { 20 } else { 0 }) {
                kit.regenerate_pad(i, None)?;
            }
        }
    }
    let state = state_for(&kit)?;
    Ok((Arc::new(kit), state))
}

/// Met le fichier audio `path` sur le pad `pad` du kit courant (ou d'un kit
/// neuf), et construit l'état à persister : le kit modifié tout entier,
/// embarqué en `.kit.zip`, pour que le projet se rouvre sans le fichier.
pub fn replace_pad(
    current: Option<&Kit>,
    pad: usize,
    path: &Path,
) -> Result<(Arc<Kit>, KitState), KitError> {
    let sample = read_audio(path)?;
    let label = path
        .file_stem()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "son".to_string());
    let mut kit = current
        .cloned()
        .unwrap_or_else(|| Kit::empty(SCRATCH_KIT_NAME));
    kit.set_pad(pad, &label, sample)?;
    let state = state_for(&kit)?;
    Ok((Arc::new(kit), state))
}

/// Recharge depuis l'état : les octets embarqués d'abord, le chemin ensuite.
pub fn load_from_state(state: &KitState) -> Result<(Arc<Kit>, KitState), KitError> {
    if let Some(b64) = &state.zip_base64 {
        if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(b64) {
            let name = state.name.clone().unwrap_or_else(|| "kit".into());
            let kit = Kit::from_zip_bytes(&name, &bytes)?;
            return Ok((Arc::new(kit), state.clone()));
        }
    }
    match &state.path {
        Some(p) => load_from_path(Path::new(p)),
        None => Err(KitError::Empty),
    }
}

/// Dossier des kits : `MAKEITDRUM_KITS` sinon `~/Documents/makeitdrum/kits`.
pub fn kits_dir() -> PathBuf {
    if let Some(p) = std::env::var_os("MAKEITDRUM_KITS") {
        return PathBuf::from(p);
    }
    dirs::document_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("makeitdrum")
        .join("kits")
}

/// Les fichiers `.zip` d'un dossier, triés par nom ; vide si le dossier manque.
pub fn scan_kits(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut kits: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.extension()
                    .map(|x| x.eq_ignore_ascii_case("zip"))
                    .unwrap_or(false)
        })
        .collect();
    kits.sort();
    kits
}

/// Le premier kit du dossier, pour démarrer avec un son prêt à jouer.
pub fn first_kit(dir: &Path) -> Option<PathBuf> {
    scan_kits(dir).into_iter().next()
}

/// Nom affiché d'un fichier de kit : sans `.kit.zip` ni `.zip`.
pub fn display_name(path: &Path) -> String {
    let base = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    base.strip_suffix(".kit.zip")
        .or_else(|| base.strip_suffix(".zip"))
        .unwrap_or(&base)
        .to_string()
}

/// Les graines du kit, pad par pad (`None` : pad vide ou son déposé).
pub fn seeds_of(kit: &Kit) -> Vec<Option<u32>> {
    (0..make_it_drum_core::kit::PAD_COUNT)
        .map(|i| kit.pads.get(i).and_then(|p| p.seed))
        .collect()
}

/// Rejoue les pads générés du kit courant avec les graines d'une série
/// précédente (les pads déposés à la main ne bougent pas), avec le cadrage
/// `shaping`.
pub fn regenerate_with_seeds(
    current: &Kit,
    seeds: &[Option<u32>],
    shaping: Shaping,
    progress: impl FnMut(&Kit, usize),
) -> Result<(Arc<Kit>, KitState), KitError> {
    let mut kit = current.clone();
    for (pad, seed) in kit.pads.iter_mut().zip(seeds) {
        if let (Some(_), Some(s)) = (pad.seed, seed) {
            pad.seed = Some(*s);
            for (k, alt) in pad.alternates.iter_mut().enumerate() {
                alt.0 = make_it_drum_core::kit::Pad::alternate_seed(*s, k + 1);
            }
        }
    }
    kit.reshape(shaping, progress)?;
    let state = state_for(&kit)?;
    Ok((Arc::new(kit), state))
}

/// Met « cut itself » et/ou « cut by all » sur tous les pads du kit courant
/// (`None` : inchangé).
pub fn set_cut_all(
    current: &Kit,
    cut_self: Option<bool>,
    cut_by_all: Option<bool>,
) -> Result<(Arc<Kit>, KitState), KitError> {
    let mut kit = current.clone();
    for i in 0..kit.pads.len() {
        let mut s = kit.pads[i].settings;
        if let Some(v) = cut_self {
            s.cut_self = v;
        }
        if let Some(v) = cut_by_all {
            s.cut_by_all = v;
        }
        kit.set_settings(i, s)?;
    }
    let state = state_for(&kit)?;
    Ok((Arc::new(kit), state))
}
