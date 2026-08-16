// ==============================================================================
// SCRIPT: golden.rs (txtmelhorator-core, teste de integração)
// DESCRIÇÃO: Paridade byte a byte com o CLI Python nos 4 livros reais
// CHAMADO POR: cargo test (lê _temp/goldens/, gerado por make_goldens.py)
// CONTRATO (RESPOSTA ESPERADA): saída Rust == golden Python para todo livro
// ==============================================================================

//! Gabaritos são local-only (conteúdo de livro não entra no git); quando
//! ausentes, o teste passa com aviso — a paridade só é exigida onde o corpus
//! existe (a máquina de referência).

use std::fs;
use std::path::PathBuf;

use txtmelhorator_core::{cleanup, structure};

/// _APP/core → raiz do projeto (2 níveis acima).
fn goldens_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../_temp/goldens")
}

/// Mostra contexto do primeiro byte divergente para depuração dirigida.
fn first_divergence(label: &str, got: &str, want: &str) -> String {
    let gb = got.as_bytes();
    let wb = want.as_bytes();
    let n = gb.len().min(wb.len());
    let mut pos = n;
    for i in 0..n {
        if gb[i] != wb[i] {
            pos = i;
            break;
        }
    }
    if pos == n && gb.len() == wb.len() {
        return format!("{label}: idênticos (?)");
    }
    let start = pos.saturating_sub(80);
    // Alinha nos limites de char p/ slice seguro.
    let mut s = start;
    while s > 0 && !got.is_char_boundary(s) {
        s -= 1;
    }
    let ge = (pos + 80).min(gb.len());
    let we = (pos + 80).min(wb.len());
    let mut ge2 = ge;
    while ge2 < gb.len() && !got.is_char_boundary(ge2) {
        ge2 += 1;
    }
    let mut we2 = we;
    while we2 < wb.len() && !want.is_char_boundary(we2) {
        we2 += 1;
    }
    let mut s2 = s;
    while s2 > 0 && !want.is_char_boundary(s2) {
        s2 -= 1;
    }
    format!(
        "{label}: divergência no byte {pos} (got len {} / want len {})\n  got : {:?}\n  want: {:?}",
        gb.len(),
        wb.len(),
        &got[s..ge2.min(gb.len())],
        &want[s2..we2.min(wb.len())],
    )
}

#[test]
fn paridade_com_goldens_python() {
    let dir = goldens_dir();
    if !dir.is_dir() {
        eprintln!("[golden] _temp/goldens ausente — rode make_goldens.py; teste pulado");
        return;
    }

    let mut books = 0usize;
    let mut failures: Vec<String> = Vec::new();

    let mut entries: Vec<_> = fs::read_dir(&dir).unwrap().flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let book_dir = entry.path();
        let raw_path = book_dir.join("raw.txt");
        if !raw_path.is_file() {
            continue;
        }
        let slug = entry.file_name().to_string_lossy().to_string();
        // GOLDEN_BOOK=<trecho-do-slug> restringe a um livro (depuração dirigida).
        if let Ok(filter) = std::env::var("GOLDEN_BOOK") {
            if !slug.contains(&filter) {
                continue;
            }
        }
        books += 1;
        let raw = fs::read_to_string(&raw_path).unwrap();
        let clean_want = fs::read_to_string(book_dir.join("clean_only.txt")).unwrap();
        let final_want = fs::read_to_string(book_dir.join("cleaned_golden.md")).unwrap();

        // Etapa 1: limpeza (defaults do batch).
        let cleaned = cleanup::clean_text(&raw, true, 0);
        if cleaned.text != clean_want {
            failures.push(first_divergence(&format!("{slug} [clean]"), &cleaned.text, &clean_want));
            continue; // estrutura dependeria de texto igual
        }

        // Etapa 2: estrutura sobre o texto limpo.
        let structured = structure::apply_structure(&cleaned.text);
        if structured.text != final_want {
            failures.push(first_divergence(
                &format!("{slug} [structure]"),
                &structured.text,
                &final_want,
            ));
        }
    }

    assert!(books > 0, "goldens dir existe mas sem livros");
    assert!(
        failures.is_empty(),
        "paridade falhou em {}/{} livros:\n{}",
        failures.len(),
        books,
        failures.join("\n\n")
    );
    println!("[golden] paridade byte a byte OK em {books} livro(s)");
}
