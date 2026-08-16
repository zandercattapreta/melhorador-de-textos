// ==============================================================================
// SCRIPT: rules.rs (melhorador-core)
// DESCRIÇÃO: Regras que o usuário ensina (R4) — aplicadas antes de estrutura/IA
// CHAMADO POR: lib.rs enhanced; comandos Tauri de persistência
// CONTRATO (RESPOSTA ESPERADA): texto só com remoções/marcações; zero invenção
// ==============================================================================

//! Preferências locais: marcar linhas/padrões como cabeçalho, nota, ou
//! “não juntar”. Não treina OCR — só filtra/anota o texto já extraído.

use serde::{Deserialize, Serialize};

/// Tipo de regra ensinada pelo usuário.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuleKind {
    /// Linha que contém o padrão → remover (cabeçalho/rodapé recorrente).
    Header,
    /// Linha que começa/contém o padrão → tratar como nota (prefixo).
    Note,
    /// Não juntar hifenização/carry se o fragmento casar o padrão.
    NoJoin,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserRule {
    pub kind: RuleKind,
    /// Substring literal (case-insensitive) a casar.
    pub pattern: String,
}

impl UserRule {
    pub fn new(kind: RuleKind, pattern: impl Into<String>) -> Self {
        Self {
            kind,
            pattern: pattern.into(),
        }
    }
}

fn fold(s: &str) -> String {
    s.to_lowercase()
}

/// Aplica regras: remove headers; prefixa notas; NoJoin é consultado à parte.
pub fn apply_user_rules(text: &str, rules: &[UserRule]) -> String {
    if rules.is_empty() {
        return text.to_string();
    }
    let headers: Vec<String> = rules
        .iter()
        .filter(|r| r.kind == RuleKind::Header)
        .map(|r| fold(&r.pattern))
        .filter(|p| !p.is_empty())
        .collect();
    let notes: Vec<String> = rules
        .iter()
        .filter(|r| r.kind == RuleKind::Note)
        .map(|r| fold(&r.pattern))
        .filter(|p| !p.is_empty())
        .collect();

    let mut out = Vec::new();
    for line in text.lines() {
        let fl = fold(line);
        if headers.iter().any(|h| fl.contains(h)) {
            continue;
        }
        if notes.iter().any(|n| fl.contains(n)) {
            let trimmed = line.trim();
            if trimmed.starts_with('[') && trimmed.contains("nota") {
                out.push(line.to_string());
            } else {
                out.push(format!("[nota] {trimmed}"));
            }
            continue;
        }
        out.push(line.to_string());
    }
    out.join("\n")
}

/// Padrões NoJoin: se o fragmento de carry casar, não transportar.
pub fn is_no_join_fragment(frag: &str, rules: &[UserRule]) -> bool {
    let f = fold(frag);
    rules.iter().any(|r| {
        r.kind == RuleKind::NoJoin && !r.pattern.is_empty() && f.contains(&fold(&r.pattern))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remove_linha_de_cabecalho() {
        let rules = vec![UserRule::new(RuleKind::Header, "LUGAR DOS GREGOS")];
        let text = "Prosa boa.\nLUGAR DOS GREGOS 17\nMais prosa.";
        let out = apply_user_rules(text, &rules);
        assert!(!out.contains("LUGAR DOS GREGOS"));
        assert!(out.contains("Prosa boa."));
        assert!(out.contains("Mais prosa."));
    }

    #[test]
    fn marca_nota() {
        let rules = vec![UserRule::new(RuleKind::Note, "Cf.")];
        let out = apply_user_rules("Cf. Aristóteles, Pol.\nProsa.", &rules);
        assert!(out.contains("[nota] Cf."));
        assert!(out.contains("Prosa."));
    }

    #[test]
    fn no_join_detecta() {
        let rules = vec![UserRule::new(RuleKind::NoJoin, "ne")];
        assert!(is_no_join_fragment("ne", &rules));
        assert!(!is_no_join_fragment("ção", &rules));
    }
}
