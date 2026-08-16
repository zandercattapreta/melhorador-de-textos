// ==============================================================================
// SCRIPT: blocks.rs (melhorador-core)
// DESCRIÇÃO: Marca blocos especiais (ficha, bibliografia, figura) — só enhanced
// CHAMADO POR: lib.rs clean_and_structure_enhanced; testes unitários
// CONTRATO (RESPOSTA ESPERADA): mesmo texto + comentários HTML / [figura]; zero invenção
// ==============================================================================

//! Heurísticas determinísticas para delimitar blocos que não são prosa corrida.
//! Marcadores: `<!-- ficha -->` / `<!-- bibliografia -->` (não viram heading inventado).
//! Figura: placeholder `[figura]` — sem descrição.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use regex::Regex;

/// Início de seção bibliográfica (texto já na fonte, só detectamos).
const BIB_MARKERS: &[&str] = &[
    "bibliografia",
    "referencias",
    "referências",
    "obras citadas",
    "obras consultadas",
    "fontes",
];

/// Fragmentos típicos de ficha / colofão.
const FICHA_FOLDS: &[&str] = &[
    "isbn",
    "todos os direitos",
    "copyright",
    "deposito legal",
    "depósito legal",
    "dados internacionais",
    "ficha catalog",
    "catalogacao",
    "catalogação",
];

fn fold(s: &str) -> String {
    s.chars()
        .map(|c| c.to_lowercase().next().unwrap_or(c))
        .collect::<String>()
}

fn isbn_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)ISBN\s*[\d\- ]{10,22}").unwrap())
}

fn split_paras(text: &str) -> Vec<String> {
    text.split("\n\n")
        .map(|p| p.to_string())
        .collect()
}

fn join_paras(paras: &[String]) -> String {
    paras.join("\n\n")
}

fn looks_like_ficha(para: &str) -> bool {
    let t = para.trim();
    if t.is_empty() {
        return false;
    }
    if isbn_re().is_match(t) {
        return true;
    }
    let f = fold(t);
    FICHA_FOLDS.iter().any(|k| f.contains(k))
}

fn is_bib_heading(para: &str) -> bool {
    let t = para.trim();
    // Heading markdown já existente ou linha curta igual ao marcador.
    let core = t.trim_start_matches('#').trim();
    if core.chars().count() > 60 {
        return false;
    }
    let f = fold(core);
    BIB_MARKERS.iter().any(|m| f == *m || f.starts_with(&format!("{m} ")))
}

fn is_markdown_h1(para: &str) -> bool {
    let t = para.trim_start();
    t.starts_with("# ") && !t.starts_with("## ")
}

/// Extrai ISBN-13/10 normalizado (só dígitos), se houver — port leve do CLI.
pub fn parse_isbn(text: &str) -> Option<String> {
    let re13 = Regex::new(r"(?i)ISBN\s*(978)[-\s]?(\d{1,5})[-\s]?(\d{1,7})[-\s]?(\d{1,7})[-\s]?(\d)")
        .ok()?;
    if let Some(c) = re13.captures(text) {
        let digits: String = (1..=5).map(|i| c[i].to_string()).collect::<Vec<_>>().join("");
        let only: String = digits.chars().filter(|c| c.is_ascii_digit()).collect();
        if only.len() == 13 {
            return Some(only);
        }
    }
    let re = Regex::new(r"(?i)ISBN\s*[-\s]?([\d\- ]{10,17})").ok()?;
    let caps = re.captures(text)?;
    let only: String = caps[1].chars().filter(|c| c.is_ascii_digit()).collect();
    if only.len() == 10 || only.len() == 13 {
        Some(only)
    } else {
        None
    }
}

/// Página (ou trecho) sem prosa útil → candidato a figura.
pub fn is_near_empty_page(text: &str) -> bool {
    text.chars().filter(|c| c.is_alphanumeric()).count() < 20
}

/// Substitui páginas `\f` quase vazias por `[figura]`.
fn annotate_empty_pages(text: &str) -> (String, i64) {
    if !text.contains('\u{0c}') {
        return (text.to_string(), 0);
    }
    let mut n = 0i64;
    let parts: Vec<String> = text
        .split('\u{0c}')
        .map(|page| {
            if is_near_empty_page(page) {
                n += 1;
                "[figura]".to_string()
            } else {
                page.to_string()
            }
        })
        .collect();
    (parts.join("\u{0c}"), n)
}

/// Envolve o primeiro bloco contínuo de parágrafos tipo ficha.
fn annotate_ficha(text: &str) -> (String, i64) {
    let paras = split_paras(text);
    if paras.is_empty() {
        return (text.to_string(), 0);
    }
    let mut start = None;
    let mut end = None;
    for (i, p) in paras.iter().enumerate() {
        if looks_like_ficha(p) {
            if start.is_none() {
                start = Some(i);
            }
            end = Some(i);
        } else if start.is_some() && end.is_some() {
            // Permite no máximo 1 parágrafo “ponte” curto dentro do bloco.
            let gap = i.saturating_sub(end.unwrap());
            if gap > 1 || p.trim().chars().count() > 120 {
                break;
            }
        }
    }
    let (Some(s), Some(e)) = (start, end) else {
        return (text.to_string(), 0);
    };
    if text.contains("<!-- ficha -->") {
        return (text.to_string(), 0);
    }
    let mut out = Vec::new();
    for (i, p) in paras.iter().enumerate() {
        if i == s {
            out.push("<!-- ficha -->".to_string());
        }
        out.push(p.clone());
        if i == e {
            out.push("<!-- /ficha -->".to_string());
        }
    }
    (join_paras(&out), 1)
}

/// Marca da linha de bibliografia até o próximo H1 (ou fim).
fn annotate_bibliography(text: &str) -> (String, i64) {
    let paras = split_paras(text);
    let start = paras.iter().position(|p| is_bib_heading(p));
    let Some(s) = start else {
        return (text.to_string(), 0);
    };
    if text.contains("<!-- bibliografia -->") {
        return (text.to_string(), 0);
    }
    let end = (s + 1..paras.len())
        .find(|&i| is_markdown_h1(&paras[i]))
        .unwrap_or(paras.len());
    let mut out = Vec::new();
    for (i, p) in paras.iter().enumerate() {
        if i == s {
            out.push("<!-- bibliografia -->".to_string());
        }
        out.push(p.clone());
        if end > s && i + 1 == end {
            out.push("<!-- /bibliografia -->".to_string());
        }
    }
    (join_paras(&out), 1)
}

/// Aplica todos os marcadores de bloco (modo aprimorado).
pub fn annotate_blocks(text: &str) -> (String, BTreeMap<&'static str, i64>) {
    let mut stats = BTreeMap::new();
    let (mut t, figs) = annotate_empty_pages(text);
    stats.insert("figure_placeholders", figs);
    let (t2, bib) = annotate_bibliography(&t);
    t = t2;
    stats.insert("bibliography_blocks", bib);
    let (t3, ficha) = annotate_ficha(&t);
    stats.insert("ficha_blocks", ficha);
    (t3, stats)
}

/// Placeholder de página sem texto útil (extração nativa).
pub fn figure_placeholder_if_empty(page_text: &str) -> String {
    if is_near_empty_page(page_text) {
        "[figura]".into()
    } else {
        page_text.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_isbn_13() {
        let isbn = parse_isbn("Dados: ISBN 978-85-7827-670-6 resto").unwrap();
        assert_eq!(isbn, "9788578276706");
    }

    #[test]
    fn marca_ficha_com_isbn() {
        let text = "Prefácio curto.\n\nISBN 978-85-0000-000-0\nTodos os direitos reservados.\n\nCapítulo um segue aqui com prosa longa.";
        let (out, stats) = annotate_blocks(text);
        assert_eq!(stats["ficha_blocks"], 1);
        assert!(out.contains("<!-- ficha -->"));
        assert!(out.contains("<!-- /ficha -->"));
        assert!(out.contains("ISBN 978-85-0000-000-0"));
    }

    #[test]
    fn marca_bibliografia() {
        let text = "# Parte 1\n\nProsa.\n\nBibliografia\n\nSOBRENOME, Nome. Livro.\n\n# Parte 2\n\nMais prosa.";
        let (out, stats) = annotate_blocks(text);
        assert_eq!(stats["bibliography_blocks"], 1);
        assert!(out.contains("<!-- bibliografia -->"));
        assert!(out.contains("<!-- /bibliografia -->"));
        assert!(out.contains("# Parte 2"));
    }

    #[test]
    fn pagina_vazia_vira_figura() {
        let text = "Prosa boa com bastante texto aqui.\u{0c}\n\n\u{0c}Mais prosa no fim.";
        let (out, stats) = annotate_blocks(text);
        assert!(stats["figure_placeholders"] >= 1);
        assert!(out.contains("[figura]"));
    }

    #[test]
    fn figure_placeholder_helper() {
        assert_eq!(figure_placeholder_if_empty("  \n"), "[figura]");
        assert!(figure_placeholder_if_empty("Um parágrafo com conteúdo real suficiente.").starts_with("Um"));
    }
}
