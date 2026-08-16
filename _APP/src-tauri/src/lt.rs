// ==============================================================================
// SCRIPT: lt.rs (melhorador-app)
// DESCRIÇÃO: LanguageTool local (URL) + Premium (keychain) — só propõe
// CHAMADO POR: comandos Tauri check_lt_*
// CONTRATO (RESPOSTA ESPERADA): DiffProposal[]; Premium avisa nuvem
// ==============================================================================

use melhorador_core::review::DiffProposal;
use serde::{Deserialize, Serialize};
use std::process::Command;

pub const DEFAULT_LT_URL: &str = "http://localhost:8081";
const KEYCHAIN_SERVICE: &str = "com.zedicoes.melhorador.lt-premium";
const KEYCHAIN_ACCOUNT: &str = "username";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LtSettings {
    pub local_url: String,
    /// Se true, UI deve mostrar aviso de nuvem antes de chamar Premium.
    pub premium_enabled: bool,
}

impl Default for LtSettings {
    fn default() -> Self {
        Self {
            local_url: DEFAULT_LT_URL.into(),
            premium_enabled: false,
        }
    }
}

#[derive(Deserialize)]
struct LtMatch {
    offset: usize,
    length: usize,
    message: Option<String>,
    replacements: Option<Vec<LtRepl>>,
}

#[derive(Deserialize)]
struct LtRepl {
    value: String,
}

#[derive(Deserialize)]
struct LtResponse {
    matches: Option<Vec<LtMatch>>,
}

pub fn check_local(text: &str, server_url: &str) -> Result<Vec<DiffProposal>, String> {
    // Chunk ~20k como o CLI.
    let chunk_size = 20_000usize;
    let mut proposals = Vec::new();
    let mut search = 0usize;
    let mut pos = 0usize;
    while pos < text.len() {
        let end = (pos + chunk_size).min(text.len());
        let chunk = &text[pos..end];
        let base = text.find(chunk).filter(|&i| i >= search).unwrap_or(pos);
        search = base + chunk.len();
        let matches = post_check(chunk, server_url, None)?;
        for m in matches {
            let Some(reps) = m.replacements.as_ref().filter(|r| !r.is_empty()) else {
                continue;
            };
            let start = base + m.offset;
            if start + m.length > text.len() {
                continue;
            }
            let original = text[start..start + m.length].to_string();
            let proposed = reps[0].value.clone();
            if original == proposed {
                continue;
            }
            if proposed.chars().count() > original.chars().count() + 8 {
                continue;
            }
            proposals.push(DiffProposal {
                original,
                proposed,
                reason: m.message.unwrap_or_else(|| "LanguageTool local".into()),
                byte_offset: start,
            });
            if proposals.len() >= 40 {
                return Ok(proposals);
            }
        }
        pos = end;
    }
    Ok(proposals)
}

/// Premium API (api.languagetoolplus.com) — requer username no keychain.
/// AVISO: texto sai da máquina.
pub fn check_premium(text: &str, username: &str, api_key: &str) -> Result<Vec<DiffProposal>, String> {
    let sample: String = text.chars().take(20_000).collect();
    let url = "https://api.languagetoolplus.com/v2/check";
    let body = ureq::post(url)
        .send_form(&[
            ("text", sample.as_str()),
            ("language", "pt-BR"),
            ("username", username),
            ("apiKey", api_key),
        ])
        .map_err(|e| format!("Premium LT: {e}"))?;
    let parsed: LtResponse = body
        .into_json()
        .map_err(|e| format!("JSON Premium: {e}"))?;
    let mut proposals = Vec::new();
    for m in parsed.matches.unwrap_or_default() {
        let Some(reps) = m.replacements.as_ref().filter(|r| !r.is_empty()) else {
            continue;
        };
        if m.offset + m.length > sample.len() {
            continue;
        }
        let original = sample[m.offset..m.offset + m.length].to_string();
        let proposed = reps[0].value.clone();
        if original == proposed || proposed.chars().count() > original.chars().count() + 8 {
            continue;
        }
        proposals.push(DiffProposal {
            original,
            proposed,
            reason: format!(
                "LT Premium (NUVEM): {}",
                m.message.unwrap_or_default()
            ),
            byte_offset: m.offset,
        });
        if proposals.len() >= 40 {
            break;
        }
    }
    Ok(proposals)
}

fn post_check(
    chunk: &str,
    server_url: &str,
    auth: Option<(&str, &str)>,
) -> Result<Vec<LtMatch>, String> {
    let url = format!("{}/v2/check", server_url.trim_end_matches('/'));
    let mut form = vec![("text", chunk), ("language", "pt-BR")];
    if let Some((u, k)) = auth {
        form.push(("username", u));
        form.push(("apiKey", k));
    }
    let resp = ureq::post(&url)
        .send_form(&form)
        .map_err(|e| format!("LT local ({url}): {e}"))?;
    let parsed: LtResponse = resp.into_json().map_err(|e| format!("JSON LT: {e}"))?;
    Ok(parsed.matches.unwrap_or_default())
}

pub fn save_premium_username(username: &str) -> Result<(), String> {
    // macOS keychain via `security`
    let status = Command::new("security")
        .args([
            "add-generic-password",
            "-U",
            "-s",
            KEYCHAIN_SERVICE,
            "-a",
            KEYCHAIN_ACCOUNT,
            "-w",
            username,
        ])
        .status()
        .map_err(|e| format!("keychain: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("Não gravei username no keychain".into())
    }
}

pub fn load_premium_username() -> Option<String> {
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

pub fn save_premium_api_key(api_key: &str) -> Result<(), String> {
    let status = Command::new("security")
        .args([
            "add-generic-password",
            "-U",
            "-s",
            KEYCHAIN_SERVICE,
            "-a",
            "apiKey",
            "-w",
            api_key,
        ])
        .status()
        .map_err(|e| format!("keychain: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("Não gravei apiKey no keychain".into())
    }
}

pub fn load_premium_api_key() -> Option<String> {
    let out = Command::new("security")
        .args([
            "find-generic-password",
            "-s",
            KEYCHAIN_SERVICE,
            "-a",
            "apiKey",
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
