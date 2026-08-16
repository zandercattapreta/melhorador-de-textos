// ==============================================================================
// SCRIPT: metadata.rs (melhorador-core)
// DESCRIÇÃO: Ficha catalográfica → autor/título/ISBN/slug (port do CLI)
// CHAMADO POR: app ao salvar; testes
// CONTRATO (RESPOSTA ESPERADA): BookMeta com slug estável; fallback = filename
// ==============================================================================

use regex::Regex;
use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BookMeta {
    pub source: String,
    pub author: Option<String>,
    pub title: Option<String>,
    pub isbn: Option<String>,
    pub slug: String,
    pub confidence: f32,
}

/// Extrai metadados do texto das primeiras páginas (já OCR/nativo).
pub fn extract_book_meta(front_text: &str, filename_stem: &str) -> BookMeta {
    let author = parse_author(front_text);
    let title = parse_title(front_text);
    let isbn = parse_isbn(front_text);

    if author.is_some() && title.is_some() && isbn.is_some() {
        let slug = normalize_slug(author.as_deref(), title.as_deref(), isbn.as_deref());
        return BookMeta {
            source: "ficha_catalografica".into(),
            author,
            title,
            isbn,
            slug,
            confidence: 0.85,
        };
    }
    if author.is_some() && isbn.is_some() && title.is_none() {
        let slug = normalize_slug(author.as_deref(), None, isbn.as_deref());
        return BookMeta {
            source: "ficha_catalografica".into(),
            author,
            title: None,
            isbn,
            slug,
            confidence: 0.73,
        };
    }

    let slug = clean_slug_part(filename_stem, 120);
    BookMeta {
        source: "filename".into(),
        author: None,
        title: None,
        isbn: None,
        slug,
        confidence: 0.55,
    }
}

fn parse_author(text: &str) -> Option<String> {
    let re = Regex::new(
        r"([A-Za-záéíóúäëïöüâêõã](?:\s?[A-Za-záéíóúäëïöüâêõã])*)\s*,\s*([A-Za-záéíóúäëïöüâêõã](?:\s?[A-Za-záéíóúäëïöüâêõã])*)\s*,?\s*\d{4}-\d{4}",
    )
    .ok()?;
    let caps = re.captures(text)?;
    let last = collapse_spaces(caps.get(1)?.as_str());
    let first = collapse_spaces(caps.get(2)?.as_str());
    if first.contains('\n') || last.contains('\n') {
        return None;
    }
    Some(format!("{first} {last}"))
}

fn parse_title(text: &str) -> Option<String> {
    let re = Regex::new(
        r"([A-ZÀ-Ÿ][a-záéíóúäëïöüâêõã]{2,}(?:\s+[a-záéíóúäëïöüâêõã]+)*)\s*:\s*([a-záéíóúäëïöüâêõã\s/\-]{5,})",
    )
    .ok()?;
    let caps = re.captures(text)?;
    let title = caps.get(1)?.as_str().trim();
    let subtitle: String = caps.get(2)?.as_str().trim().chars().take(80).collect();
    if title.len() <= 3 || title.to_uppercase().contains("ISBN") {
        return None;
    }
    Some(format!("{title} : {subtitle}"))
}

fn parse_isbn(text: &str) -> Option<String> {
    let re13 = Regex::new(
        r"(?i)ISBN\s*978[-\s]?([0-9]{1,5})[-\s]?([0-9]{1,5})[-\s]?([0-9]{1,5})[-\s]?([0-9])",
    )
    .ok()?;
    if let Some(c) = re13.captures(text) {
        let isbn = format!(
            "978{}{}{}{}",
            c.get(1)?.as_str(),
            c.get(2)?.as_str(),
            c.get(3)?.as_str(),
            c.get(4)?.as_str()
        );
        if isbn.len() >= 13 {
            return Some(isbn);
        }
    }
    None
}

fn normalize_slug(author: Option<&str>, title: Option<&str>, isbn: Option<&str>) -> String {
    let mut parts = Vec::new();
    if let Some(a) = author {
        parts.push(clean_slug_part(a, 30));
    }
    if let Some(t) = title {
        parts.push(clean_slug_part(t, 40));
    }
    if let Some(i) = isbn {
        parts.push(i.chars().take(10).collect());
    }
    let slug = parts.join("-");
    slug.chars().take(80).collect()
}

fn clean_slug_part(text: &str, max_len: usize) -> String {
    let ascii: String = text.nfkd().filter(|c| c.is_ascii()).collect();
    let lower = ascii.to_lowercase();
    let mut out = String::new();
    let mut prev_hyphen = false;
    for c in lower.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
            prev_hyphen = false;
        } else if !prev_hyphen {
            out.push('-');
            prev_hyphen = true;
        }
    }
    let trimmed = out.trim_matches('-').to_string();
    trimmed.chars().take(max_len).collect()
}

fn collapse_spaces(s: &str) -> String {
    let re = Regex::new(r"(\w)\s+([a-záéíóúäëïöüâêõã])").unwrap();
    re.replace_all(s.trim(), "$1$2").into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn isbn_e_slug_de_filename() {
        let t = "ISBN 978-85-1234-567-8\n";
        let m = extract_book_meta(t, "Pierre Leveque - Civilizacoes");
        assert_eq!(m.source, "filename"); // sem autor+título juntos
        assert!(!m.slug.is_empty());
    }

    #[test]
    fn slug_limpo() {
        assert_eq!(clean_slug_part("Paidéia!", 20), "paideia");
    }
}
