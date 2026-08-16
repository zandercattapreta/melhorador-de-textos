// ==============================================================================
// SCRIPT: docx_export.rs (txtmelhorator-core)
// DESCRIÇÃO: Markdown simples → .docx com estilos Heading/Normal (A9)
// CHAMADO POR: comando Tauri save_result format=docx
// CONTRATO (RESPOSTA ESPERADA): bytes OOXML; sem inventar conteúdo
// ==============================================================================

use docx_rs::*;

/// Converte Markdown leve (# ## ### + parágrafos) em DOCX.
pub fn markdown_to_docx(md: &str) -> Result<Vec<u8>, String> {
    let mut doc = Docx::new();
    for block in md.split("\n\n") {
        let line = block.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix("#### ") {
            doc = doc.add_paragraph(heading_para(rest, 4));
        } else if let Some(rest) = line.strip_prefix("### ") {
            doc = doc.add_paragraph(heading_para(rest, 3));
        } else if let Some(rest) = line.strip_prefix("## ") {
            doc = doc.add_paragraph(heading_para(rest, 2));
        } else if let Some(rest) = line.strip_prefix("# ") {
            doc = doc.add_paragraph(heading_para(rest, 1));
        } else {
            for l in line.lines() {
                doc = doc.add_paragraph(
                    Paragraph::new().add_run(Run::new().add_text(l.trim_end())),
                );
            }
        }
    }
    let mut buf = Vec::new();
    {
        let mut cursor = std::io::Cursor::new(&mut buf);
        doc.build()
            .pack(&mut cursor)
            .map_err(|e| format!("docx: {e}"))?;
    }
    Ok(buf)
}

fn heading_para(text: &str, level: usize) -> Paragraph {
    let size: usize = match level {
        1 => 32,
        2 => 28,
        3 => 24,
        _ => 22,
    };
    Paragraph::new()
        .style(&format!("Heading{level}"))
        .add_run(Run::new().size(size).bold().add_text(text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gera_docx_nao_vazio() {
        let bytes = markdown_to_docx("# Título\n\nParágrafo um.\n\n## Seção\n\nMais texto.").unwrap();
        assert!(bytes.len() > 100);
        // ZIP signature
        assert_eq!(&bytes[0..2], b"PK");
    }
}
