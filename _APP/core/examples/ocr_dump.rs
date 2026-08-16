// ==============================================================================
// SCRIPT: ocr_dump.rs (exemplo melhorador-core)
// DESCRIÇÃO: OCR de uma faixa de páginas p/ medir qualidade vs gabarito CLI
// CHAMADO POR: cargo run --release --example ocr_dump -- <pdf> <ini> <fim> <saida>
// CONTRATO (RESPOSTA ESPERADA): grava raw com \f entre páginas; exit 0
// ==============================================================================

use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (pdf, start, end, out) = (
        PathBuf::from(&args[1]),
        args[2].parse::<usize>().unwrap(),
        args[3].parse::<usize>().unwrap(),
        PathBuf::from(&args[4]),
    );
    let pdfium = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../src-tauri/libs/lib/libpdfium.dylib");

    let result = melhorador_core::extraction::extract_pdf(
        &pdfium,
        &pdf,
        Some((start, end)),
        "por+eng",
        None,
        &mut |done, total, _| eprintln!("[ocr] {done}/{total}"),
    )
    .expect("extração falhou");

    std::fs::write(&out, &result.raw_text).unwrap();
    eprintln!(
        "[ocr] engine={} chars={} -> {}",
        result.engine,
        result.raw_text.chars().count(),
        out.display()
    );
}
