// ==============================================================================
// SCRIPT: lib.rs (melhorador-core)
// DESCRIÇÃO: Raiz do crate — expõe os módulos do pipeline determinístico
// CHAMADO POR: src-tauri (comandos do app) e testes de paridade
// CONTRATO (RESPOSTA ESPERADA): mesmas saídas do CLI Python (_CLI/), byte a byte
// ==============================================================================

//! Port Rust do pipeline do Melhorador de Textos.
//!
//! Regra de ouro (herdada do projeto): só estabilizar e reformatar — nunca
//! completar, adivinhar ou reescrever o conteúdo do livro. Tudo determinístico.
//! A implementação de referência é o CLI Python em `_CLI/`; cada função
//! portada é validada contra as saídas dele (golden masters em tests/).

pub mod blocks;
pub mod cleanup;
pub mod docx_export;
pub mod extraction;
pub mod metadata;
pub mod preprocess;
pub mod pydifflib;
pub mod pystr;
pub mod report;
pub mod review;
pub mod rules;
pub mod structure;
pub mod typos;

/// Pipeline completo pós-extração: limpeza + estrutura Markdown.
/// Equivale a `apply_structure(clean_text(raw).text)` do CLI Python.
pub fn clean_and_structure(raw_text: &str) -> (structure::StructureResult, cleanup::CleanupResult) {
    let cleaned = cleanup::clean_text(raw_text, true, 0);
    let structured = structure::apply_structure(&cleaned.text);
    (structured, cleaned)
}

/// Pipeline no modo APRIMORADO do app (heurísticas além do CLI, documentadas
/// nos módulos): cabeçalhos de PDFs nativos, guarda de rodapé, listas, blocos.
pub fn clean_and_structure_enhanced(
    raw_text: &str,
) -> (structure::StructureResult, cleanup::CleanupResult) {
    clean_and_structure_enhanced_with_rules(raw_text, &[])
}

/// Enhanced + regras do usuário (R4) aplicadas **antes** da estrutura.
pub fn clean_and_structure_enhanced_with_rules(
    raw_text: &str,
    user_rules: &[rules::UserRule],
) -> (structure::StructureResult, cleanup::CleanupResult) {
    let cleaned = cleanup::clean_text_enhanced(raw_text, true, 0);
    let (de_typo, _) = typos::apply_ocr_typos(&cleaned.text);
    let after_rules = rules::apply_user_rules(&de_typo, user_rules);
    let mut structured = structure::apply_structure_enhanced(&after_rules);
    let (annotated, block_stats) = blocks::annotate_blocks(&structured.text);
    structured.text = annotated;
    for (k, v) in block_stats {
        structured.stats.insert(k, v);
    }
    (structured, cleaned)
}

/// Resultado do pipeline fatiado por página (conferência R3b).
pub struct PagesResult {
    /// Texto de cada página (1:1 com `\f` da extração).
    pub pages: Vec<String>,
    /// Livro inteiro (enhanced) — export.
    pub full: structure::StructureResult,
    pub cleanup: cleanup::CleanupResult,
}

/// Limpa/estrutura **cada** fatia `\f` à parte (sync página ↔ texto).
pub fn clean_and_structure_pages(raw_text: &str) -> PagesResult {
    clean_and_structure_pages_with_rules(raw_text, &[])
}

pub fn clean_and_structure_pages_with_rules(
    raw_text: &str,
    user_rules: &[rules::UserRule],
) -> PagesResult {
    let slices: Vec<&str> = if raw_text.contains('\u{0c}') {
        raw_text.split('\u{0c}').collect()
    } else {
        vec![raw_text]
    };
    let mut pages = Vec::with_capacity(slices.len());
    for slice in slices {
        let cleaned = cleanup::clean_text_enhanced(slice, true, 0);
        let (de_typo, _) = typos::apply_ocr_typos(&cleaned.text);
        let after_rules = rules::apply_user_rules(&de_typo, user_rules);
        let mut structured = structure::apply_structure_enhanced(&after_rules);
        let (annotated, _) = blocks::annotate_blocks(&structured.text);
        structured.text = annotated;
        // Conferência: nunca deixar a página “muda”. Se só sobrou [figura]/vazio,
        // mostra o bruto da fatia ou um aviso claro em PT-BR.
        let page_out = conference_page_text(&structured.text, slice);
        pages.push(page_out);
    }
    let (full, cleanup) = clean_and_structure_enhanced_with_rules(raw_text, user_rules);
    PagesResult {
        pages,
        full,
        cleanup,
    }
}

/// Texto da página para o painel de conferência (humano, sempre legível).
fn conference_page_text(structured: &str, raw_slice: &str) -> String {
    let t = structured.trim();
    let raw = raw_slice.trim();
    let only_figura = t.is_empty() || t == "[figura]" || t.starts_with("[figura]");
    if !only_figura {
        return structured.to_string();
    }
    // Ainda há algo no bruto (OCR/nativo antes da limpeza)?
    let raw_alnum = raw.chars().filter(|c| c.is_alphanumeric()).count();
    if raw_alnum >= 8 {
        return format!(
            "(Texto bruto desta página — a limpeza removeu quase tudo)\n\n{raw}"
        );
    }
    "(Esta página não tem texto capturado — no original é só imagem ou está quase vazia.)"
        .to_string()
}

#[cfg(test)]
mod pages_tests {
    use super::*;

    #[test]
    fn fatias_respeitam_form_feed() {
        let raw = "Página um com texto suficiente aqui para não sumir no chrome.\n\nSegundo parágrafo da página um com conteúdo.\u{0c}Página dois com mais texto aqui e ainda outro parágrafo longo o bastante.\n\nContinuação da página dois.\u{0c}Página três final com corpo suficiente para ficar no resultado após limpeza.";
        let r = clean_and_structure_pages(raw);
        assert_eq!(r.pages.len(), 3);
        assert!(!r.pages[0].is_empty());
        assert!(!r.full.text.is_empty());
    }
}
