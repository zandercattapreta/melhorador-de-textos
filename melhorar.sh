#!/bin/bash
# ==============================================================================
# SCRIPT: melhorar.sh
# DESCRIÇÃO: Pipeline completo — setup, batch-extract, check-lt (LanguageTool)
# CHAMADO POR: usuario final (bash melhorar.sh)
# SAÍDA: textos limpos + revisados com LanguageTool local
# ==============================================================================

set -e

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$PROJECT_DIR"

# Cores
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[✓]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[⚠]${NC} $1"; }
log_error() { echo -e "${RED}[✗]${NC} $1"; }
log_section() { echo -e "\n${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}\n${CYAN}$1${NC}\n${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}\n"; }

# ==============================================================================
# FASE 1: SETUP (Deps + Servidores)
# ==============================================================================

log_section "FASE 1: SETUP"

# Verifica Homebrew
if ! command -v brew &> /dev/null; then
    log_error "Homebrew não instalado"
    log_info "Instale com: /bin/bash -c \"\$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)\""
    exit 1
fi
log_success "Homebrew OK"

# Verifica/instala deps nativas
log_info "Verificando deps nativas..."
DEPS=("python@3.12" "tesseract" "tesseract-lang" "ghostscript" "qpdf" "unpaper" "languagetool")
for dep in "${DEPS[@]}"; do
    if ! brew list "$dep" &>/dev/null; then
        log_warn "Instalando $dep..."
        brew install "$dep" || { log_error "Falha em $dep"; exit 1; }
    fi
done
log_success "Todas as deps nativas OK"

# Setup Python venv
if [ ! -d ".venv" ]; then
    log_info "Criando .venv..."
    python3.12 -m venv .venv || { log_error "Falha ao criar .venv"; exit 1; }
fi

source .venv/bin/activate
log_success "Python venv ativado ($(python --version))"

# Instala deps Python
log_info "Instalando deps Python..."
pip install -q -e ".[dev]"
log_success "Deps Python OK"

# Inicia LanguageTool local
LT_PORT=8081
LT_LOG="/tmp/melhorador-languagetool.log"

log_info "Iniciando LanguageTool local (porta $LT_PORT)..."
pkill -f "languagetool --http" 2>/dev/null || true
sleep 1

languagetool --http --port $LT_PORT > "$LT_LOG" 2>&1 &
LT_PID=$!

# Aguarda servidor ficar pronto
echo -n "Aguardando servidor..."
for i in {1..30}; do
    if curl -s "http://localhost:$LT_PORT/v2/check" \
        -d "text=Teste&language=pt-BR" -m 2 &>/dev/null; then
        echo ""
        log_success "LanguageTool OK (PID $LT_PID)"
        break
    fi
    echo -n "."
    sleep 1

    if [ $i -eq 30 ]; then
        echo ""
        log_error "LanguageTool não respondeu"
        tail -20 "$LT_LOG"
        exit 1
    fi
done

# ==============================================================================
# FASE 2: BATCH-EXTRACT
# ==============================================================================

log_section "FASE 2: BATCH-EXTRACT"

if [ ! -d "_originais" ]; then
    log_error "Pasta _originais/ não encontrada"
    exit 1
fi

PDF_COUNT=$(ls -1 _originais/*.pdf 2>/dev/null | wc -l)
if [ "$PDF_COUNT" -eq 0 ]; then
    log_warn "Nenhum PDF encontrado em _originais/"
else
    log_info "Encontrados $PDF_COUNT PDFs em _originais/"
    log_info "Executando batch-extract..."

    melhorador-textos batch-extract \
        --input-dir _originais \
        --output-dir _output \
        --temp-dir _temp \
        --retry 1 || {
        log_error "Falha em batch-extract"
        exit 1
    }
    log_success "Batch-extract completo"
fi

# ==============================================================================
# FASE 3: CHECK-LT (LanguageTool Local)
# ==============================================================================

log_section "FASE 3: CHECK-LT (LANGUAGETOOL LOCAL)"

if [ ! -d "_output" ]; then
    log_warn "Nenhuma saída em _output/ — pulando check-lt"
else
    BOOKS=$(find _output -name "cleaned.md" -type f)
    BOOK_COUNT=$(echo "$BOOKS" | wc -l)

    if [ "$BOOK_COUNT" -eq 0 ]; then
        log_warn "Nenhum cleaned.md encontrado"
    else
        log_info "Encontrados $BOOK_COUNT livros para revisar"

        for cleaned_file in $BOOKS; do
            book_name=$(echo "$cleaned_file" | cut -d'/' -f2)
            log_info "Revisando: $book_name"

            melhorador-textos check-lt \
                --input "$cleaned_file" \
                --server "http://localhost:$LT_PORT" || {
                log_warn "Falha ao revisar $book_name — continuando"
            }
            log_success "$book_name revisado"
        done
        log_success "Check-lt completo para todos os livros"
    fi
fi

# ==============================================================================
# SUMÁRIO FINAL
# ==============================================================================

log_section "CONCLUÍDO ✓"

echo "📊 Resumo:"
echo "  ✓ Setup: Deps + Python + LanguageTool"
echo "  ✓ Batch-Extract: PDFs → cleaned.md"
echo "  ✓ Check-LT: cleaned.md → lt-local-*.{json,md,diff}"
echo ""
echo "📁 Saídas em _output/:"
echo "  └─ <livro>/pages-XXX-YYY/"
echo "     ├── raw.txt              (OCR bruto)"
echo "     ├── cleaned.md           (texto limpo)"
echo "     ├── report.json          (métricas)"
echo "     └── languagetool/"
echo "        ├── lt-local-suggestions.json    (ocorrências)"
echo "        ├── lt-local-corrected.md        (proposta)"
echo "        └── lt-local-changes.diff        (diff)"
echo ""
echo "📋 Próximos passos:"
echo "  1. Revisar propostas em lt-local-corrected.md"
echo "  2. Aprovar/rejeitar mudanças no diff"
echo "  3. (Opcional) import-lt para revisão Premium final"
echo ""
echo "🔗 LanguageTool servidor:"
echo "  URL: http://localhost:$LT_PORT"
echo "  Log: $LT_LOG"
echo "  PID: $LT_PID"
echo ""
echo "Para parar o servidor: kill $LT_PID"
echo ""
echo -e "${GREEN}Tudo pronto!${NC}"
