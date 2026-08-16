// ==============================================================================
// SCRIPT: structure.rs (txtmelhorator-core)
// DESCRIÇÃO: Headings H1–H4 e SUMÁRIO — port fiel de _CLI structure.py
// CHAMADO POR: lib.rs; pipeline do app; tests/golden.rs
// CONTRATO (RESPOSTA ESPERADA): mesmas saídas do structure.py, byte a byte
// ==============================================================================

//! Estruturação Markdown determinística (sem IA): classifica parágrafos já
//! limpos em headings (#–####), entradas de sumário ou prosa.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use regex::Regex;
use unicode_normalization::char::is_combining_mark;
use unicode_normalization::UnicodeNormalization;

use crate::pystr::{py_split_ws, py_strip};

/// Limite de caracteres para "título curto" (igual ao Python).
const HEADING_MAX_CHARS: usize = 90;

/// Palavras que abrem bloco de sumário / índice.
const TOC_MARKERS: [&str; 7] = [
    "sumario",
    "sumário",
    "indice",
    "índice",
    "conteudo",
    "conteúdo",
    "table of contents",
];

/// Fragmentos de página de créditos/colofão — não são títulos.
const COLOPHON_FOLDS: [&str; 8] = [
    "deposito legal",
    "paginacao",
    "impressao",
    "acabamento",
    "design de capa",
    "isbn",
    "copyright",
    "todos os direitos",
];

/// Nota: o Python tem também "titulo original" na lista de colofão.
const COLOPHON_EXTRA: &str = "titulo original";

/// Títulos de seção sem numeração, curtos e conhecidos (PT editorial).
const NAMED_H1: [&str; 14] = [
    "preambulo",
    "preâmbulo",
    "introducao",
    "introdução",
    "conclusao",
    "conclusão",
    "epilogo",
    "epílogo",
    "anexo",
    "apendice",
    "apêndice",
    "bibliografia",
    "agradecimentos",
    "nota do autor",
];

/// Complemento do NAMED_H1 (o array acima ficou no limite de linhas).
const NAMED_H1_EXTRA: &str = "nota do tradutor";

#[derive(Debug)]
pub struct StructureResult {
    pub text: String,
    pub stats: BTreeMap<&'static str, i64>,
}

fn numbered_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^(?:(?P<num_multi>\d+(?:\.\d+){1,3})\s+|(?P<num_simple>\d+)\.\s+)(?P<title>.+)$")
            .unwrap()
    })
}

fn toc_entry_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^.+?[\.…]{2,}\s*\d{1,4}\s*$").unwrap())
}

fn roman_ii_ocr_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^IL\s*[.\-–—]\s*").unwrap())
}

/// Normaliza para comparação (NFD sem marcas + lowercase) — _fold do Python.
fn fold(text: &str) -> String {
    py_strip(text)
        .nfd()
        .filter(|c| !is_combining_mark(*c))
        .collect::<String>()
        .to_lowercase()
}

fn looks_like_colophon(text: &str) -> bool {
    let folded = fold(text);
    COLOPHON_FOLDS.iter().any(|m| folded.contains(m)) || folded.contains(COLOPHON_EXTRA)
}

/// Proporção de letras maiúsculas entre as letras do texto.
fn letter_ratio_upper(text: &str) -> f64 {
    let letters: Vec<char> = text.chars().filter(|c| c.is_alphabetic()).collect();
    if letters.is_empty() {
        return 0.0;
    }
    let upper = letters.iter().filter(|c| c.is_uppercase()).count();
    upper as f64 / letters.len() as f64
}

fn is_toc_marker(para: &str) -> bool {
    let folded = fold(para);
    if TOC_MARKERS.contains(&folded.as_str()) {
        return true;
    }
    TOC_MARKERS
        .iter()
        .any(|m| folded == *m || folded.starts_with(&format!("{m} ")))
}

fn is_named_h1(para: &str) -> bool {
    let folded = fold(para);
    NAMED_H1.contains(&folded.as_str()) || folded == NAMED_H1_EXTRA
}

/// '1' → 2, '1.2' → 3, '1.2.3' → 4 (H2–H4).
fn numbered_level(num: &str) -> usize {
    let depth = num.matches('.').count() + 1;
    (depth + 1).min(4)
}

/// Subtítulo curto isolado entre blocos de prosa (decisão por posição).
fn is_title_case_subheading(text: &str, previous: Option<&str>, following: Option<&str>) -> bool {
    let n = text.chars().count();
    if !(8..=50).contains(&n) {
        return false;
    }
    let mut chars = text.chars();
    let first = chars.next().unwrap();
    let last = text.chars().last().unwrap();
    if !first.is_uppercase() || ".!?:;,".contains(last) {
        return false;
    }
    // Python: any(char.isdigit() ...) — dígitos decimais (Nd) cobrem o corpus.
    if text.chars().any(|c| c.is_ascii_digit() || c.is_numeric()) || looks_like_colophon(text) {
        return false;
    }
    let words = py_split_ws(text);
    if !(2..=8).contains(&words.len()) {
        return false;
    }
    let Some(following) = following else { return false };
    if following.chars().count() < 120 {
        return false;
    }
    let Some(previous) = previous else { return false };
    previous.starts_with('#') || previous.chars().count() >= 120
}

/// Correção OCR comprovável de numeral romano ("IL." → "II. — ").
fn normalize_heading_ocr(text: &str) -> (String, bool) {
    if roman_ii_ocr_re().is_match(text) {
        (roman_ii_ocr_re().replace(text, "II. — ").into_owned(), true)
    } else {
        (text.to_string(), false)
    }
}

/// Classifica um parágrafo → (nível, ainda_em_toc).
/// Some(1..=4) → heading; Some(0) → entrada de sumário; None → prosa.
fn classify_paragraph(para: &str, in_toc: bool) -> (Option<usize>, bool) {
    let text = py_strip(para);
    if text.is_empty() {
        return (None, in_toc);
    }
    let n = text.chars().count();

    // --- Bloco SUMÁRIO / ÍNDICE ---
    if is_toc_marker(text) {
        return (Some(1), true);
    }
    if in_toc {
        if toc_entry_re().is_match(text) || n <= HEADING_MAX_CHARS {
            return (Some(0), true);
        }
        return (None, false);
    }

    // --- H1 por nome conhecido ---
    if n <= HEADING_MAX_CHARS && is_named_h1(text) {
        return (Some(1), false);
    }

    // --- H1 por caixa alta ---
    if (8..=70).contains(&n)
        && letter_ratio_upper(text) >= 0.75
        && !text.ends_with('.')
        && !text.contains(';')
        && !looks_like_colophon(text)
    {
        return (Some(1), false);
    }

    // --- H2–H4 numerados curtos ---
    if n <= HEADING_MAX_CHARS {
        if let Some(caps) = numbered_re().captures(text) {
            let title = caps.name("title").unwrap().as_str();
            let num = caps
                .name("num_multi")
                .or_else(|| caps.name("num_simple"))
                .map(|m| m.as_str());
            if let Some(num) = num {
                if title.chars().any(char::is_alphabetic) {
                    return (Some(numbered_level(num)), false);
                }
            }
        }
    }

    (None, in_toc)
}

/// Entrada de sumário "à francesa": "Título, 312" / "Título, 312; Sub, 313"
/// (também aceita o pontilhado clássico). Usada no modo aprimorado para
/// ENTRAR em modo sumário quando o marcador ("Índice") veio colado a outra
/// linha e não formou parágrafo próprio.
fn is_toc_entry_like(text: &str) -> bool {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"(?:(?:[.…]\s*){2,}|,)\s*\d{1,4}\s*;?\s*$").unwrap()
    });
    text.chars().count() <= 90 && re.is_match(text)
}

/// Parágrafo que começa com marcador tipográfico de lista → item Markdown.
/// (Travessão — fica de fora: em PT é marca de diálogo, não de lista.)
fn bullet_item(text: &str) -> Option<&str> {
    for marker in ['•', '●', '▪', '‣', '◦', '*'] {
        if let Some(rest) = text.strip_prefix(marker) {
            let rest = rest.trim_start();
            if !rest.is_empty() {
                return Some(rest);
            }
        }
    }
    None
}

/// Título numerado que termina em ponto final é quase sempre NOTA DE RODAPÉ
/// incrustada ("4. Wilamowitz, op. cit., p. 178.") — não título.
fn looks_like_footnote(text: &str) -> bool {
    text.ends_with('.') && numbered_re().is_match(text)
}

/// Modo paridade (idêntico ao CLI Python).
pub fn apply_structure(text: &str) -> StructureResult {
    apply_structure_impl(text, false)
}

/// Modo APRIMORADO do app: guarda anti-nota-de-rodapé em títulos numerados
/// e listas por marcador tipográfico. Diverge do CLI de propósito.
pub fn apply_structure_enhanced(text: &str) -> StructureResult {
    apply_structure_impl(text, true)
}

/// Cauda de entrada de sumário partida: só líderes + número (", 312" / ".... 17").
fn is_toc_page_tail(text: &str) -> bool {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"^(?:[\.…]{2,}\s*|,)\s*\d{1,4}\s*;?\s*$").unwrap());
    re.is_match(text)
}

/// Junta título de sumário com a linha seguinte que só traz o número da página.
fn merge_split_toc_entries(paras: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    while i < paras.len() {
        let cur = paras[i].as_str();
        if let Some(nxt) = paras.get(i + 1) {
            if !is_toc_entry_like(cur)
                && cur.chars().count() <= HEADING_MAX_CHARS
                && is_toc_page_tail(nxt)
            {
                out.push(format!("{cur} {nxt}"));
                i += 2;
                continue;
            }
        }
        out.push(paras[i].clone());
        i += 1;
    }
    out
}

fn apply_structure_impl(text: &str, enhanced: bool) -> StructureResult {
    let raw: Vec<String> = text
        .split("\n\n")
        .map(|p| py_strip(p).to_string())
        .filter(|p| !p.is_empty())
        .collect();
    let nonempty: Vec<String> = if enhanced {
        merge_split_toc_entries(&raw)
    } else {
        raw
    };

    let mut out: Vec<String> = Vec::new();
    let mut in_toc = false;
    let mut counts: BTreeMap<&'static str, i64> = BTreeMap::new();
    for key in [
        "h1",
        "h2",
        "h3",
        "h4",
        "toc_entries",
        "prose",
        "title_case_headings",
        "heading_ocr_corrections",
        "list_items",
    ] {
        counts.insert(key, 0);
    }

    for (index, stripped) in nonempty.iter().enumerate() {
        let stripped = stripped.as_str();
        let previous: Option<String> = out.last().cloned();
        let following: Option<&str> = nonempty.get(index + 1).map(|s| s.as_str());

        // Aprimorado: lista por marcador tipográfico tem precedência.
        if enhanced && !in_toc {
            if let Some(item) = bullet_item(stripped) {
                out.push(format!("- {item}"));
                *counts.get_mut("list_items").unwrap() += 1;
                continue;
            }
        }

        // Aprimorado: 3+ entradas de sumário consecutivas ligam o modo
        // sumário mesmo sem o marcador em parágrafo próprio.
        if enhanced
            && !in_toc
            && is_toc_entry_like(stripped)
            && nonempty.get(index + 1).map_or(false, |n| is_toc_entry_like(n.as_str()))
            && nonempty.get(index + 2).map_or(false, |n| is_toc_entry_like(n.as_str()))
        {
            in_toc = true;
        }

        let (mut level, new_in_toc) = classify_paragraph(stripped, in_toc);
        in_toc = new_in_toc;

        // Aprimorado: nota de rodapé incrustada não vira título numerado.
        if enhanced {
            if let Some(lv) = level {
                if (2..=4).contains(&lv) && looks_like_footnote(stripped) {
                    level = None;
                }
            }
        }
        if level.is_none()
            && !in_toc
            && is_title_case_subheading(stripped, previous.as_deref(), following)
        {
            level = Some(2);
            *counts.get_mut("title_case_headings").unwrap() += 1;
        }

        match level {
            Some(0) => {
                out.push(format!("- {stripped}"));
                *counts.get_mut("toc_entries").unwrap() += 1;
            }
            Some(lv) => {
                let (normalized, corrected) = normalize_heading_ocr(stripped);
                out.push(format!("{} {}", "#".repeat(lv), normalized));
                let key = match lv {
                    1 => "h1",
                    2 => "h2",
                    3 => "h3",
                    _ => "h4",
                };
                *counts.get_mut(key).unwrap() += 1;
                *counts.get_mut("heading_ocr_corrections").unwrap() += i64::from(corrected);
            }
            None => {
                out.push(stripped.to_string());
                *counts.get_mut("prose").unwrap() += 1;
            }
        }
    }

    let text = if out.is_empty() {
        String::new()
    } else {
        format!("{}\n", out.join("\n\n"))
    };
    StructureResult { text, stats: counts }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Casos espelhados de _CLI/tests/test_structure.py.

    #[test]
    fn sumario_vira_lista() {
        let text = "SUMÁRIO\n\nIntrodução ........ 9\n\nCapítulo um ..... 21";
        let result = apply_structure(text);
        assert!(result.text.starts_with("# SUMÁRIO"));
        assert!(result.text.contains("- Introdução ........ 9"));
        assert_eq!(result.stats["toc_entries"], 2);
    }

    #[test]
    fn caixa_alta_vira_h1() {
        let result = apply_structure("AS PRIMEIRAS CIVILIZAÇÕES\n\nO texto do capítulo segue aqui.");
        assert!(result.text.starts_with("# AS PRIMEIRAS CIVILIZAÇÕES"));
        assert_eq!(result.stats["h1"], 1);
    }

    #[test]
    fn numerado_vira_h2_h3_h4() {
        let result = apply_structure("1. O começo\n\n1.2 O meio\n\n1.2.3 O detalhe");
        assert!(result.text.contains("## 1. O começo"));
        assert!(result.text.contains("### 1.2 O meio"));
        assert!(result.text.contains("#### 1.2.3 O detalhe"));
    }

    #[test]
    fn prosa_numerada_longa_nao_e_heading() {
        let long = format!("1. {}", "palavra ".repeat(20));
        let result = apply_structure(&long);
        assert_eq!(result.stats["prose"], 1);
        assert_eq!(result.stats["h2"], 0);
    }

    #[test]
    fn colofao_nao_vira_titulo() {
        let result = apply_structure("ISBN 978-85-0000-000-0 TODOS OS DIREITOS");
        assert_eq!(result.stats["h1"], 0);
    }

    #[test]
    fn aprimorado_rodape_numerado_nao_e_titulo() {
        let nota = "4. Wilamowitz, op. cit., p. 178.";
        // Paridade: vira h2 (fiel ao CLI). Aprimorado: prosa.
        assert_eq!(apply_structure(nota).stats["h2"], 1);
        let enh = apply_structure_enhanced(nota);
        assert_eq!(enh.stats["h2"], 0);
        assert_eq!(enh.stats["prose"], 1);
        // Título numerado legítimo (sem ponto final) segue vivo nos dois.
        assert_eq!(apply_structure_enhanced("1. O começo").stats["h2"], 1);
    }

    #[test]
    fn aprimorado_marcadores_viram_lista() {
        let enh = apply_structure_enhanced("• primeiro item\n\n• segundo item\n\nProsa normal.");
        assert!(enh.text.contains("- primeiro item"));
        assert_eq!(enh.stats["list_items"], 2);
        assert_eq!(enh.stats["prose"], 1);
    }

    #[test]
    fn aprimorado_sumario_multilinha_nao_vira_h2() {
        let text = "SUMÁRIO\n\nCapítulo primeiro — A cidade\n\n.............. 17\n\nIntrodução ........ 9";
        // Paridade: a linha sem número pode virar título. Aprimorado junta.
        let enh = apply_structure_enhanced(text);
        assert!(!enh.text.contains("## Capítulo primeiro"));
        assert!(enh.text.contains("- Capítulo primeiro — A cidade .............. 17"));
        assert!(enh.text.contains("- Introdução ........ 9"));
        assert_eq!(enh.stats["toc_entries"], 2);
    }

    #[test]
    fn correcao_ocr_il_para_ii() {
        let result = apply_structure("IL - A GUERRA\n\nO texto do capítulo segue.");
        assert!(result.text.contains("# II. — A GUERRA"));
        assert_eq!(result.stats["heading_ocr_corrections"], 1);
    }
}
