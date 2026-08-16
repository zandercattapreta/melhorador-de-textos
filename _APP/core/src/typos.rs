// ==============================================================================
// SCRIPT: typos.rs (melhorador-core)
// DESCRIÇÃO: Dicionário determinístico de typos OCR comuns (pt-BR)
// CHAMADO POR: cleanup enhanced (opt-in); CLI futuro
// CONTRATO (RESPOSTA ESPERADA): só substitui padrões fixos; zero invenção
// ==============================================================================

/// Pares (errado, certo) — só erros tipográficos OCR óbvios, sem reescrita.
const OCR_TYPOS: &[(&str, &str)] = &[
    (" rn ", " m "),
    (" cl ", " d "),
    (" vv ", " w "),
    ("ﬁ", "fi"),
    ("ﬂ", "fl"),
    ("—-", "—"),
];

/// Aplica o dicionário (ordem fixa). Conta substituições.
pub fn apply_ocr_typos(text: &str) -> (String, usize) {
    let mut out = text.to_string();
    let mut n = 0usize;
    for (bad, good) in OCR_TYPOS {
        let before = out.matches(bad).count();
        if before > 0 {
            out = out.replace(bad, good);
            n += before;
        }
    }
    (out, n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn liga_fi() {
        let (t, n) = apply_ocr_typos("aﬁrmar");
        assert_eq!(t, "afirmar");
        assert_eq!(n, 1);
    }
}
