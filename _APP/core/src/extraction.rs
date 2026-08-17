// ==============================================================================
// SCRIPT: extraction.rs (txtmelhorator-core)
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
//! Estratégia (espelho da extraction.py + híbrido):
//! 1. Carrega o PDF e seleciona a faixa de páginas pedida.
//! 2. Tenta texto nativo; abaixo de 200 chars, considera escaneado.
//! 3. Se nativo ok, ainda assim OCR nas páginas vazias/[figura] (capa, gravura).
//! 4. Escaneado total → renderiza cada página (~300 DPI) e OCR.
//! 5. Junta páginas com \f (mesmo formato do sidecar que o cleanup espera).

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
    "com", "por", "mas", "foi", "são", "sao", "nao", "não", "ou",
    // Palavras curtas comuns (não são sílaba partida):
    "era", "ser", "ter", "seu", "sua", "ele", "ela", "uns", "há", "ha", "já", "ja",
];

/// Fator de escala p/ ~300 DPI (páginas PDF são definidas em 72 pt/pol).
const RENDER_SCALE: f32 = 300.0 / 72.0;

/// Escala leve p/ conferência na UI (~150 DPI).
const UI_RENDER_SCALE: f32 = 150.0 / 72.0;

/// Renderiza uma página (1-based) em PNG (RGB). Para a tela lado a lado.
pub fn render_page_png(
    pdfium_lib: &Path,
    pdf_path: &Path,
    page: usize,
    scale: Option<f32>,
) -> Result<Vec<u8>, String> {
    if !pdf_path.exists() {
        return Err(format!("PDF não encontrado: {}", pdf_path.display()));
    }
    if page < 1 {
        return Err("Página deve ser ≥ 1".into());
    }
    let bindings = Pdfium::bind_to_library(pdfium_lib)
        .map_err(|e| format!("PDFium indisponível ({}): {e}", pdfium_lib.display()))?;
    let pdfium = Pdfium::new(bindings);
    let document = pdfium
        .load_pdf_from_file(pdf_path, None)
        .map_err(|e| format!("Não consegui abrir o PDF: {e}"))?;
    let total = document.pages().len() as usize;
    if page > total {
        return Err(format!("Página {page} inexistente (total {total})"));
    }
    let scale = scale.unwrap_or(UI_RENDER_SCALE);
    let config = PdfRenderConfig::new().scale_page_by_factor(scale);
    let pdf_page = document
        .pages()
        .get((page - 1) as u16)
        .map_err(|e| format!("Página {page}: {e}"))?;
    let bitmap = pdf_page
        .render_with_config(&config)
        .map_err(|e| format!("Render da página {page} falhou: {e}"))?;
    let rgba = bitmap.as_image().to_rgba8();
    let mut png: Vec<u8> = Vec::new();
    PngEncoder::new(&mut png)
        .write_image(
            rgba.as_raw(),
            rgba.width(),
            rgba.height(),
            ExtendedColorType::Rgba8,
        )
        .map_err(|e| format!("PNG da página {page} falhou: {e}"))?;
    Ok(png)
}

/// Contagem total de páginas do PDF.
pub fn pdf_page_count(pdfium_lib: &Path, pdf_path: &Path) -> Result<usize, String> {
    if !pdf_path.exists() {
        return Err(format!("PDF não encontrado: {}", pdf_path.display()));
    }
    let bindings = Pdfium::bind_to_library(pdfium_lib)
        .map_err(|e| format!("PDFium indisponível ({}): {e}", pdfium_lib.display()))?;
    let pdfium = Pdfium::new(bindings);
    let document = pdfium
        .load_pdf_from_file(pdf_path, None)
        .map_err(|e| format!("Não consegui abrir o PDF: {e}"))?;
    Ok(document.pages().len() as usize)
}

#[derive(Debug)]
pub struct ExtractionResult {
    pub raw_text: String,
    /// "native" | "ocr"
    pub engine: &'static str,
    pub page_count: usize,
    pub native_chars: usize,
    pub ocr_chars: usize,
}

/// Mensagem estável quando o usuário cancela (UI compara esta string).
pub const CANCELLED: &str = "CANCELLED";

fn check_cancel(should_cancel: Option<&dyn Fn() -> bool>) -> Result<(), String> {
    if should_cancel.map(|f| f()).unwrap_or(false) {
        Err(CANCELLED.into())
    } else {
        Ok(())
    }
}

/// Amostra texto nativo das primeiras páginas (p/ detecção de idioma).
/// Livro só-imagem → string vazia ou quase.
pub fn sample_native_text(
    pdfium_lib: &Path,
    pdf_path: &Path,
    max_pages: usize,
) -> Result<String, String> {
    if !pdf_path.exists() {
        return Err(format!("PDF não encontrado: {}", pdf_path.display()));
    }
    let bindings = Pdfium::bind_to_library(pdfium_lib)
        .map_err(|e| format!("PDFium indisponível ({}): {e}", pdfium_lib.display()))?;
    let pdfium = Pdfium::new(bindings);
    let document = pdfium
        .load_pdf_from_file(pdf_path, None)
        .map_err(|e| format!("Não consegui abrir o PDF: {e}"))?;
    let total = document.pages().len() as usize;
    let n = max_pages.min(total).max(0);
    let mut parts: Vec<String> = Vec::new();
    for p in 1..=n {
        let page = document
            .pages()
            .get((p - 1) as u16)
            .map_err(|e| format!("Página {p}: {e}"))?;
        let assembled = match page.text() {
            Ok(text_page) => assemble_native_page(&page, &text_page),
            Err(_) => AssembledPage::default(),
        };
        parts.push(assembled.body);
    }
    Ok(parts.join("\n"))
}

/// Heurística leve (sem IA): português vs inglês vs ambos.
/// Inconclusivo → `por+eng`.
pub fn detect_ocr_languages(sample: &str) -> &'static str {
    let lower = sample.to_lowercase();
    let chars = lower.chars().filter(|c| c.is_alphabetic()).count();
    if chars < 80 {
        return "por+eng";
    }
    let accents = lower
        .chars()
        .filter(|c| "áàâãéêíóôõúç".contains(*c))
        .count();
    let por_hits = ["ção", "ões", "não", "também", "porque", "através", "histórico"]
        .iter()
        .filter(|w| lower.contains(*w))
        .count();
    let eng_hits = ["the", "and", "with", "which", "that", "from", "this"]
        .iter()
        .filter(|w| {
            // palavra inteira aproximada
            lower.contains(&format!(" {w} ")) || lower.starts_with(&format!("{w} "))
        })
        .count();

    let accent_ratio = accents as f32 / chars as f32;
    if accent_ratio > 0.01 || por_hits >= 2 {
        if eng_hits >= 3 && por_hits < 2 {
            "por+eng"
        } else {
            "por"
        }
    } else if eng_hits >= 3 && por_hits == 0 {
        "eng"
    } else {
        "por+eng"
    }
}

/// Extrai texto de um PDF. `pages` = faixa 1-indexada inclusiva (opcional).
/// `progress(página, total, texto, preview_png)` — preview só nas páginas OCR.
/// `should_cancel`: se retornar true entre páginas, aborta com [`CANCELLED`].
/// `user_rules`: regras NoJoin (R4) aplicadas no transporte entre páginas.
pub fn extract_pdf(
    pdfium_lib: &Path,
    pdf_path: &Path,
    pages: Option<(usize, usize)>,
    languages: &str,
    tessdata_dir: Option<&Path>,
    progress: &mut dyn FnMut(usize, usize, &str, Option<&[u8]>),
    should_cancel: Option<&dyn Fn() -> bool>,
    user_rules: &[crate::rules::UserRule],
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
        check_cancel(should_cancel)?;
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
            // Paideia: o sufixo pode não ser o 1º token (cabeçalho/nº no meio).
            match merge_letter_carry(&body, &letter_carry) {
                Some(merged) => body = merged,
                None => {
                    if let Some(last) = native_parts.last_mut() {
                        append_before_footnotes(last, &letter_carry);
                    }
                }
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
        // Fragmento antes da 1ª nota ("era ne" + "103." + prosa da outra coluna).
        let mut stranded: Option<String> = None;
        if let Some((new_body, frag)) = take_carry_before_first_footnote(&body) {
            body = new_body;
            stranded = Some(frag);
        }
        if let Some((kept, frag)) = word_carry_fragment(&body) {
            if crate::rules::is_no_join_fragment(&frag, user_rules) {
                // R4 NoJoin: fragmento fica na página; não transporta.
            } else {
                letter_carry = frag;
                body = kept;
            }
        } else if let Some(frag) = stranded.or(assembled.pending_carry) {
            if !crate::rules::is_no_join_fragment(&frag, user_rules) {
                letter_carry = frag;
            }
        }
        if !assembled.footnotes.is_empty() {
            body.push_str("\n\n");
            body.push_str(&assembled.footnotes.replace('\u{2}', ""));
        }
        // Sem linhas em branco na camada nativa o reflow fundiria tudo:
        // inferimos fronteiras de parágrafo antes do pipeline.
        let page_text = infer_paragraph_breaks(&body);
        let page_out = crate::blocks::figure_placeholder_if_empty(&page_text);
        // UMA emissão por página, sempre com o texto final: página nativa com
        // conteúdo sai aqui; página muda sai depois, na passada de OCR.
        // (Antes emitia "" para todas — a UI recebia página vazia e o mesmo
        // número chegava duas vezes.)
        if !page_needs_ocr_fill(&page_out) {
            progress(p, end, &page_out, None);
        }
        native_parts.push(page_out);
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
    let native_chars = native_parts
        .iter()
        .map(|p| p.trim().chars().count())
        .sum::<usize>();
    let n = selected.len();
    let range_end = *selected.last().unwrap_or(&end);

    // --- 3. Nativo suficiente: preenche com OCR só as páginas mudas ---
    if native_chars >= NATIVE_TEXT_MIN_CHARS {
        let empty_idx: Vec<usize> = native_parts
            .iter()
            .enumerate()
            .filter(|(_, t)| page_needs_ocr_fill(t))
            .map(|(i, _)| i)
            .collect();
        if empty_idx.is_empty() {
            return Ok(ExtractionResult {
                raw_text: native_parts.join("\u{0c}"),
                engine: "native",
                page_count: n,
                native_chars,
                ocr_chars: 0,
            });
        }

        let tessdata = tessdata_dir
            .map(|p| p.to_string_lossy().into_owned())
            .or_else(default_tessdata);
        let mut ocr = LepTess::new(tessdata.as_deref(), languages)
            .map_err(|e| format!("Tesseract indisponível (idiomas {languages}): {e}"))?;
        let config = PdfRenderConfig::new().scale_page_by_factor(RENDER_SCALE);
        let mut ocr_chars = 0usize;
        for &i in &empty_idx {
            check_cancel(should_cancel)?;
            let p = selected[i];
            let (page_text, preview) = ocr_one_page(&document, &mut ocr, &config, p)?;
            // done/total = nº real da página / última da faixa (UI acompanha o PDF).
            progress(p, range_end, &page_text, Some(&preview));
            ocr_chars += page_text.trim().chars().count();
            if !crate::blocks::is_near_empty_page(&page_text) {
                native_parts[i] = page_text;
            }
        }
        return Ok(ExtractionResult {
            raw_text: native_parts.join("\u{0c}"),
            engine: "hybrid",
            page_count: n,
            native_chars,
            ocr_chars,
        });
    }

    // --- 4. Escaneado: OCR em todas as páginas ---
    let tessdata = tessdata_dir
        .map(|p| p.to_string_lossy().into_owned())
        .or_else(default_tessdata);
    let mut ocr = LepTess::new(tessdata.as_deref(), languages)
        .map_err(|e| format!("Tesseract indisponível (idiomas {languages}): {e}"))?;

    let config = PdfRenderConfig::new().scale_page_by_factor(RENDER_SCALE);
    let mut ocr_parts: Vec<String> = Vec::new();
    for &p in &selected {
        check_cancel(should_cancel)?;
        let (page_text, preview) = ocr_one_page(&document, &mut ocr, &config, p)?;
        progress(p, range_end, &page_text, Some(&preview));
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

/// Página nativa sem prosa útil → precisa de OCR de preenchimento.
fn page_needs_ocr_fill(text: &str) -> bool {
    let t = text.trim();
    t.is_empty() || t == "[figura]" || crate::blocks::is_near_empty_page(t)
}

/// Termina em pontuação que fecha sentença/bloco (fim legítimo de parágrafo).
fn ends_sentence_like(line: &str) -> bool {
    matches!(
        line.trim_end().chars().last(),
        Some('.' | '!' | '?' | ':' | ';' | '»' | '”' | '"' | '…' | ')' | ']')
    )
}

/// Limpa artefatos recorrentes do Tesseract neste acervo (evidência 16/Ago,
/// Schopenhauer I, págs. 6-12):
/// 1. Barras "|" órfãs nas bordas das linhas (sombra da margem escaneada).
/// 2. Linha em branco entre TODAS as linhas quando a entrelinha é larga —
///    cada linha virava "parágrafo" e o reflow não juntava nada.
/// 3. Palavra hifenizada separada por linha em branco ("le-" ⏎⏎ "va").
/// Determinístico: só remove ruído e quebras; nunca altera palavras.
fn normalize_ocr_page_text(text: &str) -> String {
    // Passo 1: por linha, tira espaço à direita e barras órfãs nas pontas.
    let mut lines: Vec<String> = Vec::new();
    for raw in text.lines() {
        let mut l = raw.trim_end().to_string();
        loop {
            let t = l.trim_end().to_string();
            if let Some(stripped) = t.strip_suffix('|') {
                l = stripped.trim_end().to_string();
            } else {
                l = t;
                break;
            }
        }
        // Barra órfã no início ("| texto" / "|texto" não; só barra isolada).
        if let Some(stripped) = l.strip_prefix("| ") {
            l = stripped.to_string();
        } else if l == "|" {
            l.clear();
        }
        lines.push(l);
    }

    // Passo 2: remove linha em branco espúria — a linha anterior não fecha
    // sentença (ou termina em hífen) E a seguinte começa com minúscula.
    let mut out: Vec<String> = Vec::new();
    for line in lines {
        if line.trim().is_empty() {
            // colapsa brancos consecutivos
            if out.last().map(|l: &String| l.is_empty()).unwrap_or(true) {
                continue;
            }
            out.push(String::new());
            continue;
        }
        let starts_lower = line
            .trim_start()
            .chars()
            .next()
            .map(|c| c.is_lowercase() && c.is_alphabetic())
            .unwrap_or(false);
        if starts_lower && out.last().map(|l| l.is_empty()).unwrap_or(false) {
            if let Some(prev) = out.iter().rev().find(|l| !l.is_empty()) {
                let hyphen_break = prev.ends_with('-');
                if hyphen_break || !ends_sentence_like(prev) {
                    // some a linha em branco: "le-"⏎"va" fica adjacente e o
                    // dehyphenate/reflow do cleanup junta como sempre juntou.
                    out.pop();
                }
            }
        }
        out.push(line);
    }
    // Sem branco pendurado no fim.
    while out.last().map(|l| l.is_empty()).unwrap_or(false) {
        out.pop();
    }
    out.join("\n")
}

fn ocr_one_page(
    document: &PdfDocument<'_>,
    ocr: &mut LepTess,
    config: &PdfRenderConfig,
    page_num: usize,
) -> Result<(String, Vec<u8>), String> {
    let page = document
        .pages()
        .get((page_num - 1) as u16)
        .map_err(|e| format!("Página {page_num}: {e}"))?;
    let bitmap = page
        .render_with_config(config)
        .map_err(|e| format!("Render da página {page_num} falhou: {e}"))?;
    let dynamic = bitmap.as_image();
    // Preview p/ a UI (mesmo bitmap do OCR, reduzido) — sem reabrir o PDF.
    let preview = encode_ui_preview_png(&dynamic, page_num)?;
    // Tons de cinza + contraste leve (pré-processamento R2).
    let gray = crate::preprocess::prepare_for_ocr(&dynamic);

    // PNG em memória: formato que a Leptonica lê de buffer.
    let mut png: Vec<u8> = Vec::new();
    PngEncoder::new(&mut png)
        .write_image(&gray, gray.width(), gray.height(), ExtendedColorType::L8)
        .map_err(|e| format!("PNG da página {page_num} falhou: {e}"))?;

    ocr.set_image_from_mem(&png)
        .map_err(|e| format!("Tesseract não leu a página {page_num}: {e}"))?;
    let text = ocr
        .get_utf8_text()
        .map_err(|e| format!("OCR da página {page_num} falhou: {e}"))?;
    Ok((normalize_ocr_page_text(&text), preview))
}

/// PNG leve da página para a coluna Original durante o OCR.
fn encode_ui_preview_png(img: &image::DynamicImage, page_num: usize) -> Result<Vec<u8>, String> {
    let rgba = img.to_rgba8();
    let max_w = 900u32;
    let frame = if rgba.width() > max_w {
        let h = ((rgba.height() as f64) * (max_w as f64) / (rgba.width() as f64))
            .round()
            .max(1.0) as u32;
        image::imageops::resize(
            &rgba,
            max_w,
            h,
            image::imageops::FilterType::Triangle,
        )
    } else {
        rgba
    };
    let mut png: Vec<u8> = Vec::new();
    PngEncoder::new(&mut png)
        .write_image(
            frame.as_raw(),
            frame.width(),
            frame.height(),
            ExtendedColorType::Rgba8,
        )
        .map_err(|e| format!("Preview PNG da página {page_num} falhou: {e}"))?;
    Ok(png)
}

/// Página nativa remontada por posição: corpo em ordem de leitura + notas.
#[derive(Default)]
struct AssembledPage {
    body: String,
    footnotes: String,
    /// Fragmento do fim da coluna esquerda que NÃO colou na direita
    /// (ex.: "ne" → vai para a próxima página, onde está "cessário").
    pending_carry: Option<String>,
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
        finalize_assembled(join_line_texts(&body_lines), notes, None)
    } else {
        // Duas colunas: esquerda cima→baixo, depois direita (ordem de leitura).
        let (left, right) = split_columns(body_lines, page_w);
        let mut left_text = join_line_texts(&left);
        let mut right_text = join_line_texts(&right);
        let mut pending_carry = None;
        // Notas numeradas no rodapé da coluna não podem esconder o fragmento ("era ne").
        let prose_left = strip_trailing_footnote_lines(&left_text);
        let trailing_left = left_text
            .strip_prefix(&prose_left)
            .unwrap_or("")
            .to_string();
        if let Some((kept, frag)) = word_carry_fragment(&prose_left) {
            if let Some(merged) = merge_letter_carry(&right_text, &frag) {
                left_text = format!("{kept}{trailing_left}");
                right_text = merged;
            } else {
                left_text = format!("{kept}{trailing_left}");
                pending_carry = Some(frag);
            }
        }
        let body = match (left_text.is_empty(), right_text.is_empty()) {
            (false, false) => format!("{left_text}\n{right_text}"),
            (false, true) => left_text,
            (true, false) => right_text,
            (true, true) => String::new(),
        };
        finalize_assembled(body, notes, pending_carry)
    }
}

/// Notas numeradas no meio + tabelas → Markdown; junta notas de fonte pequena.
fn finalize_assembled(
    body: String,
    mut notes: Vec<String>,
    mut pending_carry: Option<String>,
) -> AssembledPage {
    // Carry antes de puxar notas (precisa do `103.` ainda no corpo).
    let body = if pending_carry.is_none() {
        if let Some((new_body, frag)) = take_carry_before_first_footnote(&body) {
            pending_carry = Some(frag);
            new_body
        } else {
            body
        }
    } else {
        body
    };
    let (body, pulled) = pull_numbered_notes(&body);
    notes.extend(pulled);
    let body = convert_tables_in_text(&body);
    AssembledPage {
        body,
        footnotes: notes.join("\n"),
        pending_carry,
    }
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

fn join_line_texts(lines: &[StreamLine]) -> String {
    lines
        .iter()
        .map(|l| l.text.trim_end().to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Remove linhas finais tipo nota ("103. AUTOR…") para achar fragmento de palavra.
fn strip_trailing_footnote_lines(text: &str) -> String {
    let mut lines: Vec<&str> = text.lines().collect();
    while let Some(last) = lines.last() {
        let t = last.trim();
        if t.is_empty() || line_starts_like_footnote(t) {
            lines.pop();
        } else {
            break;
        }
    }
    lines.join("\n")
}

fn line_starts_like_footnote(t: &str) -> bool {
    let digs = t.chars().take_while(|c| c.is_ascii_digit()).count();
    if digs == 0 || digs > 3 {
        return false;
    }
    matches!(t.chars().nth(digs), Some('.' | ')' | ' '))
}

/// Reinício óbvio de prosa (não é continuação de nota).
fn is_prose_restart(t: &str) -> bool {
    let t = t.trim();
    if t.is_empty() || line_starts_like_footnote(t) {
        return t.is_empty();
    }
    t.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) && t.chars().count() > 35
}

/// Tira notas `N.` / `N)` do meio do corpo → lista à parte.
fn pull_numbered_notes(body: &str) -> (String, Vec<String>) {
    let lines: Vec<&str> = body.lines().collect();
    let mut prose: Vec<&str> = Vec::new();
    let mut notes: Vec<String> = Vec::new();
    let mut i = 0usize;
    while i < lines.len() {
        let t = lines[i].trim();
        if line_starts_like_footnote(t) {
            let mut note = t.to_string();
            i += 1;
            while i < lines.len() {
                let n = lines[i].trim();
                if n.is_empty() {
                    i += 1;
                    break;
                }
                if line_starts_like_footnote(n) || is_prose_restart(n) {
                    break;
                }
                note.push(' ');
                note.push_str(n);
                i += 1;
            }
            notes.push(note);
            continue;
        }
        prose.push(lines[i]);
        i += 1;
    }
    // Colapsa excesso de linhas em branco deixado pelas notas removidas.
    let mut cleaned = String::new();
    let mut blank_run = 0usize;
    for line in prose {
        if line.trim().is_empty() {
            blank_run += 1;
            if blank_run <= 2 {
                cleaned.push('\n');
            }
        } else {
            blank_run = 0;
            if !cleaned.is_empty() && !cleaned.ends_with('\n') {
                cleaned.push('\n');
            }
            cleaned.push_str(line);
        }
    }
    (cleaned, notes)
}

/// Parte células por 2+ espaços ou tab.
fn split_table_cells(line: &str) -> Vec<String> {
    let t = line.trim();
    if t.is_empty() {
        return Vec::new();
    }
    if t.contains('\t') {
        return t
            .split('\t')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
    }
    let mut cells = Vec::new();
    let mut cur = String::new();
    let mut space_run = 0usize;
    for c in t.chars() {
        if c.is_whitespace() {
            space_run += 1;
            if space_run >= 2 {
                if !cur.is_empty() {
                    cells.push(std::mem::take(&mut cur));
                }
            } else {
                cur.push(' ');
            }
        } else {
            space_run = 0;
            cur.push(c);
        }
    }
    if !cur.is_empty() {
        cells.push(cur);
    }
    cells
        .into_iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn rows_to_markdown(rows: &[Vec<String>]) -> String {
    let cols = rows[0].len();
    let mut out = String::new();
    for (i, row) in rows.iter().enumerate() {
        out.push('|');
        for cell in row {
            out.push(' ');
            out.push_str(cell);
            out.push(' ');
            out.push('|');
        }
        out.push('\n');
        if i == 0 {
            out.push('|');
            for _ in 0..cols {
                out.push_str(" --- |");
            }
            out.push('\n');
        }
    }
    out.trim_end().to_string()
}

/// Detecta bloco de linhas com N colunas alinhadas (2+ espaços) → Markdown.
fn detect_table_at(lines: &[&str], start: usize) -> Option<(String, usize)> {
    let first = split_table_cells(lines[start]);
    if first.len() < 2 {
        return None;
    }
    let cols = first.len();
    let mut rows = vec![first];
    let mut j = start + 1;
    while j < lines.len() {
        let t = lines[j].trim();
        if t.is_empty() {
            break;
        }
        let cells = split_table_cells(lines[j]);
        if cells.len() != cols {
            break;
        }
        rows.push(cells);
        j += 1;
    }
    if rows.len() < 2 {
        return None;
    }
    Some((rows_to_markdown(&rows), j - start))
}

fn convert_tables_in_text(body: &str) -> String {
    let lines: Vec<&str> = body.lines().collect();
    if lines.is_empty() {
        return String::new();
    }
    let mut out: Vec<String> = Vec::new();
    let mut i = 0usize;
    while i < lines.len() {
        if let Some((md, n)) = detect_table_at(&lines, i) {
            out.push(md);
            i += n;
            continue;
        }
        out.push(lines[i].to_string());
        i += 1;
    }
    out.join("\n")
}

/// Se a prosa antes da 1ª nota termina em fragmento (2–3 letras), remove e devolve.
/// Caso Paideia: "…era ne\\n\\n103. …\\n\\nnos discursos…" → frag "ne".
fn take_carry_before_first_footnote(body: &str) -> Option<(String, String)> {
    let lines: Vec<&str> = body.lines().collect();
    let foot_at = lines.iter().position(|l| line_starts_like_footnote(l.trim()))?;
    if foot_at == 0 {
        return None;
    }
    let prose = lines[..foot_at].join("\n");
    let rest = lines[foot_at..].join("\n");
    let (kept, frag) = word_carry_fragment(&prose)?;
    let mut out = kept;
    if !rest.is_empty() {
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        out.push_str(&rest);
    }
    Some((out, frag))
}

/// Parte linhas em coluna esquerda / direita pelo centro horizontal.
fn split_columns(lines: Vec<StreamLine>, page_w: f32) -> (Vec<StreamLine>, Vec<StreamLine>) {
    let mid = page_w * 0.5;
    let mut left = Vec::new();
    let mut right = Vec::new();
    for line in lines {
        let cx = (line.left + line.right) / 2.0;
        if cx < mid {
            left.push(line);
        } else {
            right.push(line);
        }
    }
    sort_body_visual(&mut left);
    sort_body_visual(&mut right);
    (left, right)
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

fn token_accepts_carry(token: &str) -> bool {
    let t = token.trim_start_matches(|c: char| !c.is_alphabetic());
    if t.is_empty() {
        return false;
    }
    t.chars().count() >= 5
        && t.chars()
            .next()
            .map(|c| c.is_lowercase() && c.is_alphabetic())
            .unwrap_or(false)
}

// Pendência aberta (handover 16/Ago, item 2): transporte de palavra partida na
// virada de página sem \x02. Já testada; será ligada no extract_pdf quando o
// carry for validado com casos reais.
#[allow(dead_code)]
fn next_accepts_letter_carry(next: &str) -> bool {
    let first = next.split_whitespace().next().unwrap_or("");
    token_accepts_carry(first)
}

fn token_looks_like_header(token: &str) -> bool {
    let alpha: String = token.chars().filter(|c| c.is_alphabetic()).collect();
    if alpha.is_empty() {
        return token.chars().any(|c| c.is_numeric());
    }
    alpha.chars().all(|c| c.is_uppercase()) && alpha.chars().count() <= 24
}

/// Cola `frag` só no **primeiro** token de prosa do corpo (pula nº/cabeçalho).
/// Não vasculha o meio da página — evita "ne"+"discursos".
fn merge_letter_carry(body: &str, frag: &str) -> Option<String> {
    if frag.is_empty() || body.is_empty() {
        return None;
    }
    let mut idx = 0usize;
    while idx < body.len() {
        let rest = &body[idx..];
        let ws = rest.len() - rest.trim_start().len();
        idx += ws;
        if idx >= body.len() {
            break;
        }
        let rest = &body[idx..];
        let token_len = rest
            .char_indices()
            .find(|(_, c)| c.is_whitespace())
            .map(|(i, _)| i)
            .unwrap_or(rest.len());
        let token = &rest[..token_len];
        if token_looks_like_header(token) {
            idx += token_len;
            continue;
        }
        // Primeiro token de prosa: cola ou desiste.
        if token_accepts_carry(token) {
            let mut out = String::with_capacity(body.len() + frag.len());
            out.push_str(&body[..idx]);
            out.push_str(frag);
            out.push_str(&body[idx..]);
            return Some(out);
        }
        return None;
    }
    None
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
    fn duas_colunas_ordem_leitura_esq_depois_dir() {
        // Ordem de stream bagunçada (direita primeiro no vetor).
        let lines = vec![
            line("dir-topo", 700.0, 300.0, 480.0),
            line("esq-baixo", 500.0, 10.0, 80.0),
            line("dir-meio", 600.0, 300.0, 480.0),
            line("esq-topo", 700.0, 10.0, 80.0),
            line("dir-baixo", 500.0, 300.0, 480.0),
            line("esq-meio", 600.0, 10.0, 80.0),
        ];
        assert!(!is_single_column(&lines, 500.0));
        let (left, right) = split_columns(lines, 500.0);
        assert_eq!(
            left.iter().map(|l| l.text.as_str()).collect::<Vec<_>>(),
            vec!["esq-topo", "esq-meio", "esq-baixo"]
        );
        assert_eq!(
            right.iter().map(|l| l.text.as_str()).collect::<Vec<_>>(),
            vec!["dir-topo", "dir-meio", "dir-baixo"]
        );
    }

    #[test]
    fn carry_entre_colunas_junta_ne_cessario() {
        let left = "texto da esquerda termina em era ne";
        let right = "cessário na coluna direita";
        let (kept, frag) = word_carry_fragment(left).unwrap();
        let merged = merge_letter_carry(right, &frag).unwrap();
        assert_eq!(kept, "texto da esquerda termina em era");
        assert!(merged.starts_with("necessário"));
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
    fn carry_pula_cabecalho_na_pagina_seguinte() {
        // Caso Paideia: fragmento não é o 1º token da página seguinte.
        let next = "145\nPAIDEIA\ncessário continuar a leitura do capítulo";
        let merged = merge_letter_carry(next, "ne").unwrap();
        assert!(merged.contains("necessário"));
        assert!(!merged.contains("ne cessário"));
    }

    #[test]
    fn carry_paideia_proxima_pagina_comeca_cessario() {
        let next = "cessário que ele se levantasse também pela última vez";
        let merged = merge_letter_carry(next, "ne").unwrap();
        assert!(merged.starts_with("necessário"));
    }

    #[test]
    fn carry_nao_cola_em_discursos() {
        let right = "nos discursos de Demóstenes a imortalidade. A tão admirada e";
        assert!(merge_letter_carry(right, "ne").is_none());
    }

    #[test]
    fn pending_carry_quando_direita_nao_aceita() {
        let left = "em nome da História, era ne\n\n103. ARISTÓTELES, Pol., VII.";
        let right = "nos discursos de Demóstenes a imortalidade. A tão admirada e";
        let prose = strip_trailing_footnote_lines(left);
        let (kept, frag) = word_carry_fragment(&prose).unwrap();
        assert_eq!(frag, "ne");
        assert!(kept.ends_with("era"));
        assert!(merge_letter_carry(right, &frag).is_none());
        let next = "cessário que ele se levantasse";
        let merged = merge_letter_carry(next, &frag).unwrap();
        assert!(merged.starts_with("necessário"));
    }

    #[test]
    fn take_carry_antes_da_nota_numerada() {
        let body = "em nome da História, era ne\n\n103. ARISTÓTELES, Pol.\n\nnos discursos e";
        let (out, frag) = take_carry_before_first_footnote(body).unwrap();
        assert_eq!(frag, "ne");
        assert!(out.contains("era\n\n103."));
        assert!(!out.contains("era ne"));
    }

    #[test]
    fn carry_nao_junta_um_elemento() {
        assert!(word_carry_fragment("era um").is_none());
        assert!(!next_accepts_letter_carry("a cidade"));
    }

    #[test]
    fn render_pagina_um_se_acervo_existir() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let pdf = root.join(
            "_originais/Pierre Leveque - As Primeiras Civilizações da Idade da Pedra aos Povos Semitas.pdf",
        );
        let pdfium = root.join("_APP/src-tauri/libs/lib/libpdfium.dylib");
        if !pdf.is_file() || !pdfium.is_file() {
            eprintln!("[render] acervo/pdfium ausentes — teste pulado");
            return;
        }
        let n = pdf_page_count(&pdfium, &pdf).expect("page count");
        assert!(n > 10);
        let png = render_page_png(&pdfium, &pdf, 1, Some(1.0)).expect("render");
        assert!(png.len() > 500);
        assert_eq!(&png[0..8], b"\x89PNG\r\n\x1a\n");
    }

    #[test]
    fn pull_notas_numeradas_do_meio_da_prosa() {
        let body = "Fim do parágrafo era.\n\n103. ARISTÓTELES, Pol., VII.\n\nNos discursos de Demóstenes a imortalidade continua longa o bastante.";
        let (prose, notes) = pull_numbered_notes(body);
        assert_eq!(notes.len(), 1);
        assert!(notes[0].starts_with("103."));
        assert!(!prose.contains("103."));
        assert!(prose.contains("Fim do parágrafo"));
        assert!(prose.contains("Nos discursos"));
    }

    #[test]
    fn tabela_duas_colunas_vira_markdown() {
        let body = "Nome    Idade\nAna     30\nBruno   25\n\nProsa depois.";
        let out = convert_tables_in_text(body);
        assert!(out.contains("| Nome | Idade |"));
        assert!(out.contains("| --- | --- |"));
        assert!(out.contains("| Ana | 30 |"));
        assert!(out.contains("Prosa depois."));
    }

    #[test]
    fn finalize_carrega_nota_e_pending() {
        let body = "História, era ne\n\n103. Nota curta.\n\nNos discursos seguem aqui com texto bem longo para contar como prosa.";
        let page = finalize_assembled(body.into(), Vec::new(), None);
        assert_eq!(page.pending_carry.as_deref(), Some("ne"));
        assert!(page.footnotes.contains("103."));
        assert!(!page.body.contains("103."));
        assert!(!page.body.contains("era ne"));
    }

    #[test]
    fn detecta_portugues_por_acentos() {
        let sample = "A civilização mesopotâmica não surgiu do nada. \
            Foi através da história que também se formou a noção de estado. \
            A população vivia em cidades com comércio e agricultura.";
        assert_eq!(detect_ocr_languages(sample), "por");
    }

    #[test]
    fn detecta_ingles_por_palavras() {
        let sample = "The history of this region and the people which came from \
            the east with that culture. This and that and from which the land \
            was settled with trade and farming across the plains.";
        assert_eq!(detect_ocr_languages(sample), "eng");
    }

    #[test]
    fn amostra_curta_cai_em_por_eng() {
        assert_eq!(detect_ocr_languages("oi"), "por+eng");
    }

    #[test]
    fn check_cancel_respeita_flag() {
        assert!(check_cancel(None).is_ok());
        assert!(check_cancel(Some(&|| false)).is_ok());
        let err = check_cancel(Some(&|| true)).unwrap_err();
        assert_eq!(err, CANCELLED);
    }

    // --- normalize_ocr_page_text (evidência Schopenhauer I, 16/Ago) ---

    #[test]
    fn ocr_norm_remove_barras_orfas() {
        let raw = "sofo de Frankfurt já repercutia |\n|\ncomo re- |\npresentação";
        let out = normalize_ocr_page_text(raw);
        assert_eq!(out, "sofo de Frankfurt já repercutia\ncomo re-\npresentação");
    }

    #[test]
    fn ocr_norm_junta_entrelinha_larga() {
        // Padrão real da pág. 12: linha em branco após CADA linha.
        let raw = "da ação humana não apenas no domínio de sua significação usual que le-\n\nva o egoísmo ou a malvadeza a darem as cartas nos relacionamentos hu-\n\nmanos, mas sobretudo daquela ação praticada por ascetas e santos, que\n";
        let out = normalize_ocr_page_text(raw);
        assert!(!out.contains("\n\n"), "brancos espúrios deviam sumir: {out:?}");
        assert!(out.contains("le-\nva"), "hífen deve ficar adjacente: {out:?}");
    }

    #[test]
    fn ocr_norm_preserva_paragrafo_legitimo() {
        // Fim de sentença + próxima linha maiúscula = parágrafo de verdade.
        let raw = "Sua confiança naquela forma foi completa.\n\nO que Nietzsche diz traduz boa parte\n\nda experiência vivida.";
        let out = normalize_ocr_page_text(raw);
        assert!(out.contains("completa.\n\nO que"), "parágrafo real preservado: {out:?}");
        assert!(out.contains("parte\nda experiência"), "continuação minúscula junta: {out:?}");
    }

    #[test]
    fn ocr_norm_preserva_titulo_seguido_de_maiuscula() {
        let raw = "Apresentação\n\nUm livro que embriaga";
        let out = normalize_ocr_page_text(raw);
        assert_eq!(out, "Apresentação\n\nUm livro que embriaga");
    }
}
