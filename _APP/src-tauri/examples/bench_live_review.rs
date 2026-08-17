// SCRIPT: bench_live_review.rs
// DESCRIÇÃO: Experimento — tempos de des-hífen vs 1× carga+inferência GGUF
// CHAMADO POR: cargo run --example bench_live_review -p txtmelhorator-app --release
// CONTRATO: imprime timings em stdout; não altera produto

use std::path::PathBuf;
use std::time::Instant;

fn main() {
    let sample_hyphen = "A civiliza-\nção antiga e a pala-\nvra partida no fim.";
    let sample_capa = r#"O mundo como vontade e como representação
Arthur Schopenhauer
1788-1860
Editora UNESP
01001-900 — São Paulo — SP
CIP — Brasil. Catalogação"#;

    println!("=== EXP bench_live_review ===");
    println!("cwd={}", std::env::current_dir().unwrap().display());

    // 1) Des-hifenização (core) — muitas vezes
    let t0 = Instant::now();
    let mut joined = 0u64;
    for _ in 0..1000 {
        let (out, n) = txtmelhorator_core::cleanup::dehyphenate(sample_hyphen);
        joined += n as u64;
        let _ = out.len();
    }
    let dehyph_ms = t0.elapsed().as_secs_f64() * 1000.0;
    println!(
        "dehyphenate x1000: {:.2} ms total ({:.4} ms/call), joins_total={}",
        dehyph_ms,
        dehyph_ms / 1000.0,
        joined
    );

    let (out_h, n_h) = txtmelhorator_core::cleanup::dehyphenate(sample_hyphen);
    println!("  hyphen sample -> joins={n_h} out={out_h:?}");
    let (out_c, n_c) = txtmelhorator_core::cleanup::dehyphenate(sample_capa);
    println!("  capa sample   -> joins={n_c} (quase zero = caixa 'não muda')");
    let _ = out_c;

    let heur = txtmelhorator_core::review::propose_heuristic_review(sample_hyphen);
    println!(
        "heuristic hyphen page: proposals={} note={}",
        heur.proposals.len(),
        heur.note
    );
    let heur_c = txtmelhorator_core::review::propose_heuristic_review(sample_capa);
    println!(
        "heuristic capa page:   proposals={} (por isso a capa parece 'sem melhoria')",
        heur_c.proposals.len()
    );

    // 2) Uma carga+geração GGUF (mesmo path do CoTypist se existir)
    let model = std::env::var("TXTMELHORATOR_GGUF").unwrap_or_else(|_| {
        dirs_next_cotypist()
            .unwrap_or_else(|| "(sem modelo)".into())
    });
    let path = PathBuf::from(&model);
    if !path.is_file() {
        println!("GGUF AUSENTE: {model}");
        println!("Defina TXTMELHORATOR_GGUF=/caminho/modelo.gguf");
        return;
    }
    println!("GGUF={}", path.display());
    println!("bytes={:.2} GiB", path.metadata().unwrap().len() as f64 / (1024.0 * 1024.0 * 1024.0));

    let prompt = txtmelhorator_core::review::fidelity_prompt(
        sample_hyphen,
        &["schopenhauer".into(), "vontade".into()],
    );
    println!("prompt_chars={}", prompt.chars().count());

    let t1 = Instant::now();
    let result = txtmelhorator_app_lib::llama_infer_bench(&path, &prompt, 64);
    let gen_s = t1.elapsed().as_secs_f64();
    match result {
        Ok(raw) => {
            println!("generate_once: {:.2} s OK out_chars={}", gen_s, raw.chars().count());
            println!("out_preview={}", raw.chars().take(200).collect::<String>().replace('\n', "\\n"));
        }
        Err(e) => println!("generate_once: {:.2} s ERR {e}", gen_s),
    }

    println!("=== se cada página OCR fizer 1× generate: 50 págs ≈ {:.0} s só de IA (serial) ===", gen_s * 50.0);
    println!("=== log do app: 17× print_info file size = 17 recargas na mesma sessão ===");
}

fn dirs_next_cotypist() -> Option<String> {
    let home = std::env::var_os("HOME")?;
    let p = PathBuf::from(home)
        .join("Library/Application Support/app.cotypist.Cotypist/Models/gemma-4-E4B-UD-Q5_K_XL.gguf");
    p.is_file().then(|| p.display().to_string())
}
