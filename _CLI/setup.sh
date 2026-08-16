#!/bin/bash
# ==============================================================================
# SCRIPT: setup.sh
# DESCRIÇÃO: Verifica/instala dependências e lança servidores (LanguageTool)
# CHAMADO POR: usuario final (bash setup.sh)
# SAÍDA: ambiente pronto, servidores rodando, logs em /tmp/melhorador-*.log
# ==============================================================================

set -e

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$PROJECT_DIR"

# Cores para output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Funções helper
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[✓]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[⚠]${NC} $1"
}

log_error() {
    echo -e "${RED}[✗]${NC} $1"
}

# ==============================================================================
# 1. VERIFICAR E INSTALAR DEPENDÊNCIAS NATIVAS
# ==============================================================================

log_info "Verificando dependências nativas..."

check_brew() {
    if ! command -v brew &> /dev/null; then
        log_error "Homebrew não instalado"
        log_info "Instale com: /bin/bash -c \"\$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)\""
        return 1
    fi
    log_success "Homebrew encontrado"
    return 0
}

check_and_install_brew_package() {
    local pkg=$1
    local brew_name=${2:-$pkg}

    if ! command -v "$pkg" &> /dev/null; then
        log_warn "$pkg não instalado — instalando via brew..."
        brew install "$brew_name" || {
            log_error "Falha ao instalar $brew_name"
            return 1
        }
        log_success "$brew_name instalado"
    else
        log_success "$pkg já instalado"
    fi
    return 0
}

# Verifica Homebrew
if ! check_brew; then
    exit 1
fi

# Instala deps nativas
NATIVE_DEPS=(
    "python@3.12:python@3.12"
    "tesseract:tesseract"
    "tesseract-lang:tesseract-lang"
    "ghostscript:ghostscript"
    "qpdf:qpdf"
    "unpaper:unpaper"
    "languagetool:languagetool"
)

for dep in "${NATIVE_DEPS[@]}"; do
    IFS=':' read -r cmd brew_pkg <<< "$dep"
    check_and_install_brew_package "$cmd" "$brew_pkg" || {
        log_error "Não foi possível instalar $brew_pkg — abortando"
        exit 1
    }
done

log_success "Todas as dependências nativas OK"

# ==============================================================================
# 2. VERIFICAR E SETUP PYTHON ENVIRONMENT
# ==============================================================================

log_info "Configurando ambiente Python..."

if [ ! -d ".venv" ]; then
    log_warn ".venv não existe — criando..."
    python3.12 -m venv .venv || {
        log_error "Falha ao criar .venv"
        exit 1
    }
    log_success ".venv criado"
else
    log_success ".venv já existe"
fi

# Ativa venv
source .venv/bin/activate || {
    log_error "Falha ao ativar .venv"
    exit 1
}

log_success "Python venv ativado ($(python --version))"

# Instala deps Python
log_info "Instalando dependências Python..."
pip install -q -e ".[dev]" || {
    log_error "Falha ao instalar deps Python"
    exit 1
}
log_success "Deps Python OK"

# ==============================================================================
# 3. VERIFICAR E LANÇAR SERVIDORES
# ==============================================================================

log_info "Iniciando servidores..."

# LanguageTool Local
LT_PORT=8081
LT_LOG="/tmp/melhorador-languagetool.log"

# Mata qualquer servidor anterior
pkill -f "languagetool --http" 2>/dev/null || true
sleep 1

log_info "Iniciando LanguageTool local (porta $LT_PORT)..."
languagetool --http --port $LT_PORT > "$LT_LOG" 2>&1 &
LT_PID=$!
log_info "LanguageTool PID: $LT_PID"

# Aguarda servidor ficar pronto (até 30s)
echo -n "Aguardando servidor LanguageTool..."
for i in {1..30}; do
    if curl -s "http://localhost:$LT_PORT/v2/check" \
        -d "text=Teste&language=pt-BR" \
        -m 2 &>/dev/null; then
        echo ""
        log_success "LanguageTool respondendo em http://localhost:$LT_PORT"
        break
    fi
    echo -n "."
    sleep 1

    if [ $i -eq 30 ]; then
        echo ""
        log_error "LanguageTool não respondeu após 30s"
        log_info "Verifique logs em: $LT_LOG"
        tail -20 "$LT_LOG"
        exit 1
    fi
done

# ==============================================================================
# 4. RESUMO E INSTRUÇÕES
# ==============================================================================

log_success "Setup completo! ✓"
echo ""
echo -e "${BLUE}=== RESUMO ===${NC}"
echo "✓ Dependências nativas: $(echo ${NATIVE_DEPS[@]} | wc -w) OK"
echo "✓ Python 3.12 + venv + deps"
echo "✓ LanguageTool local rodando (porta $LT_PORT, PID $LT_PID)"
echo ""
echo -e "${BLUE}=== PRÓXIMOS PASSOS ===${NC}"
echo "1. Ativar ambiente:"
echo "   source .venv/bin/activate"
echo ""
echo "2. Rodar batch-extract (já pronto em _output/):"
echo "   melhorador-textos batch-extract --input-dir _originais"
echo ""
echo "3. Revisar textos com LanguageTool local:"
echo "   melhorador-textos check-lt --input _output/<livro>/pages-XXX-YYY/cleaned.md"
echo ""
echo "4. Ver sugestões em:"
echo "   _output/<livro>/pages-XXX-YYY/languagetool/"
echo "   ├── lt-local-suggestions.json  (todas as ocorrências)"
echo "   ├── lt-local-corrected.md      (proposta com 1ª sugestão)"
echo "   └── lt-local-changes.diff      (diff para aprovação)"
echo ""
echo -e "${BLUE}=== LOGS ===${NC}"
echo "LanguageTool: $LT_LOG"
echo ""
echo -e "${GREEN}Ambiente pronto!${NC}"
