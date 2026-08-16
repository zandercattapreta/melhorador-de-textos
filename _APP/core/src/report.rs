// ==============================================================================
// SCRIPT: report.rs (melhorador-core)
// DESCRIÇÃO: Contrato report.json (paridade CLI) + trilha de diffs aprovados
// CHAMADO POR: save_result no app
// CONTRATO (RESPOSTA ESPERADA): JSON serializável com hashes e stats
// ==============================================================================

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

use crate::review::DiffProposal;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessReport {
    pub generated_at: String,
    pub tool_version: String,
    pub input_name: String,
    pub engine: String,
    pub languages: String,
    pub page_count: u32,
    pub cleanup_stats: BTreeMap<String, i64>,
    pub structure_stats: BTreeMap<String, i64>,
    pub cleanup_warnings: Vec<String>,
    pub raw_sha256: Option<String>,
    pub cleaned_sha256: String,
    pub source_sha256: String,
    pub accepted_review_diffs: Vec<DiffProposal>,
    pub metadata: Option<crate::metadata::BookMeta>,
}

pub fn sha256_text(text: &str) -> String {
    let mut h = Sha256::new();
    h.update(text.as_bytes());
    format!("{:x}", h.finalize())
}

pub fn build_report(
    input_name: &str,
    engine: &str,
    languages: &str,
    page_count: u32,
    cleaned: &str,
    source_for_hash: &str,
    cleanup_stats: &BTreeMap<&'static str, i64>,
    structure_stats: &BTreeMap<&'static str, i64>,
    warnings: &[String],
    accepted_diffs: &[DiffProposal],
    metadata: Option<crate::metadata::BookMeta>,
    raw_sha: Option<&str>,
) -> ProcessReport {
    let now = chrono::Utc::now().to_rfc3339();
    ProcessReport {
        generated_at: now,
        tool_version: env!("CARGO_PKG_VERSION").into(),
        input_name: input_name.into(),
        engine: engine.into(),
        languages: languages.into(),
        page_count,
        cleanup_stats: cleanup_stats
            .iter()
            .map(|(k, v)| ((*k).to_string(), *v))
            .collect(),
        structure_stats: structure_stats
            .iter()
            .map(|(k, v)| ((*k).to_string(), *v))
            .collect(),
        cleanup_warnings: warnings.to_vec(),
        raw_sha256: raw_sha.map(|s| s.to_string()),
        cleaned_sha256: sha256_text(cleaned),
        source_sha256: sha256_text(source_for_hash),
        accepted_review_diffs: accepted_diffs.to_vec(),
        metadata,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_estavel() {
        assert_eq!(sha256_text("abc"), sha256_text("abc"));
        assert_ne!(sha256_text("abc"), sha256_text("abd"));
    }
}
