// ==============================================================================
// SCRIPT: pipeline_dump.rs (exemplo melhorador-core)
// DESCRIÇÃO: Extração + pipeline completo com relatório p/ QA dirigido
// CHAMADO POR: cargo run --release --example pipeline_dump -- <pdf> [ini fim]
// CONTRATO (RESPOSTA ESPERADA): stats + amostras no stderr; texto no stdout
// ==============================================================================

use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let pdf = PathBuf::from(&args[1]);
    let pages = if args.len() >= 4 {
        Some((args[2].parse().unwrap(), args[3].parse().unwrap()))
    } else {
        None
    };
    let pdfium = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../src-tauri/libs/lib/libpdfium.dylib");

    let ex = melhorador_core::extraction::extract_pdf(
        &pdfium, &pdf, pages, "por+eng", None, &mut |d, t, _| {
            if d % 25 == 0 { eprintln!("[extract] {d}/{t}") }
        },
    )
    .expect("extração falhou");
    eprintln!("[pipeline] engine={} paginas={}", ex.engine, ex.page_count);

    let (structured, cleaned) = melhorador_core::clean_and_structure_enhanced(&ex.raw_text);
    eprintln!("[pipeline] cleanup: {:?}", cleaned.stats);
    eprintln!("[pipeline] structure: {:?}", structured.stats);

    // Amostra: todos os headings detectados (até 40) e 5 parágrafos do meio.
    let mut headings = 0;
    for block in structured.text.split("\n\n") {
        if block.starts_with('#') && headings < 40 {
            eprintln!("[heading] {}", block.chars().take(90).collect::<String>());
            headings += 1;
        }
    }
    let paras: Vec<&str> = structured.text.split("\n\n").collect();
    let mid = paras.len() / 2;
    for p in &paras[mid..(mid + 5).min(paras.len())] {
        eprintln!("[amostra-meio] {}", p.chars().take(160).collect::<String>());
    }
    println!("{}", structured.text);
}
