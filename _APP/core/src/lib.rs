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

pub mod cleanup;
pub mod extraction;
pub mod pydifflib;
pub mod pystr;
pub mod structure;

/// Pipeline completo pós-extração: limpeza + estrutura Markdown.
/// Equivale a `apply_structure(clean_text(raw).text)` do CLI Python.
pub fn clean_and_structure(raw_text: &str) -> (structure::StructureResult, cleanup::CleanupResult) {
    let cleaned = cleanup::clean_text(raw_text, true, 0);
    let structured = structure::apply_structure(&cleaned.text);
    (structured, cleaned)
}

/// Pipeline no modo APRIMORADO do app (heurísticas além do CLI, documentadas
/// nos módulos): cabeçalhos de PDFs nativos, guarda de rodapé, listas.
pub fn clean_and_structure_enhanced(
    raw_text: &str,
) -> (structure::StructureResult, cleanup::CleanupResult) {
    let cleaned = cleanup::clean_text_enhanced(raw_text, true, 0);
    let structured = structure::apply_structure_enhanced(&cleaned.text);
    (structured, cleaned)
}
