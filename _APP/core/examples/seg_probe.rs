// ==============================================================================
// SCRIPT: seg_probe.rs (exemplo melhorador-core)
// DESCRIÇÃO: Inspeciona segmentos PDFium (texto+caixa) p/ depurar remontagem
// CHAMADO POR: cargo run --release --example seg_probe -- <pdf> <pagina> <filtro>
// CONTRATO (RESPOSTA ESPERADA): lista de segmentos casando o filtro no stderr
// ==============================================================================

use pdfium_render::prelude::*;
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (pdf, pagina, filtro) = (
        PathBuf::from(&args[1]),
        args[2].parse::<usize>().unwrap(),
        args[3].clone(),
    );
    let lib = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../src-tauri/libs/lib/libpdfium.dylib");
    let pdfium = Pdfium::new(Pdfium::bind_to_library(lib).unwrap());
    let doc = pdfium.load_pdf_from_file(&pdf, None).unwrap();
    let page = doc.pages().get((pagina - 1) as u16).unwrap();
    let tp = page.text().unwrap();
    let mut prev: Option<(String, f32, f32)> = None;
    for seg in tp.segments().iter() {
        let t = seg.text();
        let b = seg.bounds();
        if t.contains(&filtro) || prev.as_ref().map_or(false, |(pt, _, _)| pt.contains(&filtro)) {
            eprintln!(
                "seg {:?} left={:.1} right={:.1} top={:.1} bottom={:.1}",
                t, b.left().value, b.right().value, b.top().value, b.bottom().value
            );
        }
        prev = Some((t, b.left().value, b.right().value));
    }
}
