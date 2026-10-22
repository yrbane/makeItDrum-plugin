//! Chargement des kits et état persistant : le chemin du kit et, pour un
//! fichier ZIP, ses octets en base64 pour qu'un projet se rouvre sans lui.
use std::path::{Path, PathBuf};
use std::sync::Arc;

use base64::Engine as _;
use make_it_drum_core::kit::{Kit, KitError};
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
