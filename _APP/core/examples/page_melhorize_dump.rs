// ==============================================================================
// SCRIPT: page_melhorize_dump.rs (exemplo txtmelhorator-core)
// DESCRIÇÃO: Reproduz o caminho AO VIVO do app: extrai páginas e passa cada
//            fatia por melhorize_page_with_rules (mesma função do comando
//            melhorize_page). QA dirigido do texto que aparece na caixa.
// CHAMADO POR: cargo run --release --example page_melhorize_dump -- <pdf> <ini> <fim>
// CONTRATO (RESPOSTA ESPERADA): por página: bruto (primeiras linhas) e
//            melhorizado completo no stdout
// ==============================================================================

use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let pdf = PathBuf::from(&args[1]);
    let ini: usize = args[2].parse().unwrap();
    let fim: usize = args[3].parse().unwrap();
    let pdfium = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../src-tauri/libs/lib/libpdfium.dylib");

    // Mesmo caminho do app: extract_pdf emite o texto POR PÁGINA no callback —
    // é exatamente essa fatia que a UI manda para melhorize_page.
    let mut chunks: Vec<(usize, String)> = Vec::new();
    let _ = txtmelhorator_core::extraction::extract_pdf(
        &pdfium,
        &pdf,
        Some((ini, fim)),
        "por+eng",
        None,
        &mut |done, _t, page_text, _| {
            chunks.push((done, page_text.to_string()));
        },
        None,
        &[],
    )
    .expect("extração falhou");

    for (page, raw) in &chunks {
        let out = txtmelhorator_core::melhorize_page_with_rules(raw, &[]);
        println!("===== página {page} — BRUTO ({} chars) =====", raw.len());
        for l in raw.lines().take(8) {
            println!("| {l}");
        }
        println!("----- página {page} — MELHORIZADO ({} chars) -----", out.len());
        println!("{out}");
        println!();
    }
}
