// ==============================================================================
// SCRIPT: gguf.rs (melhorador-app)
// DESCRIÇÃO: Gerenciador GGUF — CoTypist (Gemma) + modelos baixados pelo usuário
// CHAMADO POR: comandos Tauri
// CONTRATO: catálogo une pastas externas + app_data/models/; seleção em models.json
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
    /// "cotypist" | "app" — só "app" pode ser apagado pelo Melhorador
    #[serde(default = "default_source_app")]
    pub source: String,
}

fn default_source_app() -> String {
    "app".into()
}

pub fn models_dir(app_data: &Path) -> PathBuf {
    app_data.join("models")
}

pub fn state_path(app_data: &Path) -> PathBuf {
    app_data.join("models.json")
}

/// Pasta de modelos do CoTypist (Gemma 4 etc.).
pub fn cotypist_models_dir() -> Option<PathBuf> {
    let home = dirs_home()?;
    let p = home
        .join("Library/Application Support/app.cotypist.Cotypist/Models");
    if p.is_dir() {
        Some(p)
    } else {
        None
    }
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
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

fn scan_gguf_dir(dir: &Path, source: &str, prev: &[ModelEntry]) -> Vec<ModelEntry> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in rd.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("gguf") {
            continue;
        }
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        out.push(ModelEntry {
            name: name.clone(),
            path: path.to_string_lossy().into_owned(),
            sha256: prev
                .iter()
                .find(|c| c.name == name && c.source == source)
                .and_then(|c| c.sha256.clone()),
            bytes: meta.len(),
            source: source.into(),
        });
    }
    out
}

pub fn refresh_catalog(app_data: &Path) -> Result<ModelsState, String> {
    let dir = models_dir(app_data);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut state = load_state(app_data);
    let prev = state.catalog.clone();
    let mut catalog = Vec::new();

    // 1) CoTypist primeiro (Gemma 4 do Zander / quem tiver o app)
    if let Some(ct) = cotypist_models_dir() {
        catalog.extend(scan_gguf_dir(&ct, "cotypist", &prev));
    }
    // 2) Modelos baixados pelo Melhorador
    catalog.extend(scan_gguf_dir(&dir, "app", &prev));

    catalog.sort_by(|a, b| {
        // Preferir cotypist gemma no topo, depois nome
        let sa = if a.source == "cotypist" { 0 } else { 1 };
        let sb = if b.source == "cotypist" { 0 } else { 1 };
        sa.cmp(&sb).then_with(|| a.name.cmp(&b.name))
    });
    state.catalog = catalog;

    // Seleção: manter se ainda existe; senão default = Gemma CoTypist se houver
    let still_ok = state
        .selected
        .as_ref()
        .map(|s| state.catalog.iter().any(|c| &c.name == s))
        .unwrap_or(false);
    if !still_ok {
        state.selected = state
            .catalog
            .iter()
            .find(|c| c.source == "cotypist" && c.name.to_lowercase().contains("gemma"))
            .or_else(|| state.catalog.iter().find(|c| c.source == "cotypist"))
            .or_else(|| state.catalog.first())
            .map(|c| c.name.clone());
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
    let state = refresh_catalog(app_data)?;
    let entry = state
        .catalog
        .iter()
        .find(|c| c.name == name)
        .ok_or_else(|| format!("Modelo não encontrado: {name}"))?;
    if entry.source == "cotypist" {
        return Err(
            "Este modelo é do CoTypist. Não apague por aqui — troque de modelo ou use o CoTypist."
                .into(),
        );
    }
    let path = PathBuf::from(&entry.path);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| format!("rm: {e}"))?;
    }
    let mut state = refresh_catalog(app_data)?;
    if state.selected.as_deref() == Some(name) {
        state.selected = None;
        // refresh de novo escolhe default CoTypist se houver
        state = refresh_catalog(app_data)?;
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

/// Download HTTP(S) para models/ do app; confere sha256 se fornecido.
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
    // Usuário baixou de propósito → seleciona o novo
    state.selected = Some(filename.into());
    save_state(app_data, &state)?;
    Ok(state)
}

pub fn selected_path(app_data: &Path) -> Option<PathBuf> {
    let state = refresh_catalog(app_data).ok()?;
    let name = state.selected?;
    let entry = state.catalog.iter().find(|c| c.name == name)?;
    let p = PathBuf::from(&entry.path);
    if p.is_file() {
        Some(p)
    } else {
        None
    }
}

/// Opções recomendadas para o usuário (sem precisar saber URL).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogOffer {
    pub id: String,
    pub label: String,
    pub detail: String,
    pub filename: String,
    pub url: String,
    /// Já disponível no Mac (CoTypist / pasta do app)?
    pub available_locally: bool,
}

pub fn recommended_offers(app_data: &Path) -> Vec<CatalogOffer> {
    let state = refresh_catalog(app_data).unwrap_or_default();
    let has = |name: &str| state.catalog.iter().any(|c| c.name.contains(name));

    let mut offers = vec![
        CatalogOffer {
            id: "cotypist-gemma".into(),
            label: "Gemma 4 (CoTypist)".into(),
            detail: "Já no seu Mac se o CoTypist estiver instalado — recomendado.".into(),
            filename: "gemma-4-E4B-UD-Q5_K_XL.gguf".into(),
            url: String::new(),
            available_locally: has("gemma-4"),
        },
        CatalogOffer {
            id: "qwen-0.8b".into(),
            label: "Qwen 0.8B (leve)".into(),
            detail: "Pequeno e rápido — bom para testar IA local.".into(),
            filename: "Qwen3.5-0.8B-Base.i1-Q6_K.gguf".into(),
            url: "https://huggingface.co/mradermacher/Qwen3.5-0.8B-Base-i1-GGUF/resolve/main/Qwen3.5-0.8B-Base.i1-Q6_K.gguf".into(),
            available_locally: has("Qwen3.5-0.8B"),
        },
        CatalogOffer {
            id: "gemma-e2b".into(),
            label: "Gemma 4 E2B (médio)".into(),
            detail: "Equilíbrio tamanho/qualidade (download ~4–5 GB).".into(),
            filename: "gemma-4-E2B.i1-Q6_K.gguf".into(),
            url: "https://huggingface.co/mradermacher/gemma-4-E2B-i1-GGUF/resolve/main/gemma-4-E2B.i1-Q6_K.gguf".into(),
            available_locally: has("gemma-4-E2B") || has("E2B"),
        },
    ];
    // Se CoTypist não tem o arquivo, marca unavailable
    if !offers[0].available_locally {
        offers[0].detail =
            "Instale o CoTypist ou baixe outro modelo da lista.".into();
    }
    offers
}

/// Usa oferta: se local (CoTypist), só seleciona; se URL, baixa.
pub fn install_offer(app_data: &Path, offer_id: &str) -> Result<ModelsState, String> {
    let offers = recommended_offers(app_data);
    let offer = offers
        .into_iter()
        .find(|o| o.id == offer_id)
        .ok_or_else(|| "Opção desconhecida".to_string())?;
    if offer.id == "cotypist-gemma" {
        let mut state = refresh_catalog(app_data)?;
        let name = state
            .catalog
            .iter()
            .find(|c| c.source == "cotypist" && c.name.to_lowercase().contains("gemma"))
            .map(|c| c.name.clone())
            .ok_or("Gemma do CoTypist não encontrado neste Mac")?;
        state.selected = Some(name);
        save_state(app_data, &state)?;
        return Ok(state);
    }
    if offer.url.is_empty() {
        return Err("Esta opção não tem download automático".into());
    }
    download_model(app_data, &offer.url, &offer.filename, None)
}
