// ==============================================================================
// SCRIPT: App.tsx (melhorador-app)
// DESCRIÇÃO: Tela inicial — dropzone, processamento real e exportação
// CHAMADO POR: main.tsx
// CONTRATO (RESPOSTA ESPERADA): arquivo solto na janela → texto limpo + stats
// ==============================================================================

import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import "./App.css";

type ProcessResult = {
  source_name: string;
  engine: string;
  cleaned: string;
  cleanup_stats: Record<string, number>;
  structure_stats: Record<string, number>;
  warnings: string[];
};

function App() {
  const [dragging, setDragging] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [sourcePath, setSourcePath] = useState<string | null>(null);
  const [result, setResult] = useState<ProcessResult | null>(null);
  const [savedTo, setSavedTo] = useState<string | null>(null);
  const [progress, setProgress] = useState<{ done: number; total: number } | null>(null);
  const [partialText, setPartialText] = useState<string>("");

  // Progresso do OCR por página (contador + texto bruto parcial).
  useEffect(() => {
    const unlisten = listen<{ done: number; total: number; pageText: string }>(
      "extract-progress",
      (e) => {
        setProgress({ done: e.payload.done, total: e.payload.total });
        // Mantém só o rabo do texto (últimas ~12k) para não pesar o DOM.
        setPartialText((prev) => (prev + "\n" + e.payload.pageText).slice(-12000));
      },
    );
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // Drag-and-drop nativo do Tauri: entrega caminhos reais do sistema.
  useEffect(() => {
    const unlisten = getCurrentWebview().onDragDropEvent((event) => {
      if (event.payload.type === "enter" || event.payload.type === "over") setDragging(true);
      else if (event.payload.type === "drop") {
        setDragging(false);
        const path = event.payload.paths[0];
        if (path) processFile(path);
      } else setDragging(false);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  async function processFile(path: string) {
    setBusy(true);
    setError(null);
    setResult(null);
    setSavedTo(null);
    setProgress(null);
    setPartialText("");
    setSourcePath(path);
    const isPdf = path.toLowerCase().endsWith(".pdf");
    try {
      const r = await invoke<ProcessResult>(
        isPdf ? "process_pdf" : "process_text_file",
        { path },
      );
      setResult(r);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
      setProgress(null);
    }
  }

  async function save(format: "md" | "txt") {
    if (!result || !sourcePath) return;
    try {
      const dest = await invoke<string>("save_result", {
        sourcePath,
        content: result.cleaned,
        format,
      });
      setSavedTo(dest);
    } catch (e) {
      setError(String(e));
    }
  }

  const s = result?.structure_stats ?? {};
  const c = result?.cleanup_stats ?? {};
  const previewRef = useRef<HTMLPreElement>(null);

  // Salto rápido no texto completo (sumário no fim, corpo no meio, etc.).
  function jumpTo(fraction: number) {
    const el = previewRef.current;
    if (el) el.scrollTop = (el.scrollHeight - el.clientHeight) * fraction;
  }

  return (
    <main className="shell">
      <header className="topbar">
        <span className="brand">⬒ Melhorador de Textos</span>
        <span className="hint">
          Seu texto nunca é reescrito nem inventado — só limpo e organizado, 100% no seu computador
        </span>
      </header>

      <section
        className={`dropzone ${dragging ? "dragging" : ""} ${busy ? "busy" : ""}`}
      >
        {busy
          ? progress
            ? `Lendo página ${progress.done} de ${progress.total}…`
            : "Processando…"
          : "Arraste o PDF do livro aqui (ou um .txt/.md)"}
        <small>
          {busy && progress
            ? "O texto reconhecido aparece abaixo conforme as páginas são lidas"
            : "Livro escaneado inteiro leva alguns minutos — você acompanha página a página"}
        </small>
      </section>

      {error && <div className="banner error">{error}</div>}
      {savedTo && <div className="banner ok">Salvo em: {savedTo}</div>}

      {busy && partialText && (
        <section className="result">
          <div className="stats">
            <span>texto bruto parcial — a limpeza acontece ao final</span>
          </div>
          <pre className="preview partial">{partialText}</pre>
        </section>
      )}

      {result && (
        <section className="result">
          <div className="result-head">
            <h2>{result.source_name}</h2>
            <div className="actions">
              <button onClick={() => save("md")}>Salvar .md</button>
              <button onClick={() => save("txt")}>Salvar .txt</button>
            </div>
          </div>

          <div className="stats">
            <span>motor: {result.engine}</span>
            <span>títulos: {(s.h1 ?? 0) + (s.h2 ?? 0) + (s.h3 ?? 0) + (s.h4 ?? 0)}</span>
            <span>parágrafos: {s.prose ?? 0}</span>
            <span>sumário: {s.toc_entries ?? 0} itens</span>
            <span>hifenizações unidas: {c.hyphenations_joined ?? 0}</span>
            <span>cabeçalhos removidos: {c.headers_removed ?? 0}</span>
            <span>nºs de página removidos: {c.page_numbers_removed ?? 0}</span>
          </div>

          {result.warnings.length > 0 && (
            <div className="banner warn">{result.warnings.join(" · ")}</div>
          )}

          <div className="jump">
            <span className="hint">Ir para:</span>
            <button onClick={() => jumpTo(0)}>Início</button>
            <button onClick={() => jumpTo(0.5)}>Meio</button>
            <button onClick={() => jumpTo(1)}>Fim</button>
          </div>
          <pre ref={previewRef} className="preview full">
            {result.cleaned}
          </pre>
        </section>
      )}
    </main>
  );
}

export default App;
