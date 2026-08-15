#!/usr/bin/env bash
# ==============================================================================
# SCRIPT: melhorar.sh
# DESCRIÇÃO: Entrada única do pipeline — processa todos os PDFs de _originais/
# CHAMADO POR: usuário, no terminal, na raiz do projeto (./melhorar.sh)
# CONTRATO (RESPOSTA ESPERADA): EXIT 0 = todos os livros OK; saídas em _output/
# ==============================================================================
#
# Uso:
#   ./melhorar.sh                 # amostra (págs. 1–50) de cada PDF
#   ./melhorar.sh --pages 1-30    # outra faixa-amostra
#   ./melhorar.sh --full          # livro INTEIRO (OCR integral — demorado)
#   ./melhorar.sh --resume        # retoma de onde parou após uma falha
#
# Flags extras são repassadas ao comando `melhorador-textos batch-extract`.

set -euo pipefail

# Sempre opera na raiz do projeto, onde ficam _originais/ e _output/.
cd "$(dirname "$0")"

# Fail-fast de ambiente: sem .venv não há pipeline.
if [ ! -d .venv ]; then
    echo "[erro] .venv não encontrado. Prepare o ambiente primeiro:" >&2
    echo "  python3.12 -m venv .venv" >&2
    echo "  source .venv/bin/activate" >&2
    echo "  pip install -e '.[dev]'" >&2
    exit 1
fi

# shellcheck disable=SC1091
source .venv/bin/activate

if ! command -v melhorador-textos >/dev/null 2>&1; then
    echo "[erro] CLI não instalado no .venv. Rode: pip install -e '.[dev]'" >&2
    exit 1
fi

exec melhorador-textos batch-extract "$@"
