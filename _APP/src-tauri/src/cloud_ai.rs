// ==============================================================================
// SCRIPT: cloud_ai.rs (melhorador-app)
// DESCRIÇÃO: Revisão via API OpenAI-compatível (nuvem) — só propõe diffs
// CHAMADO POR: comando Tauri check_cloud_ai
// CONTRATO: texto SAI da máquina; usuário confirma; keychain guarda a chave
// ==============================================================================

use melhorador_core::review::{self, DiffProposal, ReviewReport};
use serde::{Deserialize, Serialize};
use std::process::Command;

const KEYCHAIN_SERVICE: &str = "com.zedicoes.melhorador.cloud-ai";
const KEYCHAIN_ACCOUNT: &str = "apiKey";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudAiSettings {
    /// Ex.: https://api.openai.com/v1 ou https://api.anthropic.com/... (só OpenAI-compat)
    pub base_url: String,
    pub model: String,
    pub enabled: bool,
}

impl Default for CloudAiSettings {
    fn default() -> Self {
        Self {
            base_url: "https://api.openai.com/v1".into(),
            model: "gpt-4o-mini".into(),
            enabled: false,
        }
    }
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
}

#[derive(Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Option<Vec<ChatChoice>>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: Option<ChatMessageOut>,
}

#[derive(Deserialize)]
struct ChatMessageOut {
    content: Option<String>,
}

pub fn save_api_key(api_key: &str) -> Result<(), String> {
    let status = Command::new("security")
        .args([
            "add-generic-password",
            "-U",
            "-s",
            KEYCHAIN_SERVICE,
            "-a",
            KEYCHAIN_ACCOUNT,
            "-w",
            api_key,
        ])
        .status()
        .map_err(|e| format!("keychain: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("Não gravei a chave da IA na nuvem no keychain".into())
    }
}

pub fn load_api_key() -> Option<String> {
    let out = Command::new("security")
        .args([
            "find-generic-password",
            "-s",
            KEYCHAIN_SERVICE,
            "-a",
            KEYCHAIN_ACCOUNT,
            "-w",
        ])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// Chama /chat/completions (OpenAI-compatível). Texto sai da máquina.
pub fn propose_cloud_review(text: &str, settings: &CloudAiSettings) -> Result<ReviewReport, String> {
    let api_key = load_api_key().ok_or(
        "Chave da IA na nuvem ausente. Guarde em Opções avançadas.",
    )?;
    let vocabulary = review::extract_vocabulary(text, 80);
    let prompt = review::fidelity_prompt(text, &vocabulary);
    let system = "Você corrige erros de OCR em livros. Responda SOMENTE um JSON array de {original, proposed, reason}. Nunca invente conteúdo nem reescreva o estilo.";

    let base = settings.base_url.trim_end_matches('/');
    let url = format!("{base}/chat/completions");
    let body = ChatRequest {
        model: settings.model.clone(),
        temperature: 0.1,
        messages: vec![
            ChatMessage {
                role: "system".into(),
                content: system.into(),
            },
            ChatMessage {
                role: "user".into(),
                content: prompt,
            },
        ],
    };

    let resp = ureq::post(&url)
        .set("Authorization", &format!("Bearer {api_key}"))
        .set("Content-Type", "application/json")
        .send_json(&body)
        .map_err(|e| format!("IA nuvem ({url}): {e}"))?;

    let parsed: ChatResponse = resp
        .into_json()
        .map_err(|e| format!("JSON IA nuvem: {e}"))?;
    let content = parsed
        .choices
        .and_then(|c| c.into_iter().next())
        .and_then(|c| c.message)
        .and_then(|m| m.content)
        .unwrap_or_default();

    let proposals = review::parse_llm_proposals(text, &content);
    Ok(ReviewReport {
        proposals,
        vocabulary,
        engine: format!("cloud:{}", settings.model),
        note: "IA na NUVEM — o texto saiu do seu computador. Nada entra sem você aceitar."
            .into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_default() {
        let s = CloudAiSettings::default();
        assert!(s.base_url.contains("openai"));
    }
}
