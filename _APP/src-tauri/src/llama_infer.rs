// ==============================================================================
// SCRIPT: llama_infer.rs (txtmelhorator-app)
// DESCRIÇÃO: Inferência GGUF in-process via llama.cpp (R5c) — sem app externo
// CHAMADO POR: propose_review (lib.rs)
// CONTRATO: gera texto a partir do prompt; falha → Err em PT-BR
// ==============================================================================

use std::num::NonZeroU32;
use std::path::Path;
use std::pin::pin;

use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{AddBos, LlamaModel};
use llama_cpp_2::sampling::LlamaSampler;

/// Gera até `max_tokens` a partir do prompt com o GGUF em `model_path`.
/// Carrega o modelo, gera, e libera (sob demanda — R5d).
pub fn generate(model_path: &Path, prompt: &str, max_tokens: i32) -> Result<String, String> {
    if !model_path.is_file() {
        return Err("Modelo GGUF não encontrado.".into());
    }
    if prompt.trim().is_empty() {
        return Err("Prompt vazio.".into());
    }

    let backend = LlamaBackend::init().map_err(|e| format!("Não iniciei o motor de IA: {e}"))?;

    // Metal (Apple Silicon): offload agressivo; em CPU fica no processador.
    let model_params = LlamaModelParams::default().with_n_gpu_layers(99);
    let model_params = pin!(model_params);

    let model = LlamaModel::load_from_file(&backend, model_path, &model_params)
        .map_err(|e| format!("Não carreguei o modelo: {e}"))?;

    let ctx_params = LlamaContextParams::default()
        .with_n_ctx(NonZeroU32::new(4096))
        .with_n_batch(512);

    let mut ctx = model
        .new_context(&backend, ctx_params)
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
