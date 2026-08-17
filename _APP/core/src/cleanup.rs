// ==============================================================================
// SCRIPT: cleanup.rs (txtmelhorator-core)
// DESCRIÇÃO: Limpeza determinística de texto OCR — port fiel de _CLI cleanup.py
// CHAMADO POR: lib.rs; pipeline do app; tests/golden.rs
// CONTRATO (RESPOSTA ESPERADA): mesmas saídas do cleanup.py, byte a byte
// ==============================================================================

//! Port completo do pipeline de 9 passos do `clean_text` Python.
//! Regra de ouro: só estabilizar e reformatar — nunca completar ou reescrever
//! conteúdo do livro. Validado byte a byte pelos golden masters (tests/golden.rs).

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::sync::OnceLock;

use fancy_regex::Regex as FancyRegex;
use regex::Regex;
use unicode_normalization::char::is_combining_mark;
use unicode_normalization::UnicodeNormalization;

use crate::pydifflib;
use crate::pystr::{py_rstrip, py_splitlines, py_strip};

/// Caracteres invisíveis/indesejados comuns em extração de PDF e OCR.
/// Mesmo conjunto do Python (_INVISIBLE_CHARS).
const INVISIBLE_CHARS: [char; 6] = [
    '\u{200b}', // zero-width space
    '\u{200c}', // zero-width non-joiner
    '\u{200d}', // zero-width joiner
    '\u{feff}', // BOM / zero-width no-break space
    '\u{00ad}', // soft hyphen
    '\u{180e}', // mongolian vowel separator
];

/// Resultado da limpeza com métricas para auditoria (espelha CleanupResult).
#[derive(Debug)]
pub struct CleanupResult {
    pub text: String,
    pub stats: BTreeMap<&'static str, i64>,
    pub warnings: Vec<String>,
}

// ---------------------------------------------------------------------------
// Regexes (compiladas uma vez, como as constantes de módulo do Python)
// ---------------------------------------------------------------------------

fn page_marker_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?m)^\s*--\s*\d+\s+of\s+\d+\s*--\s*$").unwrap())
}

fn page_number_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^\d{1,4}$").unwrap())
}

fn edge_garbage_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[^\wÀ-ÿ]$").unwrap())
}

fn line_hyphen_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    // Fim de linha (\n ou \r\n): "pala-\nvra" → "palavra"
    RE.get_or_init(|| Regex::new(r"([A-Za-zÀ-ÿ]+)-\r?\n([a-zà-ÿ])").unwrap())
}

fn ocr_garbage_line_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^\s*[—-]*<[-—<>]+.*$").unwrap())
}

fn leading_page_number_re() -> &'static FancyRegex {
    static RE: OnceLock<FancyRegex> = OnceLock::new();
    RE.get_or_init(|| FancyRegex::new(r"^\s*\d{1,4}\s+(?=[a-zà-ÿ])").unwrap())
}

// ---------------------------------------------------------------------------
// Passo 1 (aprox. ftfy no nosso corpus) + Passo 2 (NFC)
// ---------------------------------------------------------------------------

/// Equivalente observado do ftfy.fix_text no corpus de referência:
/// destorce aspas curvas (uncurl_quotes). Diagnóstico de 15/Ago sobre os
/// 4 livros mostrou que TODAS as mudanças do ftfy são estas substituições
/// (o caso Ω→Ω é canônico e o NFC do passo 2 cobre).
pub fn uncurl_quotes(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '\u{201c}' | '\u{201d}' => '"', // “ ”
            '\u{2018}' | '\u{2019}' => '\'', // ‘ ’
            other => other,
        })
        .collect()
}

/// unicodedata.normalize("NFC", ...) do Python.
pub fn nfc(text: &str) -> String {
    text.nfc().collect()
}

// ---------------------------------------------------------------------------
// Passo 3 — marcadores "-- N of M --"
// ---------------------------------------------------------------------------

/// Remove marcadores de página do sidecar. Retorna (texto, removidos).
pub fn remove_page_markers(text: &str) -> (String, usize) {
    let count = page_marker_re().find_iter(text).count();
    let result = page_marker_re().replace_all(text, "");
    (result.into_owned(), count)
}

// ---------------------------------------------------------------------------
// Passo 4 — cabeçalhos correntes, números de página, dedup de scans
// ---------------------------------------------------------------------------

/// Normaliza linha para comparar cabeçalhos (NFD sem marcas + casefold).
fn fold_line(line: &str) -> String {
    py_strip(line)
        .nfd()
        .filter(|c| !is_combining_mark(*c))
        .collect::<String>()
        .to_lowercase()
}

fn is_page_number(line: &str) -> bool {
    page_number_re().is_match(py_strip(line))
}

fn is_edge_garbage(line: &str) -> bool {
    edge_garbage_re().is_match(py_strip(line))
}

/// Cabeçalhos correntes: 1ª linha não-vazia que abre >= 2 páginas.
fn detect_running_headers(page_lines: &[Vec<String>]) -> std::collections::HashSet<String> {
    let mut headers = std::collections::HashSet::new();
    if page_lines.len() < 2 {
        return headers;
    }
    let mut counts: HashMap<String, usize> = HashMap::new();
    for lines in page_lines {
        if let Some(first) = lines.first() {
            *counts.entry(fold_line(first)).or_insert(0) += 1;
        }
    }
    for (fold, n) in counts {
        if n >= 2 && !fold.is_empty() {
            headers.insert(fold);
        }
    }
    headers
}

/// Número impresso no rodapé, quando isolado na última linha.
fn printed_page_number(page: &str) -> Option<String> {
    let lines: Vec<&str> = py_splitlines(page)
        .into_iter()
        .map(py_strip)
        .filter(|l| !l.is_empty())
        .collect();
    match lines.last() {
        Some(last) if is_page_number(last) => Some((*last).to_string()),
        _ => None,
    }
}

fn noise_a_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"[a-zà-ÿ][A-ZÁÀÂÃÉÊÍÓÔÕÚÇ]").unwrap())
}
fn noise_b_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?m)^\s*\d{1,4}\s+[a-zà-ÿ]").unwrap())
}
fn noise_c_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"[—-]*<[-—<>]+").unwrap())
}

/// Pontua ruído OCR; menor é melhor. (suspeita, -len em chars)
fn page_noise_score(page: &str) -> (i64, i64) {
    let mut suspicious = 0i64;
    suspicious += page.matches('|').count() as i64 * 3;
    suspicious += noise_a_re().find_iter(page).count() as i64 * 2;
    suspicious += noise_b_re().find_iter(page).count() as i64 * 4;
    suspicious += noise_c_re().find_iter(page).count() as i64 * 5;
    (suspicious, -(page.chars().count() as i64))
}

/// Remove scans duplicados (mesmo nº impresso + similaridade >= 0.55).
fn deduplicate_scanned_pages(pages: Vec<String>) -> (Vec<String>, usize) {
    // Mapa número→índices preservando ordem de inserção (dict do Python).
    let mut order: Vec<String> = Vec::new();
    let mut by_number: HashMap<String, Vec<usize>> = HashMap::new();
    for (index, page) in pages.iter().enumerate() {
        if let Some(number) = printed_page_number(page) {
            if !by_number.contains_key(&number) {
                order.push(number.clone());
            }
            by_number.entry(number).or_default().push(index);
        }
    }

    let mut remove: std::collections::HashSet<usize> = std::collections::HashSet::new();
    for number in &order {
        let indexes = &by_number[number];
        if indexes.len() < 2 {
            continue;
        }
        let candidates: Vec<usize> = indexes
            .iter()
            .copied()
            .filter(|i| !remove.contains(i))
            .collect();
        if candidates.is_empty() {
            continue;
        }
        // min() do Python: comparação de tupla, primeiro vencedor em empate.
        let mut best = candidates[0];
        let mut best_score = page_noise_score(&pages[best]);
        for &index in &candidates[1..] {
            let score = page_noise_score(&pages[index]);
            if score < best_score {
                best = index;
                best_score = score;
            }
        }
        let best_folded = fold_line(&pages[best]);
        for &index in &candidates {
            if index == best {
                continue;
            }
            let similarity = pydifflib::ratio(&best_folded, &fold_line(&pages[index]));
            if similarity >= 0.55 {
                remove.insert(index);
            }
        }
    }

    let kept: Vec<String> = pages
        .into_iter()
        .enumerate()
        .filter(|(i, _)| !remove.contains(i))
        .map(|(_, p)| p)
        .collect();
    let removed = remove.len();
    (kept, removed)
}

/// Remove número de página anexado ao fim ("TÍTULO CORRENTE 17" → "TÍTULO CORRENTE").
fn strip_trailing_page_number(line: &str) -> &str {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"^(.*\S)\s+\d{1,4}$").unwrap());
    match re.captures(py_strip(line)) {
        Some(caps) => caps.get(1).unwrap().as_str(),
        None => py_strip(line),
    }
}

/// True se a linha (ou "N Título" / modo aprimorado: "Título N") é cabeçalho.
fn is_header_line(
    line: &str,
    headers: &std::collections::HashSet<String>,
    enhanced: bool,
) -> bool {
    let stripped = py_strip(line);
    if stripped.is_empty() {
        return false;
    }
    if headers.contains(&fold_line(stripped)) {
        return true;
    }
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"^(\d{1,4})\s+(.+)$").unwrap());
    if let Some(caps) = re.captures(stripped) {
        if headers.contains(&fold_line(caps.get(2).unwrap().as_str())) {
            return true;
        }
    }
    // Aprimorado: "TÍTULO CORRENTE 17" (número anexado ao fim — PDFs nativos).
    if enhanced {
        let base = strip_trailing_page_number(stripped);
        if base != stripped && headers.contains(&fold_line(base)) {
            return true;
        }
    }
    false
}

/// Remove cabeçalhos correntes, números de página e ruído de borda.
/// Usa o \f do sidecar para processar página a página (como o Python).
/// `enhanced`: também detecta cabeçalhos na ÚLTIMA linha da página e no
/// formato "Título 17" (PDFs de texto nativo) — divergência intencional.
fn strip_page_chrome(
    text: &str,
    drop_leading_pages: usize,
    enhanced: bool,
) -> (String, BTreeMap<&'static str, i64>) {
    let mut stats: BTreeMap<&'static str, i64> = BTreeMap::new();

    // Sem separador de página: só números isolados.
    if !text.contains('\u{0c}') {
        let mut kept: Vec<&str> = Vec::new();
        let mut removed_numbers = 0i64;
        for line in py_splitlines(text) {
            if is_page_number(line) {
                removed_numbers += 1;
                continue;
            }
            kept.push(line);
        }
        stats.insert("headers_removed", 0);
        stats.insert("page_numbers_removed", removed_numbers);
        stats.insert("edge_garbage_removed", 0);
        stats.insert("pages_processed", 0);
        stats.insert("leading_pages_dropped", 0);
        stats.insert("duplicate_pages_removed", 0);
        return (kept.join("\n"), stats);
    }

    let raw_pages_all: Vec<String> = text.split('\u{0c}').map(|s| s.to_string()).collect();
    let original_page_count = raw_pages_all.len() as i64;
    let dropped = drop_leading_pages.min(raw_pages_all.len());
    let raw_pages: Vec<String> = raw_pages_all.into_iter().skip(dropped).collect();
    let (raw_pages, duplicates_removed) = deduplicate_scanned_pages(raw_pages);

    // 1ª passagem: linhas não-vazias do topo para detectar cabeçalhos.
    let first_nonempty: Vec<Vec<String>> = raw_pages
        .iter()
        .map(|page| {
            py_splitlines(page)
                .into_iter()
                .map(py_strip)
                .filter(|l| !l.is_empty())
                .map(|l| l.to_string())
                .collect()
        })
        .collect();
    let mut headers = detect_running_headers(&first_nonempty);
    if enhanced {
        // Também considera a PRIMEIRA e a ÚLTIMA linha de cada página sem o
        // número de página anexado (fim: "Título 17"; início: "32 Título") —
        // posições comuns em camadas de texto nativas.
        static LEAD_NUM: OnceLock<Regex> = OnceLock::new();
        let lead_num = LEAD_NUM.get_or_init(|| Regex::new(r"^\d{1,4}\s+(.+)$").unwrap());
        let mut counts: HashMap<String, usize> = HashMap::new();
        for lines in &first_nonempty {
            for candidate in [lines.first(), lines.last()].into_iter().flatten() {
                let no_trailing = strip_trailing_page_number(candidate);
                let base = match lead_num.captures(no_trailing) {
                    Some(caps) => caps.get(1).unwrap().as_str(),
                    None => no_trailing,
                };
                let fold = fold_line(base);
                if !fold.is_empty() {
                    *counts.entry(fold).or_insert(0) += 1;
                }
            }
        }
        for (fold, n) in counts {
            if n >= 2 {
                headers.insert(fold);
            }
        }
    }

    let mut headers_removed = 0i64;
    let mut page_numbers_removed = 0i64;
    let mut edge_garbage_removed = 0i64;
    let mut cleaned_pages: Vec<String> = Vec::new();

    for page in &raw_pages {
        // Mantém vazias para não destruir parágrafos do OCR.
        let mut lines: Vec<String> = py_splitlines(page)
            .into_iter()
            .map(|l| py_rstrip(l).to_string())
            .collect();

        // Remove linha decorativa ilegível antes de juntar parágrafos.
        lines.retain(|line| !ocr_garbage_line_re().is_match(line));

        let nonempty_idxs = |lines: &Vec<String>| -> Vec<usize> {
            lines
                .iter()
                .enumerate()
                .filter(|(_, l)| !py_strip(l).is_empty())
                .map(|(i, _)| i)
                .collect()
        };

        // Remove cabeçalhos no topo.
        loop {
            let idxs = nonempty_idxs(&lines);
            let Some(&first) = idxs.first() else { break };
            if is_header_line(&lines[first], &headers, enhanced) {
                lines.remove(first);
                headers_removed += 1;
                continue;
            }
            // Fragmento curto antes de cabeçalho conhecido = sujeira de borda.
            if idxs.len() >= 2
                && py_strip(&lines[first]).chars().count() <= 3
                && is_header_line(&lines[idxs[1]], &headers, enhanced)
            {
                lines.remove(first);
                edge_garbage_removed += 1;
                continue;
            }
            break;
        }

        // Remove rodapé: números de página e ruído curto.
        loop {
            let idxs = nonempty_idxs(&lines);
            let Some(&last) = idxs.last() else { break };
            if is_page_number(&lines[last]) {
                lines.remove(last);
                page_numbers_removed += 1;
                continue;
            }
            if is_edge_garbage(&lines[last]) {
                lines.remove(last);
                edge_garbage_removed += 1;
                continue;
            }
            // Aprimorado: cabeçalho corrente no PÉ da página sai AQUI (não só
            // no passe residual) — senão a linha em branco vizinha sobrevive
            // e vira falsa quebra de parágrafo na fronteira de páginas.
            if enhanced && is_header_line(&lines[last], &headers, true) {
                lines.remove(last);
                headers_removed += 1;
                continue;
            }
            break;
        }

        let page_text = py_strip(&lines.join("\n")).to_string();
        if !page_text.is_empty() {
            cleaned_pages.push(page_text);
        }
    }

    let joined = cleaned_pages.join("\n");

    // Passe residual: linhas que ainda são cabeçalho / número de página.
    let mut residual: Vec<&str> = Vec::new();
    for line in py_splitlines(&joined) {
        if !py_strip(line).is_empty() && is_header_line(line, &headers, enhanced) {
            headers_removed += 1;
            continue;
        }
        if is_page_number(line) {
            page_numbers_removed += 1;
            continue;
        }
        residual.push(line);
    }

    stats.insert("headers_removed", headers_removed);
    stats.insert("page_numbers_removed", page_numbers_removed);
    stats.insert("edge_garbage_removed", edge_garbage_removed);
    stats.insert("pages_processed", original_page_count);
    stats.insert("leading_pages_dropped", dropped as i64);
    stats.insert("duplicate_pages_removed", duplicates_removed as i64);
    (residual.join("\n"), stats)
}

// ---------------------------------------------------------------------------
// Passo 5 — invisíveis / controles
// ---------------------------------------------------------------------------

/// Remove caracteres invisíveis e de controle (exceto \n e \t).
/// Cc do Python ≙ `char::is_control()` do Rust.
pub fn strip_invisibles(text: &str) -> (String, usize) {
    let mut removed = 0usize;
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        if INVISIBLE_CHARS.contains(&ch) || (ch.is_control() && ch != '\n' && ch != '\t') {
            removed += 1;
            continue;
        }
        out.push(ch);
    }
    (out, removed)
}

// ---------------------------------------------------------------------------
// Passo 6 — ruído inline de OCR
// ---------------------------------------------------------------------------

/// (?<!\S)\|(?!\S): barra isolada — vizinhos ausentes ou whitespace.
/// Varredura manual linear: o equivalente fancy-regex sofre backtracking
/// catastrófico em corridas longas de whitespace (visto no Schopenhauer).
fn remove_isolated_bars(text: &str) -> (String, i64) {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut removed = 0i64;
    for (i, &c) in chars.iter().enumerate() {
        if c == '|' {
            let prev_ok = i == 0 || crate::pystr::is_py_space(chars[i - 1]);
            let next_ok = i + 1 == chars.len() || crate::pystr::is_py_space(chars[i + 1]);
            if prev_ok && next_ok {
                removed += 1;
                continue;
            }
        }
        out.push(c);
    }
    (out, removed)
}

/// (?<=[.!?»])\s+(?=\d{1,2}\.\s+[A-ZÁÀÂÃÉÊÍÓÔÕÚÇ]) → "\n\n".
/// Como \s+ é uma corrida maximal (o lookahead exige não-espaço), o match
/// é sempre a corrida inteira de whitespace após a pontuação.
fn split_embedded_sections(text: &str) -> (String, i64) {
    static AHEAD: OnceLock<Regex> = OnceLock::new();
    let ahead = AHEAD
        .get_or_init(|| Regex::new(r"^\d{1,2}\.\s+[A-ZÁÀÂÃÉÊÍÓÔÕÚÇ]").unwrap());

    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut out = String::with_capacity(text.len());
    let mut count = 0i64;
    let mut i = 0usize;
    while i < chars.len() {
        let (_, c) = chars[i];
        // Início de corrida de whitespace precedida por pontuação de fim?
        if crate::pystr::is_py_space(c) && i > 0 && matches!(chars[i - 1].1, '.' | '!' | '?' | '»')
        {
            let mut j = i;
            while j < chars.len() && crate::pystr::is_py_space(chars[j].1) {
                j += 1;
            }
            let after = if j < chars.len() { &text[chars[j].0..] } else { "" };
            if ahead.is_match(after) {
                out.push_str("\n\n");
                count += 1;
                i = j;
                continue;
            }
            // Corrida sem match: copia inteira (evita reavaliar posições).
            for k in i..j {
                out.push(chars[k].1);
            }
            i = j;
            continue;
        }
        out.push(c);
        i += 1;
    }
    (out, count)
}

/// Remove ruído estrutural inequívoco sem reconstruir palavras.
fn strip_inline_ocr_noise(text: &str) -> (String, BTreeMap<&'static str, i64>) {
    let (text, bars_removed) = remove_isolated_bars(text);
    let (text, embedded) = split_embedded_sections(&text);

    let mut lines: Vec<String> = Vec::new();
    let mut inline_page_numbers_removed = 0i64;
    let mut garbage_lines_removed = 0i64;
    let mut previous_ended_hyphen = false;
    for line in py_splitlines(&text) {
        if ocr_garbage_line_re().is_match(line) {
            garbage_lines_removed += 1;
            continue;
        }
        let mut line = line.to_string();
        if previous_ended_hyphen {
            // subn(count=1): remove só o número colado na continuação.
            if leading_page_number_re().is_match(&line).unwrap_or(false) {
                line = leading_page_number_re().replace(&line, "").into_owned();
                inline_page_numbers_removed += 1;
            }
        }
        previous_ended_hyphen = py_rstrip(&line).ends_with('-');
        lines.push(line);
    }

    let mut stats: BTreeMap<&'static str, i64> = BTreeMap::new();
    stats.insert("isolated_bars_removed", bars_removed);
    stats.insert("embedded_sections_split", embedded);
    stats.insert("inline_page_numbers_removed", inline_page_numbers_removed);
    stats.insert("garbage_lines_removed", garbage_lines_removed);
    (lines.join("\n"), stats)
}

// ---------------------------------------------------------------------------
// Passo 7 — des-hifenização
// ---------------------------------------------------------------------------

/// Junta palavras quebradas por hífen no fim da linha (conservador).
pub fn dehyphenate(text: &str) -> (String, usize) {
    let mut count = 0usize;
    let result = line_hyphen_re().replace_all(text, |caps: &regex::Captures| {
        count += 1;
        format!("{}{}", &caps[1], &caps[2])
    });
    (result.into_owned(), count)
}

// ---------------------------------------------------------------------------
// Passos 8 e 9 — reflow e whitespace
// ---------------------------------------------------------------------------

/// Linha com cara de entrada de sumário: "Título ..... 42", "Título, 312"
/// ou "Título, 312; Sub, 313". Protegida do reflow no modo aprimorado.
fn toc_line_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?:(?:[.…]\s*){2,}|,)\s*\d{1,4}\s*;?\s*$").unwrap()
    })
}

/// Isola linhas de sumário em parágrafos próprios antes do reflow, para a
/// estruturação enxergá-las como entradas (o reflow as fundiria em blocos).
fn protect_toc_lines(text: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    for line in py_splitlines(text) {
        let stripped = py_strip(line);
        // Trava de comprimento: entrada de sumário é linha curta.
        if stripped.chars().count() <= 90 && toc_line_re().is_match(stripped) {
            out.push(format!("\n{line}\n"));
        } else {
            out.push(line.to_string());
        }
    }
    out.join("\n")
}

/// Junta linhas soltas de um mesmo parágrafo em uma linha contínua.
fn reflow_paragraphs(text: &str) -> String {
    let mut reflowed: Vec<String> = Vec::new();
    for para in text.split("\n\n") {
        let lines: Vec<&str> = para
            .split('\n')
            .map(py_strip)
            .filter(|l| !l.is_empty())
            .collect();
        if lines.is_empty() {
            continue;
        }
        reflowed.push(lines.join(" "));
    }
    format!("{}\n", reflowed.join("\n\n"))
}

/// Normaliza espaços e linhas em branco preservando parágrafos.
pub fn normalize_whitespace(text: &str) -> String {
    static SPACES: OnceLock<Regex> = OnceLock::new();
    static SPACE_NL: OnceLock<Regex> = OnceLock::new();
    static NL_SPACE: OnceLock<Regex> = OnceLock::new();
    static MANY_NL: OnceLock<Regex> = OnceLock::new();

    let spaces = SPACES.get_or_init(|| Regex::new(r"[ \t]+").unwrap());
    let space_nl = SPACE_NL.get_or_init(|| Regex::new(r"[ \t]+\n").unwrap());
    let nl_space = NL_SPACE.get_or_init(|| Regex::new(r"\n[ \t]+").unwrap());
    let many_nl = MANY_NL.get_or_init(|| Regex::new(r"\n{3,}").unwrap());

    let t = spaces.replace_all(text, " ");
    let t = space_nl.replace_all(&t, "\n");
    let t = nl_space.replace_all(&t, "\n");
    let t = many_nl.replace_all(&t, "\n\n");
    format!("{}\n", py_strip(&t))
}

// ---------------------------------------------------------------------------
// Orquestrador — clean_text
// ---------------------------------------------------------------------------

/// Cronômetro por etapa, ativado com MELHORADOR_PROFILE=1 (diagnóstico).
fn profile_step(label: &str, start: std::time::Instant) -> std::time::Instant {
    if std::env::var_os("MELHORADOR_PROFILE").is_some() {
        eprintln!("[profile] {label}: {:?}", start.elapsed());
    }
    std::time::Instant::now()
}

/// Pipeline determinístico completo (ordem idêntica ao Python — modo paridade).
pub fn clean_text(text: &str, reflow: bool, drop_leading_pages: usize) -> CleanupResult {
    clean_text_impl(text, reflow, drop_leading_pages, false)
}

/// Modo APRIMORADO do app: heurísticas extras p/ PDFs de texto nativo
/// (cabeçalho na última linha, "Título 17"). Diverge do CLI de propósito.
pub fn clean_text_enhanced(text: &str, reflow: bool, drop_leading_pages: usize) -> CleanupResult {
    clean_text_impl(text, reflow, drop_leading_pages, true)
}

fn clean_text_impl(
    text: &str,
    reflow: bool,
    drop_leading_pages: usize,
    enhanced: bool,
) -> CleanupResult {
    let mut warnings: Vec<String> = Vec::new();
    let original_len = text.chars().count() as i64;
    let t = std::time::Instant::now();

    // 1. ftfy (equivalente observado no corpus: uncurl de aspas).
    let fixed = uncurl_quotes(text);
    let t = profile_step("1 uncurl", t);
    // 2. NFC.
    let fixed = nfc(&fixed);
    let t = profile_step("2 nfc", t);
    // 3. Marcadores "-- N of M --".
    let (fixed, n_markers) = remove_page_markers(&fixed);
    let t = profile_step("3 markers", t);
    // 4. Cabeçalhos/rodapés/dedup (usa \f; antes de apagar controles).
    let (fixed, chrome_stats) = strip_page_chrome(&fixed, drop_leading_pages, enhanced);
    let t = profile_step("4 chrome", t);
    // 4b. Aprimorado: \x02 é marcador de hifenização das camadas nativas;
    // no fim de linha significa "palavra continua na próxima" — juntar
    // direto (sem espaço), inclusive na fronteira de páginas.
    let fixed = if enhanced {
        fixed.replace("\u{2}\n", "")
    } else {
        fixed
    };
    // 5. Invisíveis/controles (inclui \f remanescente).
    let (fixed, n_invisible) = strip_invisibles(&fixed);
    let t = profile_step("5 invisibles", t);
    // 6. Barras isoladas, números colados, seções embutidas.
    let (fixed, inline_stats) = strip_inline_ocr_noise(&fixed);
    let t = profile_step("6 inline", t);
    // 7. Hifenização de quebra de linha.
    let (fixed, n_hyphens) = dehyphenate(&fixed);
    let t = profile_step("7 dehyphen", t);
    // 8. Reflow (opcional). Aprimorado: entradas de sumário ficam intactas.
    let fixed = if enhanced { protect_toc_lines(&fixed) } else { fixed };
    let fixed = if reflow { reflow_paragraphs(&fixed) } else { fixed };
    let t = profile_step("8 reflow", t);
    // 9. Whitespace.
    let fixed = normalize_whitespace(&fixed);
    let _ = profile_step("9 whitespace", t);

    let replacement_count = fixed.matches('\u{fffd}').count() as i64;
    if replacement_count > 0 {
        warnings.push(format!(
            "{replacement_count} caractere(s) de substituição (\u{fffd}) — revisar OCR"
        ));
    }

    let mut stats: BTreeMap<&'static str, i64> = BTreeMap::new();
    stats.insert("chars_in", original_len);
    stats.insert("chars_out", fixed.chars().count() as i64);
    stats.insert("page_markers_removed", n_markers as i64);
    stats.extend(chrome_stats);
    stats.extend(inline_stats);
    stats.insert("invisible_removed", n_invisible as i64);
    stats.insert("hyphenations_joined", n_hyphens as i64);
    stats.insert("replacement_chars", replacement_count);

    CleanupResult { text: fixed, stats, warnings }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Casos espelhados de _CLI/tests/test_cleanup.py — paridade com Python.

    #[test]
    fn dehyphenate_junta_minusculas() {
        let (out, n) = dehyphenate("pala-\nvra inteira");
        assert_eq!(out, "palavra inteira");
        assert_eq!(n, 1);
    }

    #[test]
    fn dehyphenate_preserva_maiuscula_apos_hifen() {
        let (out, n) = dehyphenate("Guarda-\nMor");
        assert_eq!(out, "Guarda-\nMor");
        assert_eq!(n, 0);
    }

    #[test]
    fn dehyphenate_acentuado() {
        let (out, n) = dehyphenate("civiliza-\nção");
        assert_eq!(out, "civilização");
        assert_eq!(n, 1);
    }

    #[test]
    fn normalize_colapsa_espacos_e_tabs() {
        assert_eq!(normalize_whitespace("a  b\t\tc"), "a b c\n");
    }

    #[test]
    fn normalize_remove_espacos_nas_bordas_de_linha() {
        assert_eq!(normalize_whitespace("a  \n  b"), "a\nb\n");
    }

    #[test]
    fn normalize_limita_quebras_a_paragrafo() {
        assert_eq!(normalize_whitespace("a\n\n\n\n\nb"), "a\n\nb\n");
    }

    #[test]
    fn normalize_garante_newline_final_unico() {
        assert_eq!(normalize_whitespace("texto"), "texto\n");
        assert_eq!(normalize_whitespace("  texto  \n\n"), "texto\n");
    }

    #[test]
    fn page_marker_removido() {
        let (out, n) = remove_page_markers("corpo\n-- 3 of 578 --\nmais corpo");
        assert_eq!(out, "corpo\n\nmais corpo");
        assert_eq!(n, 1);
    }

    #[test]
    fn page_marker_nao_confunde_travessao() {
        let (out, n) = remove_page_markers("ele disse -- e foi --\nfim");
        assert_eq!(out, "ele disse -- e foi --\nfim");
        assert_eq!(n, 0);
    }

    #[test]
    fn invisiveis_removidos_preservando_tab_e_newline() {
        let (out, n) = strip_invisibles("a\u{200b}b\u{00ad}c\td\ne\u{0007}f");
        assert_eq!(out, "abc\td\nef");
        assert_eq!(n, 3);
    }

    #[test]
    fn invisiveis_texto_limpo_intacto() {
        let (out, n) = strip_invisibles("texto normal, sem sujeira");
        assert_eq!(out, "texto normal, sem sujeira");
        assert_eq!(n, 0);
    }

    #[test]
    fn aspas_curvas_destorcidas() {
        assert_eq!(uncurl_quotes("\u{201c}ok\u{201d} \u{2018}a\u{2019}"), "\"ok\" 'a'");
    }

    #[test]
    fn chrome_remove_cabecalho_corrente_e_numero() {
        // Duas páginas com o mesmo cabeçalho e números de rodapé.
        let text = "TÍTULO DO CAPÍTULO\ncorpo um\n12\u{0c}TÍTULO DO CAPÍTULO\ncorpo dois\n13";
        let (out, stats) = strip_page_chrome(text, 0, false);
        assert_eq!(out, "corpo um\ncorpo dois");
        assert_eq!(stats["headers_removed"], 2);
        assert_eq!(stats["page_numbers_removed"], 2);
    }

    #[test]
    fn chrome_aprimorado_remove_cabecalho_no_fim_com_numero() {
        // Padrão dos PDFs nativos: "TÍTULO CORRENTE 17" na ÚLTIMA linha.
        let text = "corpo um\nLUGAR DOS GREGOS 17\u{0c}corpo dois\nLUGAR DOS GREGOS 18";
        let (out, stats) = strip_page_chrome(text, 0, true);
        assert_eq!(out, "corpo um\ncorpo dois");
        assert_eq!(stats["headers_removed"], 2);
        // Modo paridade NÃO remove (fiel ao CLI).
        let (out_par, _) = strip_page_chrome(text, 0, false);
        assert!(out_par.contains("LUGAR DOS GREGOS 17"));
    }

    #[test]
    fn clean_text_pipeline_minimo() {
        let result = clean_text("Uma pala-\nvra  quebrada.\n\n\n\nOutro parágrafo.", true, 0);
        assert_eq!(result.text, "Uma palavra quebrada.\n\nOutro parágrafo.\n");
        assert!(result.warnings.is_empty());
    }
}
