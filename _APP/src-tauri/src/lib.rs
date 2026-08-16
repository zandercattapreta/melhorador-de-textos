// ==============================================================================
// SCRIPT: lib.rs (melhorador-app / src-tauri)
// DESCRIÇÃO: Comandos Tauri — ponte entre a UI e o core Rust do pipeline
// CHAMADO POR: UI React via invoke(); main.rs
// CONTRATO (RESPOSTA ESPERADA): ProcessResult { cleaned, stats } ou erro legível
// ==============================================================================

use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

/// Resultado do processamento para a UI.
#[derive(Serialize, Debug)]
pub struct ProcessResult {
    /// Nome do arquivo de origem (exibição).
    pub source_name: String,
    /// Motor de extração: "texto" (arquivo já textual), "native" ou "ocr".
    pub engine: String,
    /// Texto limpo e estruturado (Markdown).
    pub cleaned: String,
    /// Estatísticas de limpeza (chave → valor), p/ painel de avisos.
    pub cleanup_stats: std::collections::BTreeMap<&'static str, i64>,
    /// Estatísticas de estrutura (h1..h4, sumário, prosa).
    pub structure_stats: std::collections::BTreeMap<&'static str, i64>,
    /// Avisos do pipeline (ex.: caracteres ilegíveis remanescentes).
    pub warnings: Vec<String>,
}

/// Processa um arquivo de TEXTO (raw.txt/.txt/.md) com o pipeline real.
/// PDFs escaneados exigem o OCR embutido (fase E3) — erro claro por ora.
#[tauri::command]
fn process_text_file(path: String) -> Result<ProcessResult, String> {
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
    let (structured, cleaned) = melhorador_core::clean_and_structure_enhanced(&raw);

    Ok(ProcessResult {
        source_name: p
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.clone()),
        engine: "texto".into(),
        cleaned: structured.text,
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

/// Processa um PDF com o pipeline completo: extração (nativa/OCR) →
/// limpeza → estrutura. Emite "extract-progress" (feitas, total) no OCR.
/// `async`: OCR de livro inteiro é longo; roda fora da thread da UI.
#[tauri::command(async)]
fn process_pdf(app: AppHandle, path: String) -> Result<ProcessResult, String> {
    let pdfium = find_pdfium(&app)?;
    let p = Path::new(&path);

    // Progresso parcial: contador + texto bruto da página recém-lida, para a
    // UI mostrar o trabalho acontecendo em vez de uma espera muda.
    let mut progress = |done: usize, total: usize, page_text: &str| {
        let _ = app.emit(
            "extract-progress",
            serde_json::json!({ "done": done, "total": total, "pageText": page_text }),
        );
    };
    let extracted = melhorador_core::extraction::extract_pdf(
        &pdfium,
        p,
        None, // livro inteiro; faixa/fila chegam no épico E2
        "por+eng",
        None,
        &mut progress,
    )?;

    let (structured, cleaned) = melhorador_core::clean_and_structure_enhanced(&extracted.raw_text);
    Ok(ProcessResult {
        source_name: p
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.clone()),
        engine: extracted.engine.to_string(),
        cleaned: structured.text,
        cleanup_stats: cleaned.stats,
        structure_stats: structured.stats,
        warnings: cleaned.warnings,
    })
}

/// Grava o resultado ao lado do arquivo de origem (`<nome>.melhorado.md|txt`).
/// Formato txt: remove marcação de heading/lista de forma determinística.
#[tauri::command]
fn save_result(source_path: String, content: String, format: String) -> Result<String, String> {
    let p = Path::new(&source_path);
    let stem = p
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "resultado".into());
    let dir = p.parent().unwrap_or_else(|| Path::new("."));

    let (out_name, body) = match format.as_str() {
        "md" => (format!("{stem}.melhorado.md"), content),
        "txt" => {
            // Strip determinístico: '#'+espaço vira vazio; "- " de sumário vira travessão.
            let body = content
                .lines()
                .map(|line| {
                    let t = line.trim_start_matches('#');
                    let t = if t.len() != line.len() { t.trim_start() } else { t };
                    if let Some(rest) = t.strip_prefix("- ") {
                        format!("— {rest}")
                    } else {
                        t.to_string()
                    }
                })
                .collect::<Vec<_>>()
                .join("\n");
            (format!("{stem}.melhorado.txt"), body)
        }
        other => return Err(format!("Formato não suportado: {other}")),
    };

    let dest = dir.join(out_name);
    std::fs::write(&dest, body).map_err(|e| format!("Não consegui gravar: {e}"))?;
    Ok(dest.to_string_lossy().into_owned())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            process_text_file,
            process_pdf,
            save_result
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    // Prova de máquina do comando (sem UI): o core roda de verdade aqui.
    #[test]
    fn processa_txt_e_salva_md() {
        let dir = std::env::temp_dir().join("melhorador-app-test");
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("amostra.txt");
        std::fs::write(&src, "Uma pala-\nvra quebrada.\n\n\n\nCAPÍTULO PRIMEIRO\n\nProsa.").unwrap();

        let result = process_text_file(src.to_string_lossy().into_owned()).unwrap();
        assert!(result.cleaned.contains("Uma palavra quebrada."));
        assert!(result.cleaned.contains("# CAPÍTULO PRIMEIRO"));

        let saved = save_result(
            src.to_string_lossy().into_owned(),
            result.cleaned.clone(),
            "md".into(),
        )
        .unwrap();
        assert!(std::fs::read_to_string(saved).unwrap().contains("# CAPÍTULO"));
    }

    #[test]
    fn extensao_invalida_da_erro_claro() {
        let err = process_text_file("/tmp/x.doc".into()).unwrap_err();
        assert!(err.contains("não suportada"));
    }
}
