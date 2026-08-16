// ==============================================================================
// SCRIPT: extraction.rs (melhorador-core)
// DESCRIÇÃO: Extração de texto de PDF — nativo (PDFium) ou OCR (Tesseract)
// CHAMADO POR: src-tauri (comando process_pdf); tests/extraction_real.rs
// CONTRATO (RESPOSTA ESPERADA): ExtractionResult { raw_text, engine, ... }
// ==============================================================================

//! Substitui a cadeia ocrmypdf/Ghostscript/unpaper do CLI por componentes
//! embutíveis (PDFium + Tesseract/Leptonica), conforme o plano do app.
//! Diferente de cleanup/structure, aqui NÃO há paridade byte a byte com o
//! Python: engines de render/OCR distintos produzem texto equivalente, não
//! idêntico. O critério de aceite é QA de conteúdo (golden de qualidade).
//!
//! Estratégia (espelho da extraction.py):
//! 1. Carrega o PDF e seleciona a faixa de páginas pedida.
//! 2. Tenta texto nativo; abaixo de 200 chars, considera escaneado.
//! 3. Escaneado → renderiza cada página (~300 DPI, tons de cinza) e OCR.
//! 4. Junta páginas com \f (mesmo formato do sidecar que o cleanup espera).

use std::path::Path;

use image::codecs::png::PngEncoder;
use image::{ExtendedColorType, ImageEncoder};
use leptess::LepTess;
use pdfium_render::prelude::*;

/// Abaixo deste total de caracteres nativos, tratamos como escaneado.
const NATIVE_TEXT_MIN_CHARS: usize = 200;

/// Palavras curtas que NÃO são fragmento de hifenização na virada de página.
const NO_PAGE_CARRY: &[&str] = &[
    "um", "uma", "de", "da", "do", "em", "no", "na", "os", "as", "ao", "se", "que",
    "com", "por", "mas", "foi", "são", "nao", "não", "ou",
];

/// Fator de escala p/ ~300 DPI (páginas PDF são definidas em 72 pt/pol).
const RENDER_SCALE: f32 = 300.0 / 72.0;

#[derive(Debug)]
pub struct ExtractionResult {
    pub raw_text: String,
    /// "native" | "ocr"
    pub engine: &'static str,
    pub page_count: usize,
    pub native_chars: usize,
    pub ocr_chars: usize,
}

/// Extrai texto de um PDF. `pages` = faixa 1-indexada inclusiva (opcional).
/// `progress(feitas, total, texto_da_pagina)` é chamado por página na fase
/// de OCR — permite à UI mostrar o processamento parcial em tempo real.
pub fn extract_pdf(
    pdfium_lib: &Path,
    pdf_path: &Path,
    pages: Option<(usize, usize)>,
    languages: &str,
    tessdata_dir: Option<&Path>,
    progress: &mut dyn FnMut(usize, usize, &str),
) -> Result<ExtractionResult, String> {
    if !pdf_path.exists() {
        return Err(format!("PDF não encontrado: {}", pdf_path.display()));
    }

    // PDFium carregada em runtime da dylib embutida no app.
    let bindings = Pdfium::bind_to_library(pdfium_lib)
        .map_err(|e| format!("PDFium indisponível ({}): {e}", pdfium_lib.display()))?;
    let pdfium = Pdfium::new(bindings);
    let document = pdfium
        .load_pdf_from_file(pdf_path, None)
        .map_err(|e| format!("Não consegui abrir o PDF: {e}"))?;

    let total = document.pages().len() as usize;
    let (start, end) = match pages {
        Some((s, e)) => {
            if s < 1 || e < s || e > total {
                return Err(format!("Faixa de páginas inválida: {s}-{e} (total {total})"));
            }
            (s, e)
        }
        None => (1, total),
    };
    let selected: Vec<usize> = (start..=end).collect();

    // --- 2. Texto nativo: remontagem POR POSIÇÃO (layout-aware) ---
    // Cabeçalho/nº de página caem por faixa de margem; notas de rodapé
    // (fonte menor, metade inferior) são realocadas para depois do corpo;
    // palavra partida na virada de página é emendada por transporte (\x02).
    let mut native_parts: Vec<String> = Vec::new();
    let mut carry = String::new(); // fragmento "ne\x02" do fim da página anterior
    let mut letter_carry = String::new(); // fragmento sem \x02 ("ne" + "cessário")
    for &p in &selected {
        let page = document
            .pages()
            .get((p - 1) as u16)
            .map_err(|e| format!("Página {p}: {e}"))?;
        let assembled = match page.text() {
            Ok(text_page) => assemble_native_page(&page, &text_page),
            Err(_) => AssembledPage::default(),
        };

        let mut body = assembled.body;
        if !carry.is_empty() {
            body = format!("{carry}{body}");
            carry.clear();
        }
        if !letter_carry.is_empty() {
            if next_accepts_letter_carry(&body) {
                body = format!("{letter_carry}{body}");
            } else if let Some(last) = native_parts.last_mut() {
                append_before_footnotes(last, &letter_carry);
            }
            letter_carry.clear();
        }
        // Corpo termina com palavra hifenizada? Transporta p/ a próxima página.
        if let Some(idx) = body.rfind('\u{2}') {
            if body[idx + 1..].trim().is_empty() {
                let word_start = body[..idx]
                    .rfind(|c: char| c.is_whitespace())
                    .map(|i| i + 1)
                    .unwrap_or(0);
                carry = body[word_start..idx].to_string();
                body.truncate(word_start);
            }
        }
        // \x02 no meio de linha = hifenização interna: juntar as metades.
        let mut body = body.replace('\u{2}', "");
        if let Some((kept, frag)) = word_carry_fragment(&body) {
            letter_carry = frag;
            body = kept;
        }
        if !assembled.footnotes.is_empty() {
            body.push_str("\n\n");
            body.push_str(&assembled.footnotes.replace('\u{2}', ""));
        }
        // Sem linhas em branco na camada nativa o reflow fundiria tudo:
        // inferimos fronteiras de parágrafo antes do pipeline.
        native_parts.push(infer_paragraph_breaks(&body));
    }
    if !carry.is_empty() {
        // Livro terminou com hífen pendente: devolve o fragmento.
        if let Some(last) = native_parts.last_mut() {
            last.push_str(&carry.replace('\u{2}', ""));
        }
    }
    if !letter_carry.is_empty() {
        if let Some(last) = native_parts.last_mut() {
            append_before_footnotes(last, &letter_carry);
        }
    }
    // DIVERGÊNCIA INTENCIONAL do CLI (16/Ago): páginas nativas unidas com \f,
    // como no OCR. O CLI usa "\n\n" e por isso NUNCA removeu cabeçalhos e
    // rodapés de PDFs com texto embutido (comprovado nos gabaritos: Paideia
    // h1=0/headers=0). Com \f, a máquina de limpeza validada funciona igual
    // nos dois caminhos.
    let native_text = native_parts.join("\u{0c}");
    let native_chars = native_text.trim().chars().count();
    if native_chars >= NATIVE_TEXT_MIN_CHARS {
        return Ok(ExtractionResult {
            raw_text: native_text,
            engine: "native",
            page_count: selected.len(),
            native_chars,
            ocr_chars: 0,
        });
    }

    // --- 3. OCR página a página ---
    let tessdata = tessdata_dir
        .map(|p| p.to_string_lossy().into_owned())
        .or_else(default_tessdata);
    let mut ocr = LepTess::new(tessdata.as_deref(), languages)
        .map_err(|e| format!("Tesseract indisponível (idiomas {languages}): {e}"))?;

    let config = PdfRenderConfig::new().scale_page_by_factor(RENDER_SCALE);
    let mut ocr_parts: Vec<String> = Vec::new();
    let n = selected.len();
    for (done, &p) in selected.iter().enumerate() {
        let page = document
            .pages()
            .get((p - 1) as u16)
            .map_err(|e| format!("Página {p}: {e}"))?;
        let bitmap = page
            .render_with_config(&config)
            .map_err(|e| format!("Render da página {p} falhou: {e}"))?;
        // Tons de cinza reduzem ruído e o custo do OCR.
        let gray = bitmap.as_image().to_luma8();

        // PNG em memória: formato que a Leptonica lê de buffer.
        let mut png: Vec<u8> = Vec::new();
        PngEncoder::new(&mut png)
            .write_image(&gray, gray.width(), gray.height(), ExtendedColorType::L8)
            .map_err(|e| format!("PNG da página {p} falhou: {e}"))?;

        ocr.set_image_from_mem(&png)
            .map_err(|e| format!("Tesseract não leu a página {p}: {e}"))?;
        let page_text = ocr
            .get_utf8_text()
            .map_err(|e| format!("OCR da página {p} falhou: {e}"))?;
        progress(done + 1, n, &page_text);
        ocr_parts.push(page_text);
    }

    // \f entre páginas: mesmo contrato do sidecar que o cleanup espera.
    let raw_text = ocr_parts.join("\u{0c}");
    let ocr_chars = raw_text.trim().chars().count();
    Ok(ExtractionResult {
        raw_text,
        engine: "ocr",
        page_count: n,
        native_chars,
        ocr_chars,
    })
}

/// Página nativa remontada por posição: corpo em ordem de leitura + notas.
#[derive(Default)]
struct AssembledPage {
    body: String,
    footnotes: String,
}

/// Linha reconstituída do fluxo de caracteres, com caixa envolvente.
struct StreamLine {
    text: String,
    top: f32,
    bottom: f32,
    left: f32,
    right: f32,
}

impl StreamLine {
    fn height(&self) -> f32 {
        (self.top - self.bottom).abs().max(1.0)
    }
}

/// Fragmento de texto com posição (coordenadas PDF: origem no rodapé).
/// Em muitos PDFs os segmentos vêm POR GLIFO — a remontagem precisa
/// reconstituir palavras (espaço só quando há vão horizontal real).
struct Frag {
    text: String,
    left: f32,
    right: f32,
    center_y: f32,
    top: f32,
    height: f32,
}

/// Remonta a página. Ordem do fluxo de caracteres (igual ao CLI) + coordenadas
/// para: fatiar linhas, descartar margem, realocar notas, e — se for uma
/// coluna — ordenar o corpo de cima para baixo (y PDF cresce para cima).
fn assemble_native_page(page: &PdfPage, text_page: &PdfPageText) -> AssembledPage {
    let page_h = page.height().value;
    let page_w = page.width().value;
    if page_h <= 0.0 {
        return AssembledPage::default();
    }

    // --- 1. Fatia o fluxo em linhas com caixa envolvente ---
    let mut lines: Vec<StreamLine> = Vec::new();
    let mut cur_text = String::new();
    let mut cur_top = f32::MIN;
    let mut cur_bottom = f32::MAX;
    let mut cur_left = f32::MAX;
    let mut cur_right = f32::MIN;

    let flush = |text: &mut String,
                     top: &mut f32,
                     bottom: &mut f32,
                     left: &mut f32,
                     right: &mut f32,
                     out: &mut Vec<StreamLine>| {
        if !text.trim().is_empty() {
            out.push(StreamLine {
                text: std::mem::take(text),
                top: *top,
                bottom: *bottom,
                left: if *left == f32::MAX { 0.0 } else { *left },
                right: if *right == f32::MIN { 0.0 } else { *right },
            });
        } else {
            text.clear();
        }
        *top = f32::MIN;
        *bottom = f32::MAX;
        *left = f32::MAX;
        *right = f32::MIN;
    };

    for ch in text_page.chars().iter() {
        let Some(c) = ch.unicode_char() else { continue };
        if c == '\r' {
            continue;
        }
        if c == '\n' {
            flush(
                &mut cur_text,
                &mut cur_top,
                &mut cur_bottom,
                &mut cur_left,
                &mut cur_right,
                &mut lines,
            );
            continue;
        }
        if let Ok(b) = ch.loose_bounds() {
            let (top, bottom) = (b.top().value, b.bottom().value);
            // Salto vertical abrupto dentro da mesma "linha" do fluxo =
            // outro bloco visual (ex.: cabeçalho grudado) → fatia aqui.
            if cur_top != f32::MIN {
                let cur_center = (cur_top + cur_bottom) / 2.0;
                let ch_center = (top + bottom) / 2.0;
                let line_h = (cur_top - cur_bottom).abs().max(6.0);
                if (cur_center - ch_center).abs() > 1.5 * line_h && !c.is_whitespace() {
                    flush(
                        &mut cur_text,
                        &mut cur_top,
                        &mut cur_bottom,
                        &mut cur_left,
                        &mut cur_right,
                        &mut lines,
                    );
                }
            }
            if !c.is_whitespace() {
                cur_top = if cur_top == f32::MIN { top } else { cur_top.max(top) };
                cur_bottom = if cur_bottom == f32::MAX {
                    bottom
                } else {
                    cur_bottom.min(bottom)
                };
                let l = b.left().value;
                let r = b.right().value;
                cur_left = cur_left.min(l);
                cur_right = cur_right.max(r);
            }
        }
        cur_text.push(c);
    }
    flush(
        &mut cur_text,
        &mut cur_top,
        &mut cur_bottom,
        &mut cur_left,
        &mut cur_right,
        &mut lines,
    );

    if lines.is_empty() {
        return AssembledPage::default();
    }

    // Mediana das alturas de linha (proxy do corpo da fonte).
    let mut heights: Vec<f32> = lines.iter().map(|l| l.height()).collect();
    heights.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median_h = heights[heights.len() / 2];

    // --- 2 e 3. Classifica cada linha ---
    let mut body_lines: Vec<StreamLine> = Vec::new();
    let mut notes: Vec<String> = Vec::new();
    for line in lines {
        let short = line.text.trim().chars().count() <= 80;
        let in_top_band = line.bottom / page_h > 0.93;
        let in_bottom_band = line.top / page_h < 0.07;
        if (in_top_band || in_bottom_band) && short {
            continue; // cabeçalho corrente / número de página
        }
        let is_footnote = line.height() < 0.85 * median_h && line.top / page_h < 0.5;
        if is_footnote {
            notes.push(line.text.trim_end().to_string());
        } else {
            body_lines.push(line);
        }
    }

    if is_single_column(&body_lines, page_w) {
        sort_body_visual(&mut body_lines);
    }

    AssembledPage {
        body: body_lines
            .iter()
            .map(|l| l.text.trim_end().to_string())
            .collect::<Vec<_>>()
            .join("\n"),
        footnotes: notes.join("\n"),
    }
}

/// (Mantida para referência do caminho por segmentos; não usada no fluxo
/// principal — a remontagem oficial é assemble_native_page.)
#[allow(dead_code)]
fn assemble_native_page_by_segments(page: &PdfPage, text_page: &PdfPageText) -> AssembledPage {
    let page_h = page.height().value;
    if page_h <= 0.0 {
        return AssembledPage::default();
    }

    let mut frags: Vec<Frag> = Vec::new();
    for segment in text_page.segments().iter() {
        let text = segment.text();
        let trimmed = text.trim();
        if trimmed.is_empty() {
            continue;
        }
        let b = segment.bounds();
        let (top, bottom) = (b.top().value, b.bottom().value);
        let (left, right) = (b.left().value, b.right().value);
        // 1. Margens: só descarta trecho CURTO (corpo nunca é descartado).
        let in_top_band = bottom / page_h > 0.93;
        let in_bottom_band = top / page_h < 0.07;
        if (in_top_band || in_bottom_band) && trimmed.chars().count() <= 80 {
            continue;
        }
        frags.push(Frag {
            text: trimmed.to_string(),
            left,
            right,
            center_y: (top + bottom) / 2.0,
            top,
            height: (top - bottom).abs().max(1.0),
        });
    }
    if frags.is_empty() {
        return AssembledPage::default();
    }

    // Mediana das alturas (proxy determinístico do corpo da fonte).
    let mut heights: Vec<f32> = frags.iter().map(|f| f.height).collect();
    heights.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median_h = heights[heights.len() / 2];

    // 2. Notas de rodapé: fonte menor + metade inferior da página.
    let (foot, body): (Vec<Frag>, Vec<Frag>) = frags
        .into_iter()
        .partition(|f| f.height < 0.85 * median_h && f.top / page_h < 0.5);

    AssembledPage {
        body: frags_to_lines(body, median_h),
        footnotes: frags_to_lines(foot, median_h),
    }
}

/// Ordena fragmentos em linhas de leitura (topo→base; esquerda→direita).
/// Espaço entre fragmentos só quando há VÃO horizontal real (fragmentos
/// por glifo não podem ganhar espaço à força — viraria s o p a d e l e t r a s).
fn frags_to_lines(mut frags: Vec<Frag>, median_h: f32) -> String {
    if frags.is_empty() {
        return String::new();
    }
    // Topo da página primeiro (coordenada y decrescente), pelo CENTRO.
    frags.sort_by(|a, b| b.center_y.partial_cmp(&a.center_y).unwrap());
    let mut lines: Vec<Vec<Frag>> = Vec::new();
    let mut line_center = f32::MAX;
    for frag in frags {
        let mut tolerance = 0.55 * frag.height.max(median_h);
        // Pontuação tem caixa minúscula no baseline: tolerância maior para
        // não abrir "linha da vírgula" própria.
        let is_punct = frag.text.chars().count() == 1
            && frag
                .text
                .chars()
                .next()
                .map(|c| ",.;:!?»«\"'“”‘’-–—".contains(c))
                .unwrap_or(false);
        if is_punct {
            tolerance = 1.2 * median_h;
        }
        if line_center == f32::MAX || line_center - frag.center_y > tolerance {
            line_center = frag.center_y;
            lines.push(Vec::new());
        }
        lines.last_mut().unwrap().push(frag);
    }
    let mut out: Vec<String> = Vec::new();
    for mut line in lines {
        line.sort_by(|a, b| a.left.partial_cmp(&b.left).unwrap());
        let mut buf = String::new();
        let mut prev_right: Option<f32> = None;
        for frag in &line {
            if let Some(pr) = prev_right {
                // Vão > ~25% do corpo da fonte = separação de palavra.
                if frag.left - pr > 0.25 * frag.height.max(median_h) {
                    buf.push(' ');
                } else if frag.left < pr - 0.15 * frag.height.max(median_h) {
                    // Sobreposição horizontal: segmentos de ligadura repetem
                    // glifos ("fo"+"forma" → "foforma"). Descarta o prefixo
                    // do fragmento que já está no fim do buffer.
                    let fchars: Vec<char> = frag.text.chars().collect();
                    let mut overlap = 0usize;
                    for k in (1..=fchars.len().min(4)).rev() {
                        let prefix: String = fchars[..k].iter().collect();
                        if buf.ends_with(&prefix) {
                            overlap = k;
                            break;
                        }
                    }
                    if overlap > 0 {
                        let rest: String = fchars[overlap..].iter().collect();
                        buf.push_str(&rest);
                        prev_right = Some(frag.right.max(pr));
                        continue;
                    }
                }
            }
            buf.push_str(&frag.text);
            prev_right = Some(frag.right.max(prev_right.unwrap_or(f32::MIN)));
        }
        out.push(buf);
    }
    out.join("\n")
}

/// Infere fronteiras de parágrafo em texto nativo sem linhas em branco.
///
/// Regras determinísticas (nenhuma reescrita, só quebras):
/// - Nova fronteira quando a linha anterior termina sentença (. ! ? » ” ")
///   e a atual começa com maiúscula, aspas de abertura ou travessão.
/// - Linha curta em CAIXA ALTA (título/cabeçalho) vira bloco isolado.
/// - Junta o hífen duplicado de clítico ("ignorá-\n-lo" → "ignorá-lo").
pub fn infer_paragraph_breaks(page_text: &str) -> String {
    let lines: Vec<&str> = page_text.lines().map(|l| l.trim_end()).collect();
    let mut out: Vec<String> = Vec::new();
    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            out.push(String::new());
            continue;
        }
        let isolated_caps = is_isolated_caps(trimmed);
        if let Some(prev) = out.last() {
            let prev_trim = prev.trim_end();
            let sentence_end = prev_trim
                .chars()
                .last()
                .map(|c| matches!(c, '.' | '!' | '?' | '»' | '”' | '"'))
                .unwrap_or(false);
            let starts_new = trimmed
                .chars()
                .next()
                .map(|c| c.is_uppercase() || matches!(c, '«' | '“' | '"' | '—' | '–'))
                .unwrap_or(false);
            if !prev_trim.is_empty()
                && (isolated_caps || is_isolated_caps(prev_trim) || (sentence_end && starts_new))
            {
                out.push(String::new()); // linha em branco = fronteira
            }
        }
        // Clítico: linha anterior termina com '-' e esta começa com "-x".
        if let Some(prev) = out.last_mut() {
            if prev.trim_end().ends_with('-') && trimmed.starts_with('-') {
                let joined = trimmed.trim_start_matches('-');
                prev.push_str(joined);
                continue;
            }
        }
        out.push(trimmed.to_string());
    }
    out.join("\n")
}

/// Linha curta toda em caixa alta (≥60% maiúsculas entre letras) — título
/// ou cabeçalho corrente, possivelmente com número de página anexado.
fn is_isolated_caps(line: &str) -> bool {
    let n = line.chars().count();
    if !(4..=80).contains(&n) {
        return false;
    }
    let letters: Vec<char> = line.chars().filter(|c| c.is_alphabetic()).collect();
    if letters.len() < 4 {
        return false;
    }
    let upper = letters.iter().filter(|c| c.is_uppercase()).count();
    (upper as f64 / letters.len() as f64) >= 0.6
}

/// tessdata padrão: instalação do Homebrew (dev). O app gerenciado (E3)
/// passará o diretório próprio via parâmetro.
fn default_tessdata() -> Option<String> {
    for candidate in [
        "/opt/homebrew/share/tessdata",
        "/usr/local/share/tessdata",
        "/usr/share/tesseract-ocr/5/tessdata",
    ] {
        if Path::new(candidate).is_dir() {
            return Some(candidate.to_string());
        }
    }
    None
}

/// Uma coluna: não há grupo à esquerda E outro à direita ao mesmo tempo.
fn is_single_column(lines: &[StreamLine], page_w: f32) -> bool {
    if page_w <= 0.0 || lines.len() < 6 {
        return true;
    }
    let leftish = lines.iter().filter(|l| l.right < page_w * 0.48).count();
    let rightish = lines.iter().filter(|l| l.left > page_w * 0.52).count();
    !(leftish >= 3 && rightish >= 3)
}

/// Ordem visual: topo da página primeiro (y PDF cresce para cima).
fn sort_body_visual(lines: &mut [StreamLine]) {
    lines.sort_by(|a, b| b.top.partial_cmp(&a.top).unwrap_or(std::cmp::Ordering::Equal));
}

/// Último token é fragmento de palavra (2–3 letras minúsculas, não stopword).
fn word_carry_fragment(body: &str) -> Option<(String, String)> {
    let t = body.trim_end();
    let last = t.split_whitespace().last()?;
    let n = last.chars().count();
    if !(2..=3).contains(&n) {
        return None;
    }
    if !last.chars().all(|c| c.is_alphabetic() && c.is_lowercase()) {
        return None;
    }
    if NO_PAGE_CARRY.contains(&last) {
        return None;
    }
    if !t.ends_with(last) {
        return None;
    }
    let kept = t[..t.len() - last.len()].trim_end().to_string();
    Some((kept, last.to_string()))
}

fn next_accepts_letter_carry(next: &str) -> bool {
    let first = next.split_whitespace().next().unwrap_or("");
    first.chars().count() >= 5
        && first
            .chars()
            .next()
            .map(|c| c.is_lowercase() && c.is_alphabetic())
            .unwrap_or(false)
}

fn append_before_footnotes(page: &mut String, frag: &str) {
    if let Some(idx) = page.rfind("\n\n") {
        page.insert_str(idx, &format!(" {frag}"));
    } else if page.is_empty() {
        page.push_str(frag);
    } else {
        page.push_str(&format!(" {frag}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(text: &str, top: f32, left: f32, right: f32) -> StreamLine {
        StreamLine {
            text: text.into(),
            top,
            bottom: top - 10.0,
            left,
            right,
        }
    }

    #[test]
    fn coluna_unica_ordena_de_cima_para_baixo() {
        let mut lines = vec![
            line("rabo do parágrafo", 100.0, 50.0, 400.0),
            line("começo visual", 700.0, 50.0, 400.0),
        ];
        assert!(is_single_column(&lines, 500.0));
        sort_body_visual(&mut lines);
        assert_eq!(lines[0].text, "começo visual");
        assert_eq!(lines[1].text, "rabo do parágrafo");
    }

    #[test]
    fn duas_colunas_nao_embaralha() {
        let lines = vec![
            line("a", 700.0, 10.0, 80.0),
            line("b", 600.0, 10.0, 80.0),
            line("c", 500.0, 10.0, 80.0),
            line("d", 700.0, 300.0, 480.0),
            line("e", 600.0, 300.0, 480.0),
            line("f", 500.0, 300.0, 480.0),
        ];
        assert!(!is_single_column(&lines, 500.0));
    }

    #[test]
    fn carry_junta_ne_cessario() {
        let (kept, frag) = word_carry_fragment("era ne").unwrap();
        assert_eq!(kept, "era");
        assert_eq!(frag, "ne");
        assert!(next_accepts_letter_carry("cessário continuar"));
        let joined = format!("{frag}{}", "cessário continuar");
        assert!(joined.starts_with("necessário"));
    }

    #[test]
    fn carry_nao_junta_um_elemento() {
        assert!(word_carry_fragment("era um").is_none());
        assert!(!next_accepts_letter_carry("a cidade"));
    }
}
