// ==============================================================================
// SCRIPT: pystr.rs (txtmelhorator-core)
// DESCRIÇÃO: Semântica de strings do Python (strip/splitlines/isspace) em Rust
// CHAMADO POR: cleanup.rs, structure.rs
// CONTRATO (RESPOSTA ESPERADA): comportamento idêntico ao CPython 3.12
// ==============================================================================

//! O port é validado byte a byte contra o CLI Python; qualquer diferença de
//! semântica de `strip()`/`splitlines()` quebraria a paridade. Estes helpers
//! reproduzem o CPython exatamente onde o Rust padrão diverge.

/// `str.isspace()` do Python: inclui \x1c–\x1f, que `char::is_whitespace`
/// do Rust NÃO considera espaço.
pub fn is_py_space(c: char) -> bool {
    c.is_whitespace() || matches!(c, '\u{1c}'..='\u{1f}')
}

/// `str.strip()` do Python.
pub fn py_strip(s: &str) -> &str {
    s.trim_matches(is_py_space)
}

/// `str.rstrip()` do Python.
pub fn py_rstrip(s: &str) -> &str {
    s.trim_end_matches(is_py_space)
}

/// `str.splitlines()` do Python: quebra em \n, \r, \r\n, \v, \f, \x1c–\x1e,
/// \x85,  ,   — sem incluir o terminador e sem linha fantasma final.
pub fn py_splitlines(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let bytes_len = s.len();
    let mut start = 0usize;
    let mut iter = s.char_indices().peekable();
    while let Some((i, c)) = iter.next() {
        let is_break = matches!(
            c,
            '\n' | '\r' | '\u{0b}' | '\u{0c}' | '\u{1c}' | '\u{1d}' | '\u{1e}'
                | '\u{85}' | '\u{2028}' | '\u{2029}'
        );
        if is_break {
            out.push(&s[start..i]);
            let mut next_start = i + c.len_utf8();
            // \r\n conta como UMA quebra.
            if c == '\r' {
                if let Some(&(j, '\n')) = iter.peek() {
                    iter.next();
                    next_start = j + 1;
                }
            }
            start = next_start;
        }
    }
    if start < bytes_len {
        out.push(&s[start..]);
    }
    out
}

/// `str.split()` sem argumentos do Python: separa por runs de whitespace,
/// ignorando bordas (nunca produz strings vazias).
pub fn py_split_ws(s: &str) -> Vec<&str> {
    s.split(is_py_space).filter(|w| !w.is_empty()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_inclui_controles_x1c() {
        assert_eq!(py_strip("\u{1c}abc\u{1f}"), "abc");
        assert_eq!(py_strip("  abc  "), "abc");
    }

    #[test]
    fn splitlines_sem_linha_fantasma() {
        assert_eq!(py_splitlines("a\nb\n"), vec!["a", "b"]);
        assert_eq!(py_splitlines("a\r\nb\rc"), vec!["a", "b", "c"]);
        assert_eq!(py_splitlines(""), Vec::<&str>::new());
    }

    #[test]
    fn split_ws_ignora_bordas() {
        assert_eq!(py_split_ws("  a  b\tc "), vec!["a", "b", "c"]);
    }
}
