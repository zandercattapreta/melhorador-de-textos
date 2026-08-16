// ==============================================================================
// SCRIPT: extraction_real.rs (melhorador-core, teste de integração)
// DESCRIÇÃO: QA de conteúdo do OCR embutido contra um livro real do acervo
// CHAMADO POR: cargo test --release (pula se _originais/ não existir)
// CONTRATO (RESPOSTA ESPERADA): OCR reconhece o conteúdo esperado da amostra
// ==============================================================================

//! Sem paridade byte a byte aqui (engine de OCR diferente do ocrmypdf):
//! o critério é reconhecer o conteúdo. Amostra: 3 páginas do menor livro.

use std::path::PathBuf;

use melhorador_core::extraction::extract_pdf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn ocr_reconhece_paginas_do_livro_real() {
    let pdf = root().join("_originais/Pierre Leveque - As Primeiras Civilizações da Idade da Pedra aos Povos Semitas.pdf");
    let pdfium = root().join("_APP/src-tauri/libs/lib/libpdfium.dylib");
    if !pdf.is_file() || !pdfium.is_file() {
        eprintln!("[extraction] acervo/pdfium ausentes — teste pulado");
        return;
    }

    let mut calls = 0usize;
    let result = extract_pdf(
        &pdfium,
        &pdf,
        Some((21, 23)),
        "por+eng",
        None,
        &mut |done, total, page_text| {
            calls += 1;
            assert!(done <= total);
            assert!(!page_text.is_empty(), "texto parcial da página deve vir no callback");
        },
    )
    .expect("extração falhou");

    // Livro escaneado: precisa ter caído no OCR e produzido texto real.
    assert_eq!(result.engine, "ocr");
    assert_eq!(result.page_count, 3);
    assert_eq!(calls, 3, "callback de progresso por página");
    assert!(
        result.ocr_chars > 1000,
        "OCR de 3 páginas devia render >1000 chars, veio {}",
        result.ocr_chars
    );
    // Separador de páginas no contrato do sidecar (o cleanup depende dele).
    assert_eq!(result.raw_text.matches('\u{0c}').count(), 2);

    // Conteúdo: palavras confirmadas nas páginas 21-23 do golden do CLI
    // (raw.txt do ocrmypdf): "civilizações", "poder", "camponeses", "estado".
    eprintln!("[extraction] amostra OCR: {:?}", &result.raw_text.chars().take(200).collect::<String>());
    let lower = result.raw_text.to_lowercase();
    let hits = ["civiliza", "poder", "campones", "estado"]
        .iter()
        .filter(|w| lower.contains(*w))
        .count();
    assert!(hits >= 3, "esperava >=3 termos do livro na amostra, achei {hits}");
}
