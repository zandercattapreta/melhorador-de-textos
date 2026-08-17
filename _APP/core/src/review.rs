// ==============================================================================
// SCRIPT: review.rs (txtmelhorator-core)
// DESCRIÇÃO: Revisão opt-in (R5) — heurística + IA; UI aplica + Desfazer
// CHAMADO POR: comando Tauri propose_review; UI
// CONTRATO: propostas ancoradas; inclui des-hifenização de fim de linha
// ==============================================================================

//! IA local (GGUF) entra depois, se existir no aparelho. Sem modelo: só
//! heurísticas determinísticas (espaços, hifenação de linha, etc.).
//! Vocabulário = termos da própria fonte. Proposta sem âncora é rejeitada.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::sync::OnceLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffProposal {
    /// Trecho exato no texto atual (âncora).
    pub original: String,
    /// Substituição proposta (deve preservar sentido; sem invenção).
    pub proposed: String,
    pub reason: String,
    /// Índice de byte no texto (aproximado; UI busca `original`).
    pub byte_offset: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewReport {
    pub proposals: Vec<DiffProposal>,
    /// Termos extraídos do próprio texto (vocabulário do livro).
    pub vocabulary: Vec<String>,
    /// "heuristic" | "unavailable_llm"
    pub engine: String,
    pub note: String,
}

/// Extrai vocabulário: tokens capitalizados repetidos / palavras longas únicas.
pub fn extract_vocabulary(text: &str, limit: usize) -> Vec<String> {
    let mut counts: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for raw in text.split_whitespace() {
        let w: String = raw
            .chars()
            .filter(|c| c.is_alphabetic() || *c == '-' || *c == '’' || *c == '\'')
            .collect();
        if w.chars().count() < 5 {
            continue;
        }
        let key = w.to_lowercase();
        *counts.entry(key).or_insert(0) += 1;
    }
    let mut items: Vec<(String, usize)> = counts.into_iter().collect();
    items.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    items
        .into_iter()
        .filter(|(_, n)| *n >= 2)
        .take(limit)
        .map(|(w, _)| w)
        .collect()
}

fn reject_if_unanchored(text: &str, original: &str, proposed: &str) -> bool {
    if original.is_empty() || !text.contains(original) {
        return true;
    }
    // Proposta não pode ser muito mais longa (anti-invenção grosseira).
    if proposed.chars().count() > original.chars().count() + 8 {
        return true;
    }
    false
}

/// Revisão heurística local (sem LLM). A UI aplica + Desfazer.
pub fn propose_heuristic_review(text: &str) -> ReviewReport {
    let vocabulary = extract_vocabulary(text, 80);
    let vocab_set: BTreeSet<_> = vocabulary.iter().cloned().collect();
    let mut proposals = Vec::new();

    proposals.extend(dehyphen_proposals(text));

    // Espaços duplos → um espaço (âncora literal).
    if let Some(idx) = text.find("  ") {
        let original = "  ".to_string();
        let proposed = " ".to_string();
        if !reject_if_unanchored(text, &original, &proposed) {
            proposals.push(DiffProposal {
                original,
                proposed,
                reason: "espaço duplo".into(),
                byte_offset: idx,
            });
        }
    }

    // "palavra ," → "palavra,"
    for (i, _) in text.match_indices(" ,") {
        let original = " ,".to_string();
        let proposed = ",".to_string();
        if !reject_if_unanchored(text, &original, &proposed) {
            proposals.push(DiffProposal {
                original,
                proposed,
                reason: "espaço antes da vírgula".into(),
                byte_offset: i,
            });
            break; // uma amostra; UI pode re-rodar
        }
    }

    let _ = vocab_set;
    let n_hyph = proposals
        .iter()
        .filter(|p| p.reason.contains("hifenação"))
        .count();
    ReviewReport {
        proposals,
        vocabulary,
        engine: "basico".into(),
        note: if n_hyph > 0 {
            format!("Inclui {n_hyph} juntura(s) de hifenação. LanguageTool/IA pegam o resto.")
        } else {
            "Correções simples (hífens de linha, espaços). LanguageTool/IA ampliam.".into()
        },
    }
}

fn review_hyphen_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"([A-Za-zÀ-ÿ]+)-\r?\n([a-zà-ÿ])").unwrap())
}

/// Uma proposta por ocorrência: "civiliza-\nção" → "civilização".
pub fn dehyphen_proposals(text: &str) -> Vec<DiffProposal> {
    let mut out = Vec::new();
    for caps in review_hyphen_re().captures_iter(text) {
        let m = caps.get(0).expect("match");
        let original = m.as_str().to_string();
        let proposed = format!("{}{}", &caps[1], &caps[2]);
        if reject_if_unanchored(text, &original, &proposed) {
            continue;
        }
        out.push(DiffProposal {
            original,
            proposed,
            reason: "hifenação de fim de linha".into(),
            byte_offset: m.start(),
        });
    }
    // Soft hyphen (OCR) — remove o caractere invisível.
    if let Some(idx) = text.find('\u{00ad}') {
        out.push(DiffProposal {
            original: "\u{00ad}".into(),
            proposed: "".into(),
            reason: "hífen suave (OCR)".into(),
            byte_offset: idx,
        });
    }
    out
}

/// Prompt de fidelidade: só diffs ancorados; sem reescrita de estilo.
pub fn fidelity_prompt(text: &str, vocabulary: &[String]) -> String {
    let sample: String = text.chars().take(6000).collect();
    let vocab = vocabulary.iter().take(40).cloned().collect::<Vec<_>>().join(", ");
    format!(
        r#"Você revisa texto OCR de livro. Regras absolutas:
1) Só proponha correções de OCR/digitação ancoradas no trecho original.
2) Nunca invente frases, nomes ou fatos.
3) Nunca reescreva o estilo do autor.
4) SEMPRE junte hifenação de fim de linha: "pala-\nvra" → "palavra"; "civiliza-\nção" → "civilização". Não remova hífens reais de compostos (ex.: guarda-chuva) quando as duas partes estão na mesma linha.
5) Vocabulário do livro (use como âncora): {vocab}
6) Responda SOMENTE JSON array: [{{"original":"...","proposed":"...","reason":"..."}}]
Texto:
---
{sample}
---"#
    )
}

/// Interpreta JSON de propostas do modelo; rejeita sem âncora.
pub fn parse_llm_proposals(text: &str, raw_json: &str) -> Vec<DiffProposal> {
    let trimmed = raw_json.trim();
    let start = trimmed.find('[').unwrap_or(0);
    let end = trimmed.rfind(']').map(|i| i + 1).unwrap_or(trimmed.len());
    let slice = &trimmed[start..end];
    let Ok(items) = serde_json::from_str::<Vec<serde_json::Value>>(slice) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for item in items {
        let original = item
            .get("original")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let proposed = item
            .get("proposed")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let reason = item
            .get("reason")
            .and_then(|v| v.as_str())
            .unwrap_or("llm")
            .to_string();
        if reject_if_unanchored(text, &original, &proposed) {
            continue;
        }
        let byte_offset = text.find(&original).unwrap_or(0);
        out.push(DiffProposal {
            original,
            proposed,
            reason,
            byte_offset,
        });
    }
    out
}

/// Combina heurística + saída do modelo (já gerada in-process pelo app).
pub fn merge_llm_review(text: &str, model_stdout: &str) -> ReviewReport {
    let vocabulary = extract_vocabulary(text, 80);
    let mut base = propose_heuristic_review(text);
    let mut llm = parse_llm_proposals(text, model_stdout);
    if llm.is_empty() && model_stdout.trim().is_empty() {
        base.engine = "ia-local-erro".into();
        base.note =
            "A IA local não devolveu sugestões. Tente LanguageTool ou outro trecho.".into();
        return base;
    }
    base.proposals.append(&mut llm);
    base.engine = "ia-local".into();
    base.note =
        "IA local: inclui juntar hifenação de linha. Desfazer restaura o anterior.".into();
    base.vocabulary = vocabulary;
    base
}

/// @deprecated Substituído por inferência in-process no app (R5c). Mantido p/ testes.
pub fn propose_llama_review(text: &str, model_path: &std::path::Path) -> ReviewReport {
    let mut base = propose_heuristic_review(text);
    if !model_path.is_file() {
        base.note = "Nenhum modelo de IA local selecionado. Use LanguageTool, ou escolha um modelo em Ajustes.".into();
        base.engine = "basico".into();
        return base;
    }
    base.engine = "ia-local-indisponivel".into();
    base.note =
        "A revisão com IA roda dentro do app (não por programa externo). Abra pelo TXTMelhorator.".into();
    base
}

/// Benchmark mínimo de fidelidade: propostas não podem alongar demais.
pub fn fidelity_benchmark_ok(text: &str, proposals: &[DiffProposal]) -> bool {
    proposals.iter().all(|p| {
        !reject_if_unanchored(text, &p.original, &p.proposed)
            && p.proposed.chars().count() <= p.original.chars().count() + 8
    })
}

/// Aplica só as propostas aceitas (ordem inversa por offset para não deslocar).
pub fn apply_accepted_diffs(text: &str, accepted: &[DiffProposal]) -> Result<String, String> {
    let mut items: Vec<&DiffProposal> = accepted.iter().collect();
    items.sort_by(|a, b| b.byte_offset.cmp(&a.byte_offset));
    let mut out = text.to_string();
    for p in items {
        if reject_if_unanchored(&out, &p.original, &p.proposed) {
            return Err(format!(
                "Proposta sem âncora rejeitada: {:?}",
                p.original
            ));
        }
        // Substitui a primeira ocorrência a partir do offset se possível.
        if let Some(rel) = out[p.byte_offset.min(out.len())..].find(&p.original) {
            let at = p.byte_offset.min(out.len()) + rel;
            out.replace_range(at..at + p.original.len(), &p.proposed);
        } else if let Some(at) = out.find(&p.original) {
            out.replace_range(at..at + p.original.len(), &p.proposed);
        } else {
            return Err(format!("Âncora sumiu: {:?}", p.original));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vocabulario_do_proprio_texto() {
        let t = "Atenas Atenas pólis Demóstenes Demóstenes história história história";
        let v = extract_vocabulary(t, 10);
        assert!(v.iter().any(|w| w == "atenas" || w == "demóstenes" || w.contains("histor")));
    }

    #[test]
    fn dehyphen_propostas() {
        let t = "civiliza-\nção e pala-\nvra";
        let ps = dehyphen_proposals(t);
        assert!(ps.len() >= 2);
        let applied = apply_accepted_diffs(t, &ps).unwrap();
        assert!(applied.contains("civilização"));
        assert!(applied.contains("palavra"));
        assert!(!applied.contains("-\n"));
    }

    #[test]
    fn proposta_espaco_duplo() {
        let t = "Uma frase  com erro.";
        let r = propose_heuristic_review(t);
        assert!(!r.proposals.is_empty());
        let applied = apply_accepted_diffs(t, &r.proposals[..1]).unwrap();
        assert!(applied.contains("frase com"));
        assert!(!applied.contains("frase  com"));
    }

    #[test]
    fn rejeita_sem_ancora() {
        let t = "texto limpo";
        let bad = DiffProposal {
            original: "xyz".into(),
            proposed: "abc".into(),
            reason: "teste".into(),
            byte_offset: 0,
        };
        assert!(apply_accepted_diffs(t, &[bad]).is_err());
    }

    #[test]
    fn benchmark_fidelidade_basico() {
        let t = "Uma frase  com erro.";
        let r = propose_heuristic_review(t);
        assert!(fidelity_benchmark_ok(t, &r.proposals));
    }

    #[test]
    fn parse_llm_rejeita_invencao() {
        let t = "O autor escreveu isto.";
        let json = r#"[{"original":"isto","proposed":"isto e muito mais inventado demais","reason":"x"}]"#;
        assert!(parse_llm_proposals(t, json).is_empty());
    }
}
