// ==============================================================================
// SCRIPT: lt.rs (txtmelhorator-app)
// DESCRIÇÃO: LanguageTool local (URL) + Premium (keychain) — UI aplica + Desfazer
// CHAMADO POR: comandos Tauri check_lt_*
// CONTRATO (RESPOSTA ESPERADA): DiffProposal[]; Premium avisa nuvem
// ==============================================================================

use txtmelhorator_core::review::DiffProposal;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;

pub const DEFAULT_LT_URL: &str = "http://localhost:8081";
const KEYCHAIN_SERVICE: &str = "com.zedicoes.txtmelhorator.lt-premium";
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

pub fn server_is_up(server_url: &str) -> bool {
    let url = format!("{}/v2/languages", server_url.trim_end_matches('/'));
    ureq::get(&url).timeout(std::time::Duration::from_secs(3)).call().is_ok()
}

/// Sobe `languagetool-server` se instalado e a URL estiver caída (como o CLI).
pub fn ensure_server(server_url: &str) -> Result<(), String> {
    if server_is_up(server_url) {
        return Ok(());
    }
    let binary = which("languagetool-server").ok_or_else(|| {
        "LanguageTool não encontrado neste Mac. Instale pelo site languagetool.org ou: brew install languagetool".to_string()
    })?;
    let port = url_port(server_url).unwrap_or(8081);
    Command::new(binary)
        .args(["--port", &port.to_string(), "--allow-origin"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("Não subi o LanguageTool: {e}"))?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(90);
    while std::time::Instant::now() < deadline {
        if server_is_up(server_url) {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(800));
    }
    Err(format!(
        "LanguageTool não respondeu a tempo (porta {port}). Verifique: brew services start languagetool"
    ))
}

/// Descobre LanguageTool no Mac e devolve URL pronta.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoverLtResult {
    pub found: bool,
    pub url: String,
    pub detail: String,
}

pub fn discover_languagetool() -> DiscoverLtResult {
    let default_url = DEFAULT_LT_URL.to_string();
    if server_is_up(&default_url) {
        return DiscoverLtResult {
            found: true,
            url: default_url,
            detail: "LanguageTool já está rodando neste Mac.".into(),
        };
    }
    if which("languagetool-server").is_some() {
        match ensure_server(&default_url) {
            Ok(()) => {
                return DiscoverLtResult {
                    found: true,
                    url: default_url,
                    detail: "LanguageTool encontrado e iniciado.".into(),
                };
            }
            Err(e) => {
                return DiscoverLtResult {
                    found: false,
                    url: default_url,
                    detail: e,
                };
            }
        }
    }
    // App "LanguageTool for Desktop" existe mas sem API local
    let desktop = PathBuf::from("/Applications/LanguageTool.app");
    let desktop_alt = dirs_home()
        .map(|h| h.join("Library/Application Support/LanguageTool for Desktop"))
        .filter(|p| p.exists());
    if desktop.exists() || desktop_alt.is_some() {
        return DiscoverLtResult {
            found: false,
            url: default_url,
            detail: "Há o app LanguageTool no Mac, mas ele não oferece API local. Use a versão de linha de comando (brew install languagetool) ou LanguageTool Premium na nuvem.".into(),
        };
    }
    DiscoverLtResult {
        found: false,
        url: default_url,
        detail: "LanguageTool não encontrado. Instale com: brew install languagetool".into(),
    }
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

fn which(cmd: &str) -> Option<PathBuf> {
    Command::new("which")
        .arg(cmd)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if s.is_empty() {
                None
            } else {
                Some(std::path::PathBuf::from(s))
            }
        })
}

fn url_port(url: &str) -> Option<u16> {
    let after = url.split("://").nth(1)?;
    let hostport = after.split('/').next()?;
    hostport.split(':').nth(1)?.parse().ok()
}

pub fn check_local(text: &str, server_url: &str) -> Result<Vec<DiffProposal>, String> {
    ensure_server(server_url)?;
    // Chunk ~20k como o CLI (limites de char, não byte).
    let chunk_size = 20_000usize;
    let mut proposals = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut char_pos = 0usize;
    let mut byte_base = 0usize;
    while char_pos < chars.len() {
        let end = (char_pos + chunk_size).min(chars.len());
        let chunk: String = chars[char_pos..end].iter().collect();
        let matches = post_check(&chunk, server_url, None)?;
        for m in matches {
            let Some(reps) = m.replacements.as_ref().filter(|r| !r.is_empty()) else {
                continue;
            };
            let start = byte_base + m.offset;
            if start + m.length > text.len() {
                continue;
            }
            let original = text[start..start + m.length].to_string();
            let proposed = reps[0].value.clone();
            if original == proposed {
                continue;
            }
            // Correção OCR pode alongar um pouco; LT costuma ser curto.
            if proposed.chars().count() > original.chars().count() + 24 {
                continue;
            }
            proposals.push(DiffProposal {
                original,
                proposed,
                reason: m.message.unwrap_or_else(|| "LanguageTool".into()),
                byte_offset: start,
            });
            if proposals.len() >= 200 {
                return Ok(proposals);
            }
        }
        byte_base += chunk.len();
        char_pos = end;
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
