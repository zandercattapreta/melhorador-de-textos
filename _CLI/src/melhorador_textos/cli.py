# ==============================================================================
# SCRIPT: cli.py
# DESCRIÇÃO: CLI da PoC — extrai/limpa PDF e prepara revisão no LanguageTool
# CHAMADO POR: entry point `melhorador-textos` (pyproject scripts), terminal
# DEPENDÊNCIAS: argparse, json; módulos internos extraction/cleanup/languagetool_review
# CONTRATO (RESPOSTA ESPERADA): EXIT 0 = sucesso; escreve artefatos em _output/
# ==============================================================================
"""Interface de linha de comando da PoC.

Comandos:
- extract: recorta páginas, extrai (nativo/OCR), limpa e gera Markdown + relatório.
- prepare-lt: gera pacote de revisão manual do LanguageTool.
- import-lt: reimporta o texto corrigido e gera o diff.
"""

from __future__ import annotations

import argparse
import json
import sys
from datetime import datetime, timezone
from pathlib import Path

from . import __version__
from .cleanup import clean_text
from .extraction import extract_pages, parse_page_range
from .languagetool_review import import_correction, prepare_package, sha256_text
from .structure import apply_structure

# Diretórios padrão local-only (ver .gitignore).
_OUTPUT_ROOT = Path("_output")
_TEMP_ROOT = Path("_temp")


def _slug_pages(pages: list[int]) -> str:
    """Nome de pasta estável para a faixa, ex.: pages-021-030."""
    return f"pages-{pages[0]:03d}-{pages[-1]:03d}"


def run_extract(
    input_pdf: Path,
    pages: list[int],
    name: str,
    *,
    languages: str = "por+eng",
    reflow: bool = True,
    drop_leading_pages: int = 0,
) -> dict:
    """Executa o pipeline extract completo (extração → limpeza → estrutura).

    Reutilizável pelo comando `extract` e pelo batch (`batch_extract.py`).
    Retorna o report gravado em disco (inclui caminhos em `outputs`).
    """
    out_dir = _OUTPUT_ROOT / name / _slug_pages(pages)
    temp_dir = _TEMP_ROOT / name / _slug_pages(pages)
    out_dir.mkdir(parents=True, exist_ok=True)

    print(f"[extract] {input_pdf.name} páginas {pages[0]}–{pages[-1]}")
    result = extract_pages(input_pdf, pages, languages=languages, temp_dir=temp_dir)
    print(f"[extract] engine={result.engine} chars={len(result.raw_text.strip())}")

    raw_path = out_dir / "raw.txt"
    raw_path.write_text(result.raw_text, encoding="utf-8")

    cleaned = clean_text(
        result.raw_text,
        reflow=reflow,
        drop_leading_pages=drop_leading_pages,
    )
    # Estrutura Markdown (H1–H4 / SUMÁRIO) — heurísticas, sem IA.
    structured = apply_structure(cleaned.text)
    cleaned_path = out_dir / "cleaned.md"
    cleaned_path.write_text(structured.text, encoding="utf-8")

    report = {
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "tool_version": __version__,
        "input_pdf": input_pdf.name,
        "pages": pages,
        "drop_leading_pages": drop_leading_pages,
        "engine": result.engine,
        "languages": languages,
        "extraction_meta": result.meta,
        "cleanup_stats": cleaned.stats,
        "cleanup_warnings": cleaned.warnings,
        "structure_stats": structured.stats,
        "raw_sha256": sha256_text(result.raw_text),
        "cleaned_sha256": sha256_text(structured.text),
        "outputs": {
            "raw": str(raw_path),
            "cleaned": str(cleaned_path),
        },
    }
    report_path = out_dir / "report.json"
    report_path.write_text(
        json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8"
    )

    print(f"[extract] raw     -> {raw_path}")
    print(f"[extract] cleaned -> {cleaned_path}")
    print(f"[extract] report  -> {report_path}")
    print(
        "[extract] structure "
        f"h1={structured.stats['h1']} h2={structured.stats['h2']} "
        f"h3={structured.stats['h3']} h4={structured.stats['h4']} "
        f"toc={structured.stats['toc_entries']}"
    )
    for w in cleaned.warnings:
        print(f"[extract] AVISO: {w}")
    return report


def _cmd_extract(args: argparse.Namespace) -> int:
    run_extract(
        Path(args.input),
        parse_page_range(args.pages),
        args.name,
        languages=args.languages,
        reflow=not args.no_reflow,
        drop_leading_pages=args.drop_leading_pages,
    )
    return 0


def _cmd_prepare_lt(args: argparse.Namespace) -> int:
    cleaned_path = Path(args.input)
    cleaned_text = cleaned_path.read_text(encoding="utf-8")
    out_dir = cleaned_path.parent / "languagetool"
    pkg = prepare_package(cleaned_text, out_dir)
    print(f"[prepare-lt] original  -> {pkg.original_path}")
    print(f"[prepare-lt] manifest  -> {pkg.manifest_path}")
    print(f"[prepare-lt] chunks    -> {pkg.chunks}")
    print(
        "[prepare-lt] Revise no editor Premium (pt-BR) e salve como "
        f"{out_dir / 'corrected.md'}"
    )
    return 0


def _cmd_import_lt(args: argparse.Namespace) -> int:
    original_path = Path(args.original)
    corrected_path = Path(args.corrected)
    out_dir = corrected_path.parent
    manifest_path = out_dir / "manifest.json"
    result = import_correction(
        original_path,
        corrected_path,
        out_dir,
        manifest_path=manifest_path,
    )
    print(f"[import-lt] diff -> {result.diff_path}")
    print(f"[import-lt] alterações: {'sim' if result.changed else 'nenhuma'}")
    if result.source_hash_matches is False:
        print(
            "[import-lt] AVISO: original.txt não confere com o manifesto — "
            "o texto-base pode ter mudado desde o prepare-lt."
        )
    return 0


def _cmd_check_lt(args: argparse.Namespace) -> int:
    # Import tardio: módulo só é necessário neste comando.
    from .languagetool_local import DEFAULT_SERVER_URL, ensure_server, review_file

    server_url = args.server or DEFAULT_SERVER_URL
    if not ensure_server(server_url):
        raise RuntimeError(
            "servidor LanguageTool local indisponível — instale com "
            "`brew install languagetool` ou suba com `brew services start languagetool`"
        )
    result = review_file(Path(args.input), server_url=server_url)
    print(f"[check-lt] ocorrências : {result.stats['total_matches']}")
    print(f"[check-lt] na proposta : {result.stats['applied_in_proposal']}")
    print(f"[check-lt] sugestões   -> {result.suggestions_path}")
    print(f"[check-lt] proposta    -> {result.corrected_path}")
    print(f"[check-lt] diff        -> {result.diff_path}")
    print("[check-lt] Nada foi aplicado ao cleaned.md — revise o diff.")
    return 0


def _cmd_batch_extract(args: argparse.Namespace) -> int:
    # Import tardio: batch_extract importa run_extract deste módulo.
    from .batch_extract import batch_extract

    # Diretórios são fixos pelo contrato; falha claro se pedirem outros.
    if args.output_dir != "_output" or args.temp_dir != "_temp":
        raise ValueError(
            "saída/temp são fixos (_output/_temp) nesta versão — "
            "flags aceitas apenas com os valores padrão"
        )

    report = batch_extract(
        Path(args.input_dir),
        pages_spec=args.pages,
        full=args.full,
        languages=args.languages,
        max_retries=args.retry,
        resume=args.resume,
        lt_local=not args.no_lt,
    )
    # Falha em algum livro → exit 1 (fail-fast já interrompeu o batch).
    return 1 if report.failed else 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="melhorador-textos",
        description="PoC: extrai texto de PDF, limpa e prepara revisão no LanguageTool.",
    )
    parser.add_argument("--version", action="version", version=f"%(prog)s {__version__}")
    sub = parser.add_subparsers(dest="command", required=True)

    p_extract = sub.add_parser("extract", help="Extrai e limpa uma faixa de páginas")
    p_extract.add_argument("--input", required=True, help="Caminho do PDF de entrada")
    p_extract.add_argument("--pages", required=True, help="Faixa de páginas, ex.: 21-30")
    p_extract.add_argument(
        "--name", default="mesopotamia", help="Slug do documento em _output/"
    )
    p_extract.add_argument(
        "--languages", default="por+eng", help="Idiomas do Tesseract (ex.: por+eng)"
    )
    p_extract.add_argument(
        "--no-reflow", action="store_true", help="Não juntar linhas de parágrafo"
    )
    p_extract.add_argument(
        "--drop-leading-pages",
        type=int,
        default=0,
        help="Descarta N páginas iniciais da faixa (rosto/créditos)",
    )
    p_extract.set_defaults(func=_cmd_extract)

    p_batch = sub.add_parser(
        "batch-extract",
        help="Processa todos os PDFs de _originais/ (saídas em _output/)",
    )
    p_batch.add_argument(
        "--input-dir",
        default="_originais",
        help="Pasta com PDFs (padrão: _originais)",
    )
    # Saída/temp são fixos pelo contrato do projeto; flags aceitas por
    # compatibilidade com chamadores existentes (ex.: melhorar.sh).
    p_batch.add_argument(
        "--output-dir",
        default="_output",
        help="Saída (fixa em _output; aceito por compatibilidade)",
    )
    p_batch.add_argument(
        "--temp-dir",
        default="_temp",
        help="Temporários (fixo em _temp; aceito por compatibilidade)",
    )
    p_batch.add_argument(
        "--pages",
        default=None,
        help="Faixa-amostra por livro, ex.: 1-50 (padrão: 1-50)",
    )
    p_batch.add_argument(
        "--full",
        action="store_true",
        help="Processa o livro INTEIRO (OCR integral — autorização explícita)",
    )
    p_batch.add_argument(
        "--languages", default="por+eng", help="Idiomas do Tesseract (ex.: por+eng)"
    )
    p_batch.add_argument(
        "--retry",
        type=int,
        default=1,
        help="Tentativas por livro (padrão: 1 — fail-fast)",
    )
    p_batch.add_argument(
        "--resume",
        action="store_true",
        help="Retoma do checkpoint, pulando livros já concluídos",
    )
    p_batch.add_argument(
        "--no-lt",
        action="store_true",
        help="Pula a revisão automática no LanguageTool local",
    )
    p_batch.set_defaults(func=_cmd_batch_extract)

    p_lt = sub.add_parser(
        "check-lt", help="Revisão automática no LanguageTool LOCAL (gera proposta+diff)"
    )
    p_lt.add_argument("--input", required=True, help="Caminho do cleaned.md")
    p_lt.add_argument(
        "--server", default=None, help="URL do servidor local (padrão: localhost:8081)"
    )
    p_lt.set_defaults(func=_cmd_check_lt)

    p_prep = sub.add_parser("prepare-lt", help="Gera pacote de revisão manual do LT")
    p_prep.add_argument("--input", required=True, help="Caminho do cleaned.md")
    p_prep.set_defaults(func=_cmd_prepare_lt)

    p_imp = sub.add_parser("import-lt", help="Importa corrigido e gera diff")
    p_imp.add_argument("--original", required=True, help="original.txt do pacote")
    p_imp.add_argument("--corrected", required=True, help="corrected.md revisado")
    p_imp.set_defaults(func=_cmd_import_lt)

    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        return args.func(args)
    except (FileNotFoundError, ValueError, RuntimeError) as exc:
        # Fail-fast: reporta a causa e sai com erro, sem stack trace ruidoso.
        print(f"[erro] {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
