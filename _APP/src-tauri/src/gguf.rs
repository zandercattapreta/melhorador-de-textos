// ==============================================================================
// SCRIPT: gguf.rs (melhorador-app)
// DESCRIÇÃO: Gerenciador de modelos GGUF (lista, download+hash, remoção, seleção)
// CHAMADO POR: comandos Tauri
// CONTRATO (RESPOSTA ESPERADA): arquivos em app_data/models/; seleção em models.json
// ==============================================================================

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelsState {
    pub selected: Option<String>,
    pub catalog: Vec<ModelEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelEntry {
    pub name: String,
    pub path: String,
    pub sha256: Option<String>,
    pub bytes: u64,
}

pub fn models_dir(app_data: &Path) -> PathBuf {
    app_data.join("models")
}

pub fn state_path(app_data: &Path) -> PathBuf {
    app_data.join("models.json")
}

pub fn load_state(app_data: &Path) -> ModelsState {
    let p = state_path(app_data);
    std::fs::read_to_string(p)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_state(app_data: &Path, state: &ModelsState) -> Result<(), String> {
    let p = state_path(app_data);
    let json = serde_json::to_string_pretty(state).map_err(|e| e.to_string())?;
    std::fs::write(p, json).map_err(|e| format!("models.json: {e}"))
}

pub fn refresh_catalog(app_data: &Path) -> Result<ModelsState, String> {
    let dir = models_dir(app_data);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut state = load_state(app_data);
    let mut catalog = Vec::new();
    for entry in std::fs::read_dir(&dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("gguf") {
            continue;
        }
        let meta = entry.metadata().map_err(|e| e.to_string())?;
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        catalog.push(ModelEntry {
            name: name.clone(),
            path: path.to_string_lossy().into_owned(),
            sha256: state
                .catalog
                .iter()
                .find(|c| c.name == name)
                .and_then(|c| c.sha256.clone()),
            bytes: meta.len(),
        });
    }
    catalog.sort_by(|a, b| a.name.cmp(&b.name));
    state.catalog = catalog;
    if let Some(sel) = &state.selected {
        if !state.catalog.iter().any(|c| &c.name == sel) {
            state.selected = None;
        }
    }
    save_state(app_data, &state)?;
    Ok(state)
}

pub fn select_model(app_data: &Path, name: &str) -> Result<ModelsState, String> {
    let mut state = refresh_catalog(app_data)?;
    if !state.catalog.iter().any(|c| c.name == name) {
        return Err(format!("Modelo não encontrado: {name}"));
    }
    state.selected = Some(name.into());
    save_state(app_data, &state)?;
    Ok(state)
}

pub fn remove_model(app_data: &Path, name: &str) -> Result<ModelsState, String> {
    let dir = models_dir(app_data);
    let path = dir.join(name);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| format!("rm: {e}"))?;
    }
    let mut state = refresh_catalog(app_data)?;
    if state.selected.as_deref() == Some(name) {
        state.selected = None;
        save_state(app_data, &state)?;
    }
    Ok(state)
}

pub fn file_sha256(path: &Path) -> Result<String, String> {
    let mut f = File::open(path).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 1024 * 64];
    loop {
        let n = f.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

/// Download HTTP(S) para models/; confere sha256 se fornecido.
pub fn download_model(
    app_data: &Path,
    url: &str,
    filename: &str,
    expected_sha256: Option<&str>,
) -> Result<ModelsState, String> {
    let dir = models_dir(app_data);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let dest = dir.join(filename);
    let resp = ureq::get(url)
        .call()
        .map_err(|e| format!("download: {e}"))?;
    let mut reader = resp.into_reader();
    let mut file = File::create(&dest).map_err(|e| e.to_string())?;
    let mut buf = [0u8; 1024 * 64];
    loop {
        let n = reader.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
    }
    drop(file);
    let hash = file_sha256(&dest)?;
    if let Some(exp) = expected_sha256 {
        if !exp.is_empty() && exp.to_lowercase() != hash {
            let _ = std::fs::remove_file(&dest);
            return Err(format!("SHA256 diverge: esperado {exp}, veio {hash}"));
        }
    }
    let mut state = refresh_catalog(app_data)?;
    if let Some(entry) = state.catalog.iter_mut().find(|c| c.name == filename) {
        entry.sha256 = Some(hash);
    }
    if state.selected.is_none() {
        state.selected = Some(filename.into());
    }
    save_state(app_data, &state)?;
    Ok(state)
}

pub fn selected_path(app_data: &Path) -> Option<PathBuf> {
    let state = load_state(app_data);
    let name = state.selected?;
    let p = models_dir(app_data).join(name);
    if p.is_file() {
        Some(p)
    } else {
        None
    }
}
