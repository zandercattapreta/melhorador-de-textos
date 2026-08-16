// ==============================================================================
// SCRIPT: review.rs (melhorador-core)
// DESCRIÇÃO: Revisão opt-in (R5) — propõe diffs; nunca aplica sozinha
// CHAMADO POR: comando Tauri propose_review; UI aceitar/rejeitar
// CONTRATO (RESPOSTA ESPERADA): lista de propostas ancoradas no texto fonte
// ==============================================================================

//! IA local (GGUF) entra depois, se existir no aparelho. Sem modelo: só
//! heurísticas determinísticas (espaços duplos, etc.). Vocabulário = termos
//! da própria fonte. Proposta sem âncora no original é rejeitada.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

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

/// Revisão heurística local (sem LLM). Só propõe; não aplica.
pub fn propose_heuristic_review(text: &str) -> ReviewReport {
    let vocabulary = extract_vocabulary(text, 80);
    let vocab_set: BTreeSet<_> = vocabulary.iter().cloned().collect();
    let mut proposals = Vec::new();

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

    // Aviso se termo do vocabulário aparece partido com espaço no meio (muito conservador: skip)

    let _ = vocab_set;
    ReviewReport {
        proposals,
        vocabulary,
        engine: "heuristic".into(),
        note: "Revisão local sem LLM. Ligue um modelo GGUF depois (R5 completo) para mais propostas. Nada é aplicado sem você aceitar.".into(),
    }
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
4) Vocabulário do livro (use como âncora): {vocab}
5) Responda SOMENTE JSON array: [{{"original":"...","proposed":"...","reason":"..."}}]
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

/// Chama `llama-cli` / `llama-server` se existir no PATH + modelo GGUF.
pub fn propose_llama_review(text: &str, model_path: &std::path::Path) -> ReviewReport {
    let vocabulary = extract_vocabulary(text, 80);
    let mut base = propose_heuristic_review(text);
    if !model_path.is_file() {
        base.engine = "heuristic".into();
        base.note = "Modelo GGUF ausente — só heurística.".into();
        return base;
    }
    let bin = ["llama-cli", "llama-completion", "main"]
        .into_iter()
        .find(|b| std::process::Command::new(b).arg("--version").output().is_ok());
    let Some(bin) = bin else {
        base.note = "llama.cpp não encontrado no PATH. Instale e selecione um GGUF.".into();
        base.engine = "heuristic+unavailable_llm".into();
        return base;
    };
    let prompt = fidelity_prompt(text, &vocabulary);
    let out = std::process::Command::new(bin)
        .args([
            "-m",
            &model_path.to_string_lossy(),
            "-p",
            &prompt,
            "-n",
            "512",
            "--temp",
            "0.1",
        ])
        .output();
    match out {
        Ok(o) if o.status.success() => {
            let stdout = String::from_utf8_lossy(&o.stdout);
            let mut llm = parse_llm_proposals(text, &stdout);
            base.proposals.append(&mut llm);
            base.engine = format!("heuristic+{bin}");
            base.note = "Propostas heurísticas + LLM local. Nada aplicado sem aceite.".into();
            base.vocabulary = vocabulary;
            base
        }
        Ok(o) => {
            base.engine = "heuristic+llm_error".into();
            base.note = format!(
                "LLM falhou: {}",
                String::from_utf8_lossy(&o.stderr).chars().take(200).collect::<String>()
            );
            base
        }
        Err(e) => {
            base.engine = "heuristic+llm_error".into();
            base.note = format!("Não executei {bin}: {e}");
            base
        }
    }
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
