// ==============================================================================
// SCRIPT: llama_infer.rs (txtmelhorator-app)
// DESCRIÇÃO: Inferência GGUF in-process via llama.cpp (R5c) — sem app externo.
//            U1c: modelo RESIDENTE (carrega 1×, reutiliza entre páginas) com
//            fila de 1 inferência por vez; descarga explícita ao fim (R5d).
// CHAMADO POR: propose_review / unload_llama_model (lib.rs); bench via generate()
// CONTRATO: gera texto a partir do prompt; falha → Err em PT-BR
// ==============================================================================

use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::pin::pin;
use std::sync::{Arc, Mutex};

use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{AddBos, LlamaModel};
use llama_cpp_2::sampling::LlamaSampler;

/// Erro sentinela: geração interrompida pelo "Parar" do usuário.
/// lib.rs compara com esta constante para não mostrar erro assustador.
pub const CANCELLED: &str = "Revisão cancelada.";

/// Motor carregado. Ordem dos campos importa: `model` é liberado ANTES de
/// `backend` (o Drop do backend desfaz o singleton global do llama.cpp).
struct Engine {
    model: LlamaModel,
    backend: LlamaBackend,
    path: PathBuf,
}

/// Estado residente do motor de IA, gerenciado pelo Tauri (U1c).
/// O Mutex também é a FILA: uma inferência por vez; páginas seguintes aguardam
/// na trava — o OCR nunca espera, porque tudo roda fora da main thread.
#[derive(Clone)]
pub struct LlamaState {
    inner: Arc<Mutex<Option<Engine>>>,
}

impl LlamaState {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(None)),
        }
    }

    /// Descarrega o modelo residente (R5d — fim da sessão de revisão).
    /// Retorna true se havia modelo carregado.
    pub fn unload(&self) -> bool {
        let mut guard = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        guard.take().is_some()
    }

    /// Gera até `max_tokens` reutilizando o modelo residente. Carrega o GGUF
    /// só na 1ª chamada ou quando o usuário troca de modelo — nunca por página.
    /// BLOQUEANTE: chamar via spawn_blocking (nunca na main thread).
    pub fn generate(
        &self,
        model_path: &Path,
        prompt: &str,
        max_tokens: i32,
        should_stop: Option<&(dyn Fn() -> bool + Send + Sync)>,
    ) -> Result<String, String> {
        if !model_path.is_file() {
            return Err("Modelo GGUF não encontrado.".into());
        }
        if prompt.trim().is_empty() {
            return Err("Prompt vazio.".into());
        }

        // into_inner: se uma inferência anterior deu panic, recupera a trava
        // em vez de envenenar a revisão para sempre.
        let mut guard = self.inner.lock().unwrap_or_else(|p| p.into_inner());

        // Usuário cancelou enquanto esta página esperava a vez na fila.
        if should_stop.map(|f| f()).unwrap_or(false) {
            return Err(CANCELLED.into());
        }

        // Troca de GGUF: libera o antigo ANTES de carregar o novo
        // (memória de 6 GiB + backend é singleton global).
        if guard
            .as_ref()
            .map(|e| e.path.as_path() != model_path)
            .unwrap_or(false)
        {
            *guard = None;
        }
        if guard.is_none() {
            let backend =
                LlamaBackend::init().map_err(|e| format!("Não iniciei o motor de IA: {e}"))?;
            // Metal (Apple Silicon): offload agressivo; em CPU fica no processador.
            let model_params = LlamaModelParams::default().with_n_gpu_layers(99);
            let model_params = pin!(model_params);
            let model = LlamaModel::load_from_file(&backend, model_path, &model_params)
                .map_err(|e| format!("Não carreguei o modelo: {e}"))?;
            *guard = Some(Engine {
                model,
                backend,
                path: model_path.to_path_buf(),
            });
        }
        let engine = guard.as_ref().expect("engine carregado acima");
        infer(&engine.backend, &engine.model, prompt, max_tokens, should_stop)
    }
}

/// Uma geração com modelo já residente. Contexto novo por chamada é barato
/// (KV-cache pequeno); o caro — pesos de 6 GiB — fica residente no Engine.
fn infer(
    backend: &LlamaBackend,
    model: &LlamaModel,
    prompt: &str,
    max_tokens: i32,
    should_stop: Option<&(dyn Fn() -> bool + Send + Sync)>,
) -> Result<String, String> {
    let ctx_params = LlamaContextParams::default()
        .with_n_ctx(NonZeroU32::new(4096))
        .with_n_batch(512);

    let mut ctx = model
        .new_context(backend, ctx_params)
        .map_err(|e| format!("Não abri o contexto da IA: {e}"))?;

    let tokens = model
        .str_to_token(prompt, AddBos::Always)
        .map_err(|e| format!("Não tokenizei o prompt: {e}"))?;

    let n_ctx = ctx.n_ctx() as i32;
    let n_prompt = tokens.len() as i32;
    if n_prompt >= n_ctx - 64 {
        return Err(
            "Texto longo demais para a IA local nesta versão. Revise um trecho menor ou use LanguageTool."
                .into(),
        );
    }
    let n_len = (n_prompt + max_tokens).min(n_ctx - 1);

    let mut batch = LlamaBatch::new(512, 1);
    let last = (tokens.len() as i32) - 1;
    for (i, token) in (0_i32..).zip(tokens.into_iter()) {
        batch
            .add(token, i, &[0], i == last)
            .map_err(|e| format!("Lote IA: {e}"))?;
    }
    ctx.decode(&mut batch)
        .map_err(|e| format!("Decode do prompt falhou: {e}"))?;

    let mut sampler = LlamaSampler::chain_simple([
        LlamaSampler::temp(0.1),
        LlamaSampler::dist(1234),
        LlamaSampler::greedy(),
    ]);

    let mut decoder = encoding_rs::UTF_8.new_decoder();
    let mut out = String::new();
    let mut n_cur = batch.n_tokens();

    while n_cur <= n_len {
        // "Parar" do usuário derruba a geração no próximo token.
        if should_stop.map(|f| f()).unwrap_or(false) {
            return Err(CANCELLED.into());
        }
        let token = sampler.sample(&ctx, batch.n_tokens() - 1);
        sampler.accept(token);
        if model.is_eog_token(token) {
            break;
        }
        let piece = model
            .token_to_piece(token, &mut decoder, true, None)
            .map_err(|e| format!("Token IA: {e}"))?;
        out.push_str(&piece);
        batch.clear();
        batch
            .add(token, n_cur, &[0], true)
            .map_err(|e| format!("Lote IA: {e}"))?;
        ctx.decode(&mut batch)
            .map_err(|e| format!("Decode IA: {e}"))?;
        n_cur += 1;
    }

    Ok(out)
}
