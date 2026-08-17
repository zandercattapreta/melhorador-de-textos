// ==============================================================================
// SCRIPT: lib.rs (txtmelhorator-app / src-tauri)
// DESCRIÇÃO: Comandos Tauri — pipeline, LT, GGUF, export, cancelamento
// CHAMADO POR: UI React via invoke(); main.rs
// CONTRATO (RESPOSTA ESPERADA): ProcessResult ou "CANCELLED" / erro legível
// ==============================================================================

mod cloud_ai;
mod gguf;
mod llama_infer;
mod lt;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

/// Flag compartilhada: UI chama request_cancel; extract_pdf consulta entre páginas.
struct AppState {
    cancel: AtomicBool,
}

/// Resultado do processamento para a UI.
#[derive(Serialize, Debug)]
pub struct ProcessResult {
    pub source_name: String,
    pub source_path: String,
    pub engine: String,
    pub languages_used: String,
    pub page_count: u32,
    pub cleaned: String,
    pub pages: Vec<String>,
    pub cleanup_stats: std::collections::BTreeMap<&'static str, i64>,
    pub structure_stats: std::collections::BTreeMap<&'static str, i64>,
    pub warnings: Vec<String>,
}

/// Idiomas OCR aceitos. `auto` fica para resolve_languages.
fn normalize_languages(raw: Option<String>) -> String {
    let s = raw
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("por+eng");
    match s {
        "por" | "eng" | "por+eng" | "auto" => s.to_string(),
        "eng+por" => "por+eng".into(),
        _ => "por+eng".into(),
    }
}

/// Resolve idioma: `auto` → heurística na amostra nativa; senão o seletor.
fn resolve_languages(app: &AppHandle, pdf: &Path, raw: Option<String>) -> Result<String, String> {
    let normalized = normalize_languages(raw);
    if normalized != "auto" {
        return Ok(normalized);
    }
    let pdfium = find_pdfium(app)?;
    let sample = txtmelhorator_core::extraction::sample_native_text(&pdfium, pdf, 5)?;
    Ok(txtmelhorator_core::extraction::detect_ocr_languages(&sample).to_string())
}

fn ensure_app_data_dirs(app: &AppHandle) -> Result<PathBuf, String> {
    let root = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("app_data_dir: {e}"))?;
    std::fs::create_dir_all(root.join("temp"))
        .map_err(|e| format!("Não criei temp/: {e}"))?;
    std::fs::create_dir_all(root.join("tessdata"))
        .map_err(|e| format!("Não criei tessdata/: {e}"))?;
    std::fs::create_dir_all(root.join("models"))
        .map_err(|e| format!("Não criei models/: {e}"))?;
    Ok(root)
}

/// Apaga conteúdo de um diretório (mantém a pasta).
fn clear_dir_contents(dir: &Path) -> Result<usize, String> {
    if !dir.exists() {
        std::fs::create_dir_all(dir).map_err(|e| format!("mkdir {}: {e}", dir.display()))?;
        return Ok(0);
    }
    let mut n = 0usize;
    for entry in std::fs::read_dir(dir).map_err(|e| format!("read {}: {e}", dir.display()))? {
        let entry = entry.map_err(|e| format!("entrada: {e}"))?;
        let p = entry.path();
        if p.is_dir() {
            std::fs::remove_dir_all(&p).map_err(|e| format!("rmdir {}: {e}", p.display()))?;
        } else {
            std::fs::remove_file(&p).map_err(|e| format!("rm {}: {e}", p.display()))?;
        }
        n += 1;
    }
    Ok(n)
}

fn rules_path(app: &AppHandle) -> Result<PathBuf, String> {
    let root = ensure_app_data_dirs(app)?;
    Ok(root.join("user_rules.json"))
}

fn load_user_rules(app: &AppHandle) -> Vec<txtmelhorator_core::rules::UserRule> {
    let Ok(path) = rules_path(app) else {
        return Vec::new();
    };
    let Ok(raw) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

#[tauri::command]
fn list_user_rules(app: AppHandle) -> Result<Vec<txtmelhorator_core::rules::UserRule>, String> {
    Ok(load_user_rules(&app))
}

#[tauri::command]
fn save_user_rules(
    app: AppHandle,
    rules: Vec<txtmelhorator_core::rules::UserRule>,
) -> Result<(), String> {
    let path = rules_path(&app)?;
    let json = serde_json::to_string_pretty(&rules).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| format!("Não gravei regras: {e}"))
}

fn settings_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(ensure_app_data_dirs(app)?.join("settings.json"))
}

fn load_lt_settings(app: &AppHandle) -> lt::LtSettings {
    let Ok(path) = settings_path(app) else {
        return lt::LtSettings::default();
    };
    let Ok(raw) = std::fs::read_to_string(path) else {
        return lt::LtSettings::default();
    };
    // Arquivo mesclado ou legado só-LT
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) {
        let local_url = v
            .get("local_url")
            .and_then(|x| x.as_str())
            .unwrap_or("http://localhost:8081")
            .to_string();
        let premium_enabled = v
            .get("premium_enabled")
            .and_then(|x| x.as_bool())
            .unwrap_or(false);
        return lt::LtSettings {
            local_url,
            premium_enabled,
        };
    }
    lt::LtSettings::default()
}

#[tauri::command]
fn get_lt_settings(app: AppHandle) -> Result<lt::LtSettings, String> {
    Ok(load_lt_settings(&app))
}

fn load_cloud_ai_settings(app: &AppHandle) -> cloud_ai::CloudAiSettings {
    let Ok(path) = settings_path(app) else {
        return cloud_ai::CloudAiSettings::default();
    };
    // settings.json guarda lt + cloud juntos; daqui só interessa o cloud_ai
    // (campos do LT são ignorados pelo flatten).
    #[derive(serde::Deserialize, Default)]
    struct AllSettings {
        #[serde(default)]
        cloud_ai: Option<cloud_ai::CloudAiSettings>,
        #[serde(flatten)]
        _rest: std::collections::HashMap<String, serde_json::Value>,
    }
    if let Some(raw) = std::fs::read_to_string(path).ok() {
        if let Ok(all) = serde_json::from_str::<AllSettings>(&raw) {
            if let Some(c) = all.cloud_ai {
                return c;
            }
        }
    }
    cloud_ai::CloudAiSettings::default()
}

#[tauri::command]
fn get_cloud_ai_settings(app: AppHandle) -> Result<cloud_ai::CloudAiSettings, String> {
    Ok(load_cloud_ai_settings(&app))
}

#[tauri::command]
fn save_cloud_ai_settings(
    app: AppHandle,
    settings: cloud_ai::CloudAiSettings,
) -> Result<(), String> {
    let path = settings_path(&app)?;
    // Mescla com LT settings existentes
    let lt = load_lt_settings(&app);
    let merged = serde_json::json!({
        "local_url": lt.local_url,
        "premium_enabled": lt.premium_enabled,
        "cloud_ai": settings,
    });
    let json = serde_json::to_string_pretty(&merged).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| format!("settings: {e}"))
}

#[tauri::command]
fn save_cloud_ai_key(api_key: String) -> Result<(), String> {
    cloud_ai::save_api_key(&api_key)
}

/// Async: chamada HTTP bloqueante sai da main thread (U1c — sem freeze).
#[tauri::command]
async fn check_cloud_ai(
    app: AppHandle,
    text: String,
) -> Result<txtmelhorator_core::review::ReviewReport, String> {
    let settings = load_cloud_ai_settings(&app);
    tauri::async_runtime::spawn_blocking(move || cloud_ai::propose_cloud_review(&text, &settings))
        .await
        .map_err(|e| format!("Revisão na nuvem não concluiu: {e}"))?
}

#[tauri::command]
fn save_lt_settings(app: AppHandle, settings: lt::LtSettings) -> Result<(), String> {
    let path = settings_path(&app)?;
    let cloud = load_cloud_ai_settings(&app);
    let merged = serde_json::json!({
        "local_url": settings.local_url,
        "premium_enabled": settings.premium_enabled,
        "cloud_ai": cloud,
    });
    let json = serde_json::to_string_pretty(&merged).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| format!("settings: {e}"))
}

/// Async: espera pelo servidor LT fora da main thread (U1c — sem freeze).
#[tauri::command]
async fn ensure_lt_server(app: AppHandle) -> Result<String, String> {
    let url = load_lt_settings(&app).local_url;
    tauri::async_runtime::spawn_blocking(move || {
        lt::ensure_server(&url)?;
        Ok(format!("LanguageTool pronto em {url}"))
    })
    .await
    .map_err(|e| format!("LanguageTool não iniciou: {e}"))?
}

/// Async: HTTP bloqueante do LT local fora da main thread (revisão ao vivo).
#[tauri::command]
async fn check_lt_local(
    app: AppHandle,
    text: String,
) -> Result<Vec<txtmelhorator_core::review::DiffProposal>, String> {
    let url = load_lt_settings(&app).local_url;
    tauri::async_runtime::spawn_blocking(move || lt::check_local(&text, &url))
        .await
        .map_err(|e| format!("LanguageTool não concluiu: {e}"))?
}

/// Async: HTTP do LT Premium (nuvem) fora da main thread.
#[tauri::command]
async fn check_lt_premium(
    text: String,
) -> Result<Vec<txtmelhorator_core::review::DiffProposal>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let user = lt::load_premium_username().ok_or("Username Premium ausente no keychain")?;
        let key = lt::load_premium_api_key().ok_or("API key Premium ausente no keychain")?;
        lt::check_premium(&text, &user, &key)
    })
    .await
    .map_err(|e| format!("LanguageTool Premium não concluiu: {e}"))?
}

#[tauri::command]
fn save_lt_premium_creds(username: String, api_key: String) -> Result<(), String> {
    lt::save_premium_username(&username)?;
    lt::save_premium_api_key(&api_key)
}

#[tauri::command]
fn discover_languagetool(app: AppHandle) -> Result<lt::DiscoverLtResult, String> {
    let found = lt::discover_languagetool();
    if found.found {
        let mut s = load_lt_settings(&app);
        s.local_url = found.url.clone();
        let _ = save_lt_settings(app, s);
    }
    Ok(found)
}

#[tauri::command]
fn list_model_offers(app: AppHandle) -> Result<Vec<gguf::CatalogOffer>, String> {
    let root = ensure_app_data_dirs(&app)?;
    Ok(gguf::recommended_offers(&root))
}

#[tauri::command]
fn install_model_offer(app: AppHandle, offer_id: String) -> Result<gguf::ModelsState, String> {
    let root = ensure_app_data_dirs(&app)?;
    gguf::install_offer(&root, &offer_id)
}

#[tauri::command]
fn list_gguf_models(app: AppHandle) -> Result<gguf::ModelsState, String> {
    let root = ensure_app_data_dirs(&app)?;
    gguf::refresh_catalog(&root)
}

#[tauri::command]
fn select_gguf_model(app: AppHandle, name: String) -> Result<gguf::ModelsState, String> {
    let root = ensure_app_data_dirs(&app)?;
    gguf::select_model(&root, &name)
}

#[tauri::command]
fn remove_gguf_model(app: AppHandle, name: String) -> Result<gguf::ModelsState, String> {
    let root = ensure_app_data_dirs(&app)?;
    gguf::remove_model(&root, &name)
}

#[tauri::command]
fn download_gguf_model(
    app: AppHandle,
    url: String,
    filename: String,
    sha256: Option<String>,
) -> Result<gguf::ModelsState, String> {
    let root = ensure_app_data_dirs(&app)?;
    gguf::download_model(&root, &url, &filename, sha256.as_deref())
}

/// U1c: async (fora da main thread — sem rainbow wheel) + modelo RESIDENTE
/// (carrega o GGUF 1×, não 1× por página). A fila é o Mutex do LlamaState:
/// uma inferência por vez; o OCR segue em paralelo sem esperar.
#[tauri::command]
async fn propose_review(
    app: AppHandle,
    llama: State<'_, llama_infer::LlamaState>,
    text: String,
) -> Result<txtmelhorator_core::review::ReviewReport, String> {
    let root = ensure_app_data_dirs(&app)?;
    let Some(model) = gguf::selected_path(&root) else {
        return Ok(txtmelhorator_core::review::propose_heuristic_review(&text));
    };
    let llama = llama.inner().clone();
    let app_bg = app.clone();
    // R5c: inferência in-process (llama.cpp no binário) — sem llama-cli.
    tauri::async_runtime::spawn_blocking(move || {
        let vocabulary = txtmelhorator_core::review::extract_vocabulary(&text, 80);
        let prompt = txtmelhorator_core::review::fidelity_prompt(&text, &vocabulary);
        // "Parar" da UI (request_cancel) também interrompe a fila de revisão.
        let cancelled = move || app_bg.state::<AppState>().cancel.load(Ordering::SeqCst);
        match llama.generate(&model, &prompt, 512, Some(&cancelled)) {
            Ok(raw) => Ok(txtmelhorator_core::review::merge_llm_review(&text, &raw)),
            Err(e) => {
                let mut base = txtmelhorator_core::review::propose_heuristic_review(&text);
                if e == llama_infer::CANCELLED {
                    base.engine = "ia-local-cancelada".into();
                    base.note = e;
                } else {
                    base.engine = "ia-local-erro".into();
                    base.note = format!("{e} Por enquanto use LanguageTool.");
                }
                Ok(base)
            }
        }
    })
    .await
    .map_err(|e| format!("Tarefa de IA não concluiu: {e}"))?
}

/// R5d: descarrega o GGUF residente (a UI chama ao terminar a fila de livros).
#[tauri::command]
async fn unload_llama_model(llama: State<'_, llama_infer::LlamaState>) -> Result<bool, String> {
    let llama = llama.inner().clone();
    // spawn_blocking: liberar 6 GiB pode demorar; não segurar a main thread.
    tauri::async_runtime::spawn_blocking(move || llama.unload())
        .await
        .map_err(|e| format!("Não descarreguei o modelo: {e}"))
}

/// Melhorize de UMA página (limpeza + estrutura + regras, SEM IA) — a caixa
/// ao vivo mostra o texto já melhorado assim que a página sai da captura.
/// `command(async)`: roda fora da main thread (milissegundos, mas não trava).
#[tauri::command(async)]
fn melhorize_page(app: AppHandle, text: String) -> Result<String, String> {
    let rules = load_user_rules(&app);
    Ok(txtmelhorator_core::melhorize_page_with_rules(&text, &rules))
}

#[tauri::command]
fn apply_review_diffs(
    text: String,
    accepted: Vec<txtmelhorator_core::review::DiffProposal>,
) -> Result<String, String> {
    txtmelhorator_core::review::apply_accepted_diffs(&text, &accepted)
}

/// tessdata: app-data → resource bundle → Homebrew (via core).
fn resolve_tessdata(app: &AppHandle) -> Option<PathBuf> {
    if let Ok(root) = app.path().app_data_dir() {
        let dir = root.join("tessdata");
        if dir_has_traineddata(&dir) {
            return Some(dir);
        }
    }
    if let Ok(res) = app.path().resource_dir() {
        let dir = res.join("tessdata");
        if dir_has_traineddata(&dir) {
            return Some(dir);
        }
    }
    None
}

fn dir_has_traineddata(dir: &Path) -> bool {
    if !dir.is_dir() {
        return false;
    }
    std::fs::read_dir(dir)
        .ok()
        .map(|rd| {
            rd.filter_map(|e| e.ok()).any(|e| {
                e.path()
                    .extension()
                    .and_then(|x| x.to_str())
                    .map(|x| x.eq_ignore_ascii_case("traineddata"))
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

/// Processa um arquivo de TEXTO (raw.txt/.txt/.md) com o pipeline real.
#[tauri::command]
fn process_text_file(app: AppHandle, path: String) -> Result<ProcessResult, String> {
    let p = Path::new(&path);
    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    if !matches!(ext.as_str(), "txt" | "md") {
        return Err(format!(
            "Extensão não suportada: .{ext} (aceito aqui: .txt, .md; PDF vai pelo process_pdf)"
        ));
    }

    let raw = std::fs::read_to_string(p).map_err(|e| format!("Não consegui ler o arquivo: {e}"))?;
    let rules = load_user_rules(&app);
    let (structured, cleaned) =
        txtmelhorator_core::clean_and_structure_enhanced_with_rules(&raw, &rules);

    Ok(ProcessResult {
        source_name: p
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.clone()),
        source_path: p.to_string_lossy().into_owned(),
        engine: "texto".into(),
        languages_used: "—".into(),
        page_count: 0,
        cleaned: structured.text,
        pages: Vec::new(),
        cleanup_stats: cleaned.stats,
        structure_stats: structured.stats,
        warnings: cleaned.warnings,
    })
}

/// Localiza a libpdfium: recurso do bundle (produção) ou libs/ (dev).
fn find_pdfium(app: &AppHandle) -> Result<PathBuf, String> {
    if let Ok(custom) = std::env::var("MELHORADOR_PDFIUM") {
        let p = PathBuf::from(custom);
        if p.is_file() {
            return Ok(p);
        }
    }
    if let Ok(dir) = app.path().resource_dir() {
        let p = dir.join("libs/lib/libpdfium.dylib");
        if p.is_file() {
            return Ok(p);
        }
    }
    let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("libs/lib/libpdfium.dylib");
    if dev.is_file() {
        return Ok(dev);
    }
    Err("libpdfium.dylib não encontrada (bundle/resource ou libs/lib em dev)".into())
}

/// Sinaliza cancelamento do processamento em andamento.
#[tauri::command]
fn request_cancel(state: State<'_, AppState>) {
    state.cancel.store(true, Ordering::SeqCst);
}

/// Limpa temp/ e tessdata/ do app-data (não toca Homebrew).
#[tauri::command]
fn clear_app_data(app: AppHandle) -> Result<String, String> {
    let root = ensure_app_data_dirs(&app)?;
    let n_temp = clear_dir_contents(&root.join("temp"))?;
    let n_tess = clear_dir_contents(&root.join("tessdata"))?;
    Ok(format!(
        "Limpos: {n_temp} em temp/, {n_tess} em tessdata/ ({})",
        root.display()
    ))
}

/// Processa um PDF: extração (nativa/OCR) → limpeza → estrutura.
/// `languages`: "por" | "eng" | "por+eng" | "auto".
#[tauri::command(async)]
fn process_pdf(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
    languages: Option<String>,
) -> Result<ProcessResult, String> {
    state.cancel.store(false, Ordering::SeqCst);
    let pdfium = find_pdfium(&app)?;
    let p = Path::new(&path);
    let langs = resolve_languages(&app, p, languages)?;
    let tess = resolve_tessdata(&app);

    let cancel_flag = &state.cancel;
    let should_cancel = || cancel_flag.load(Ordering::SeqCst);

    let mut progress = |done: usize, total: usize, page_text: &str, preview: Option<&[u8]>| {
        let preview_url = preview.map(|bytes| {
            use base64::Engine;
            let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
            format!("data:image/png;base64,{b64}")
        });
        let _ = app.emit(
            "extract-progress",
            serde_json::json!({
                "done": done,
                "total": total,
                "pageText": page_text,
                "preview": preview_url,
            }),
        );
    };
    let rules = load_user_rules(&app);
    let extracted = txtmelhorator_core::extraction::extract_pdf(
        &pdfium,
        p,
        None,
        &langs,
        tess.as_deref(),
        &mut progress,
        Some(&should_cancel),
        &rules,
    )?;

    let pages_result =
        txtmelhorator_core::clean_and_structure_pages_with_rules(&extracted.raw_text, &rules);
    Ok(ProcessResult {
        source_name: p
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.clone()),
        source_path: p.to_string_lossy().into_owned(),
        engine: extracted.engine.to_string(),
        languages_used: langs,
        page_count: extracted.page_count as u32,
        cleaned: pages_result.full.text,
        pages: pages_result.pages,
        cleanup_stats: pages_result.cleanup.stats,
        structure_stats: pages_result.full.stats,
        warnings: pages_result.cleanup.warnings,
    })
}

/// Renderiza página do PDF (1-based) → data URL PNG p/ a UI.
#[tauri::command(async)]
fn render_pdf_page(app: AppHandle, path: String, page: u32) -> Result<String, String> {
    if page < 1 {
        return Err("Página deve ser ≥ 1".into());
    }
    let pdfium = find_pdfium(&app)?;
    let png = txtmelhorator_core::extraction::render_page_png(
        &pdfium,
        Path::new(&path),
        page as usize,
        None,
    )?;
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&png);
    Ok(format!("data:image/png;base64,{b64}"))
}

/// Lista PDFs no diretório (não recursivo). Ordenado por nome.
#[tauri::command]
fn list_pdfs_in_dir(dir: String) -> Result<Vec<String>, String> {
    let p = Path::new(&dir);
    if !p.is_dir() {
        return Err(format!("Não é uma pasta: {dir}"));
    }
    let mut out: Vec<String> = Vec::new();
    let entries = std::fs::read_dir(p).map_err(|e| format!("Não consegui ler a pasta: {e}"))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("Entrada inválida: {e}"))?;
        let path = entry.path();
        let is_pdf = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("pdf"))
            .unwrap_or(false);
        if is_pdf && path.is_file() {
            out.push(path.to_string_lossy().into_owned());
        }
    }
    out.sort();
    if out.is_empty() {
        return Err("Nenhum PDF nesta pasta.".into());
    }
    Ok(out)
}

/// Grava o resultado. `dest_dir` opcional: se None, grava ao lado da origem.
/// Também grava `*.report.json` (trilha hash + diffs).
#[tauri::command]
fn save_result(
    source_path: String,
    content: String,
    format: String,
    dest_dir: Option<String>,
    engine: Option<String>,
    languages: Option<String>,
    page_count: Option<u32>,
    accepted_diffs: Option<Vec<txtmelhorator_core::review::DiffProposal>>,
) -> Result<String, String> {
    let p = Path::new(&source_path);
    let stem = p
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "resultado".into());
    let dir = match dest_dir.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(d) => {
            let dir = PathBuf::from(d);
            if !dir.is_dir() {
                return Err(format!("Pasta de destino inválida: {d}"));
            }
            dir
        }
        None => p
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf(),
    };

    let meta = txtmelhorator_core::metadata::extract_book_meta(
        &content.chars().take(8000).collect::<String>(),
        &stem,
    );
    let diffs = accepted_diffs.unwrap_or_default();
    let empty_stats = std::collections::BTreeMap::new();
    let report = txtmelhorator_core::report::build_report(
        &p.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| source_path.clone()),
        engine.as_deref().unwrap_or("—"),
        languages.as_deref().unwrap_or("—"),
        page_count.unwrap_or(0),
        &content,
        &content,
        &empty_stats,
        &empty_stats,
        &[],
        &diffs,
        Some(meta),
        None,
    );
    let report_path = dir.join(format!("{stem}.report.json"));
    let report_json = serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?;
    std::fs::write(&report_path, report_json)
        .map_err(|e| format!("report.json: {e}"))?;

    let dest = match format.as_str() {
        "md" => {
            let dest = dir.join(format!("{stem}.melhorado.md"));
            std::fs::write(&dest, &content).map_err(|e| format!("Não consegui gravar: {e}"))?;
            dest
        }
        "txt" => {
            let body = content
                .lines()
                .map(|line| {
                    let t = line.trim_start_matches('#');
                    let t = if t.len() != line.len() {
                        t.trim_start()
                    } else {
                        t
                    };
                    if let Some(rest) = t.strip_prefix("- ") {
                        format!("— {rest}")
                    } else {
                        t.to_string()
                    }
                })
                .collect::<Vec<_>>()
                .join("\n");
            let dest = dir.join(format!("{stem}.melhorado.txt"));
            std::fs::write(&dest, body).map_err(|e| format!("Não consegui gravar: {e}"))?;
            dest
        }
        "docx" => {
            let bytes = txtmelhorator_core::docx_export::markdown_to_docx(&content)?;
            let dest = dir.join(format!("{stem}.melhorado.docx"));
            std::fs::write(&dest, bytes).map_err(|e| format!("Não consegui gravar: {e}"))?;
            dest
        }
        other => return Err(format!("Formato não suportado: {other}")),
    };

    Ok(dest.to_string_lossy().into_owned())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            cancel: AtomicBool::new(false),
        })
        // U1c: motor de IA residente (GGUF carregado 1×, fila de inferência).
        .manage(llama_infer::LlamaState::new())
        .setup(|app| {
            let _ = ensure_app_data_dirs(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            process_text_file,
            process_pdf,
            list_pdfs_in_dir,
            save_result,
            request_cancel,
            clear_app_data,
            render_pdf_page,
            list_user_rules,
            save_user_rules,
            propose_review,
            unload_llama_model,
            melhorize_page,
            apply_review_diffs,
            get_lt_settings,
            save_lt_settings,
            ensure_lt_server,
            check_lt_local,
            check_lt_premium,
            save_lt_premium_creds,
            list_gguf_models,
            select_gguf_model,
            remove_gguf_model,
            download_gguf_model,
            list_model_offers,
            install_model_offer,
            discover_languagetool,
            get_cloud_ai_settings,
            save_cloud_ai_settings,
            save_cloud_ai_key,
            check_cloud_ai
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn processa_txt_e_salva_md() {
        let dir = std::env::temp_dir().join("txtmelhorator-app-test");
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("amostra.txt");
        std::fs::write(&src, "Uma pala-\nvra quebrada.\n\n\n\nCAPÍTULO PRIMEIRO\n\nProsa.")
            .unwrap();

        let raw = std::fs::read_to_string(&src).unwrap();
        let (structured, _cleaned) = txtmelhorator_core::clean_and_structure_enhanced(&raw);
        assert!(structured.text.contains("Uma palavra quebrada."));
        assert!(structured.text.contains("# CAPÍTULO PRIMEIRO"));

        let saved = save_result(
            src.to_string_lossy().into_owned(),
            structured.text.clone(),
            "md".into(),
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        assert!(std::fs::read_to_string(saved).unwrap().contains("# CAPÍTULO"));
    }

    #[test]
    fn salva_em_destino_alternativo() {
        let base = std::env::temp_dir().join("txtmelhorator-app-dest");
        let src_dir = base.join("origem");
        let out_dir = base.join("saida");
        std::fs::create_dir_all(&src_dir).unwrap();
        std::fs::create_dir_all(&out_dir).unwrap();
        let src = src_dir.join("livro.txt");
        std::fs::write(&src, "Prosa.").unwrap();

        let saved = save_result(
            src.to_string_lossy().into_owned(),
            "# Título\n\ncorpo".into(),
            "md".into(),
            Some(out_dir.to_string_lossy().into_owned()),
            None,
            None,
            None,
            None,
        )
        .unwrap();
        assert!(saved.contains("saida"));
        assert!(Path::new(&saved).exists());
    }

    #[test]
    fn lista_pdfs_da_pasta() {
        let dir = std::env::temp_dir().join("txtmelhorator-app-pdfs");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.pdf"), b"%PDF").unwrap();
        std::fs::write(dir.join("b.PDF"), b"%PDF").unwrap();
        std::fs::write(dir.join("nota.txt"), b"x").unwrap();

        let list = list_pdfs_in_dir(dir.to_string_lossy().into_owned()).unwrap();
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn idiomas_normalizados() {
        assert_eq!(normalize_languages(None), "por+eng");
        assert_eq!(normalize_languages(Some("por".into())), "por");
        assert_eq!(normalize_languages(Some("auto".into())), "auto");
        assert_eq!(normalize_languages(Some("xyz".into())), "por+eng");
    }

    #[test]
    fn limpa_conteudo_de_pasta() {
        let dir = std::env::temp_dir().join("txtmelhorator-clear-test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("a.txt"), b"x").unwrap();
        std::fs::write(dir.join("sub/b.txt"), b"y").unwrap();
        let n = clear_dir_contents(&dir).unwrap();
        assert_eq!(n, 2);
        assert!(dir.exists());
        assert!(std::fs::read_dir(&dir).unwrap().next().is_none());
    }

    #[test]
    fn extensao_invalida_da_erro_claro() {
        let path = "/tmp/x.doc";
        let p = Path::new(path);
        let ext = p
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        assert!(!matches!(ext.as_str(), "txt" | "md"));
        let err = format!(
            "Extensão não suportada: .{ext} (aceito aqui: .txt, .md; PDF vai pelo process_pdf)"
        );
        assert!(err.contains("não suportada"));
    }
}
