# ==============================================================================
# SCRIPT: make_goldens.py
# DESCRIÇÃO: Gera golden masters do pipeline Python p/ validar o port Rust
# CHAMADO POR: desenvolvedor (PYTHONPATH=_CLI/src python _CLI/tools/make_goldens.py)
# DEPENDÊNCIAS: melhorador_textos (cleanup, structure), ftfy
# CONTRATO (RESPOSTA ESPERADA): _temp/goldens/<slug>/{raw.txt, clean_only.txt,
#   cleaned_golden.md, stats.json}; exit 0 = todos gerados e consistentes
# ==============================================================================
"""Congela as saídas de referência (golden masters) dos livros em _output/.

Para cada _output/<slug>/pages-XXX-YYY/ com raw.txt:
1. Verifica se ftfy.fix_text é identidade no raw (o port Rust assume isso).
2. Roda clean_text (defaults do batch: reflow=True, drop_leading_pages=0).
3. Roda apply_structure.
4. Grava raw + intermediário + final + stats em _temp/goldens/<slug>/.
5. Compara com o cleaned.md em disco (informativo).
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

import ftfy

from melhorador_textos.cleanup import clean_text
from melhorador_textos.structure import apply_structure

_OUTPUT = Path("_output")
_GOLDENS = Path("_temp/goldens")


def main() -> int:
    runs = sorted(_OUTPUT.glob("*/pages-*/raw.txt"))
    if not runs:
        print("[goldens] nenhum raw.txt em _output/ — nada a congelar")
        return 1

    ftfy_neutral = True
    for raw_path in runs:
        slug = raw_path.parent.parent.name
        out_dir = _GOLDENS / slug
        out_dir.mkdir(parents=True, exist_ok=True)

        raw = raw_path.read_text(encoding="utf-8")

        # 1. O port Rust trata o passo ftfy como identidade — provar aqui.
        fixed = ftfy.fix_text(raw)
        if fixed != raw:
            ftfy_neutral = False
            print(f"[goldens] AVISO: ftfy ALTEROU o texto de {slug} — "
                  "port Rust precisa cobrir essas correções!")

        # 2-3. Pipeline de referência com os defaults do batch.
        cleaned = clean_text(raw, reflow=True, drop_leading_pages=0)
        structured = apply_structure(cleaned.text)

        # 4. Grava gabaritos.
        (out_dir / "raw.txt").write_text(raw, encoding="utf-8")
        (out_dir / "clean_only.txt").write_text(cleaned.text, encoding="utf-8")
        (out_dir / "cleaned_golden.md").write_text(structured.text, encoding="utf-8")
        (out_dir / "stats.json").write_text(
            json.dumps(
                {"cleanup": cleaned.stats, "structure": structured.stats},
                ensure_ascii=False,
                indent=2,
            ),
            encoding="utf-8",
        )

        # 5. Confere contra o cleaned.md gerado pelo batch (informativo).
        disk = raw_path.parent / "cleaned.md"
        match = disk.exists() and disk.read_text(encoding="utf-8") == structured.text
        print(f"[goldens] {slug}: {len(raw)} chars raw -> "
              f"{len(structured.text)} chars golden · disco={'IGUAL' if match else 'DIFERE'}")

    print(f"[goldens] ftfy neutro no corpus: {'SIM' if ftfy_neutral else 'NÃO'}")
    print(f"[goldens] gabaritos em {_GOLDENS}/")
    return 0


if __name__ == "__main__":
    sys.exit(main())
