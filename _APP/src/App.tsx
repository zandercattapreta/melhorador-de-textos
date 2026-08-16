// ==============================================================================
// SCRIPT: App.tsx (txtmelhorator-app)
// DESCRIÇÃO: Rotina completa — fila, conferência sync, regras (R4), revisão (R5)
// CHAMADO POR: main.tsx
// CONTRATO (RESPOSTA ESPERADA): processar → conferir página|texto → salvar; opt-in review
// ==============================================================================

import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open } from "@tauri-apps/plugin-dialog";
import "./App.css";

type OcrLang = "auto" | "por" | "eng" | "por+eng";
type Outcome = "ok" | "fail" | "skip" | "stop";
type TextView = "page" | "book";
type RuleKind = "header" | "note" | "no_join";

type ProcessResult = {
  source_name: string;
  source_path: string;
  engine: string;
  languages_used: string;
  page_count: number;
  cleaned: string;
  pages: string[];
  cleanup_stats: Record<string, number>;
  structure_stats: Record<string, number>;
  warnings: string[];
};

type UserRule = { kind: RuleKind; pattern: string };

type DiffProposal = {
  original: string;
  proposed: string;
  reason: string;
  byte_offset: number;
};

type ReviewReport = {
  proposals: DiffProposal[];
  vocabulary: string[];
  engine: string;
  note: string;
};

type ModelOffer = {
  id: string;
  label: string;
  detail: string;
  filename: string;
  url: string;
  available_locally: boolean;
};

const LS_LANG = "txtmelhorator.lang";
const LS_SAVE_DIR = "txtmelhorator.lastSaveDir";
const LS_VIEW = "txtmelhorator.textView";
const CANCELLED = "CANCELLED";

function loadLang(): OcrLang {
  const v = localStorage.getItem(LS_LANG);
  if (v === "auto" || v === "por" || v === "eng" || v === "por+eng") return v;
  return "por+eng";
}

function loadView(): TextView {
  return localStorage.getItem(LS_VIEW) === "book" ? "book" : "page";
}

function parentDir(path: string): string {
  const i = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  return i > 0 ? path.slice(0, i) : path;
}

function errText(e: unknown): string {
  return String(e);
}

function isCancelled(e: unknown): boolean {
  const s = errText(e);
  return s === CANCELLED || s.includes(CANCELLED);
}

function App() {
  const [dragging, setDragging] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [info, setInfo] = useState<string | null>(null);
  const [result, setResult] = useState<ProcessResult | null>(null);
  const [savedTo, setSavedTo] = useState<string | null>(null);
  const [progress, setProgress] = useState<{ done: number; total: number } | null>(null);
  const [partialText, setPartialText] = useState<string>("");
  const [processingPath, setProcessingPath] = useState<string | null>(null);
  const [lang, setLang] = useState<OcrLang>(loadLang);
  const [queue, setQueue] = useState<string[]>([]);
  const [queueIndex, setQueueIndex] = useState(0);
  const [confPage, setConfPage] = useState(1);
  const [pageImg, setPageImg] = useState<string | null>(null);
  const [pageBusy, setPageBusy] = useState(false);
  const [textView, setTextView] = useState<TextView>(loadView);
  const [rules, setRules] = useState<UserRule[]>([]);
  const [ruleKind, setRuleKind] = useState<RuleKind>("header");
  const [rulePattern, setRulePattern] = useState("");
  const [review, setReview] = useState<ReviewReport | null>(null);
  const [accepted, setAccepted] = useState<Set<number>>(new Set());
  const [acceptedTrail, setAcceptedTrail] = useState<DiffProposal[]>([]);
  const [reviewBusy, setReviewBusy] = useState(false);
  const [leftPanel, setLeftPanel] = useState<"none" | "revisao" | "ajustes">("none");
  const [gguf, setGguf] = useState<{
    selected: string | null;
    catalog: { name: string; bytes: number; source?: string }[];
  } | null>(null);
  const [ggufUrl, setGgufUrl] = useState("");
  const [ggufName, setGgufName] = useState("");
  const [ltUrl, setLtUrl] = useState("http://localhost:8081");
  const [ltUser, setLtUser] = useState("");
  const [ltKey, setLtKey] = useState("");
  const [cloudUrl, setCloudUrl] = useState("https://api.openai.com/v1");
  const [cloudModel, setCloudModel] = useState("gpt-4o-mini");
  const [cloudKey, setCloudKey] = useState("");
  const [modelOffers, setModelOffers] = useState<ModelOffer[]>([]);
  const [showModelUrlDownload, setShowModelUrlDownload] = useState(false);
  const langRef = useRef(lang);
  langRef.current = lang;
  const stopAllRef = useRef(false);
  const previewRef = useRef<HTMLPreElement>(null);

  useEffect(() => {
    localStorage.setItem(LS_LANG, lang);
  }, [lang]);

  useEffect(() => {
    localStorage.setItem(LS_VIEW, textView);
  }, [textView]);

  useEffect(() => {
    void invoke<UserRule[]>("list_user_rules")
      .then(setRules)
      .catch(() => setRules([]));
  }, []);

  useEffect(() => {
    // Durante o OCR a preview vem no evento — não reabrir o PDF (travava em Carregando).
    if (processingPath) return;
    const path = result?.source_path;
    if (!path || confPage < 1) return;
    let cancelled = false;
    setPageBusy(true);
    void invoke<string>("render_pdf_page", {
      path,
      page: confPage,
    })
      .then((url) => {
        if (!cancelled) setPageImg(url);
      })
      .catch((e) => {
        if (!cancelled) setError(errText(e));
      })
      .finally(() => {
        setPageBusy(false);
      });
    return () => {
      cancelled = true;
    };
  }, [result?.source_path, processingPath, confPage]);

  useEffect(() => {
    setConfPage(1);
    setReview(null);
    setAccepted(new Set());
    setAcceptedTrail([]);
    if (result?.source_path) setPageImg(null);
  }, [result?.source_path]);

  useEffect(() => {
    const unlisten = listen<{
      done: number;
      total: number;
      pageText: string;
      preview?: string | null;
    }>("extract-progress", (e) => {
      const page = e.payload.done;
      const total = e.payload.total;
      setProgress({ done: page, total });
      if (e.payload.preview) {
        setPageImg(e.payload.preview);
        setPageBusy(false);
      }
      const chunk = (e.payload.pageText ?? "").trim();
      if (chunk) {
        setPartialText(chunk);
        if (page >= 1) setConfPage(page);
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  async function promptAndSave(r: ProcessResult, format: "md" | "txt" | "docx") {
    const remembered = localStorage.getItem(LS_SAVE_DIR);
    const fallback = parentDir(r.source_path);
    const defaultPath = remembered && remembered.length > 0 ? remembered : fallback;
    try {
      const picked = await open({
        directory: true,
        multiple: false,
        defaultPath,
        title: "Onde salvar o texto melhorado?",
      });
      if (picked === null) return;
      const destDir = Array.isArray(picked) ? picked[0] : picked;
      if (!destDir) return;
      localStorage.setItem(LS_SAVE_DIR, destDir);
      const dest = await invoke<string>("save_result", {
        sourcePath: r.source_path,
        content: r.cleaned,
        format,
        destDir,
        engine: r.engine,
        languages: r.languages_used,
        pageCount: r.page_count,
        acceptedDiffs: acceptedTrail,
      });
      setSavedTo(dest);
    } catch (e) {
      setError(errText(e));
    }
  }

  const processOne = useCallback(async (path: string): Promise<Outcome> => {
    setBusy(true);
    setError(null);
    setInfo(null);
    setResult(null);
    setSavedTo(null);
    setProgress(null);
    setPartialText("");
    setProcessingPath(path);
    setConfPage(1);
    setPageImg(null);
    setPageBusy(false);
    stopAllRef.current = false;
    const isPdf = path.toLowerCase().endsWith(".pdf");
    if (isPdf) {
      // Primeira página antes do OCR (ainda sem lock no arquivo).
      setPageBusy(true);
      void invoke<string>("render_pdf_page", { path, page: 1 })
        .then((url) => setPageImg(url))
        .catch(() => undefined)
        .finally(() => setPageBusy(false));
    }
    try {
      const r = await invoke<ProcessResult>(
        isPdf ? "process_pdf" : "process_text_file",
        isPdf ? { path, languages: langRef.current } : { path },
      );
      setResult(r);
      await promptAndSave(r, "md");
      return "ok";
    } catch (e) {
      if (isCancelled(e)) {
        if (stopAllRef.current) {
          setInfo("Fila parada.");
          return "stop";
        }
        setInfo("Livro pulado.");
        return "skip";
      }
      setError(errText(e));
      return "fail";
    } finally {
      setBusy(false);
      setProgress(null);
      setProcessingPath(null);
    }
  }, []);

  const runQueue = useCallback(
    async (paths: string[], startAt = 0) => {
      setQueue(paths);
      for (let i = startAt; i < paths.length; i++) {
        setQueueIndex(i);
        const outcome = await processOne(paths[i]);
        if (outcome === "fail" || outcome === "stop") break;
      }
    },
    [processOne],
  );

  const handlePath = useCallback(
    async (path: string) => {
      setError(null);
      setInfo(null);
      try {
        const pdfs = await invoke<string[]>("list_pdfs_in_dir", { dir: path });
        await runQueue(pdfs);
        return;
      } catch {
        // não é pasta
      }
      setQueue([path]);
      setQueueIndex(0);
      await processOne(path);
    },
    [processOne, runQueue],
  );

  useEffect(() => {
    const unlisten = getCurrentWebview().onDragDropEvent((event) => {
      if (event.payload.type === "enter" || event.payload.type === "over") setDragging(true);
      else if (event.payload.type === "drop") {
        setDragging(false);
        const path = event.payload.paths[0];
        if (path) void handlePath(path);
      } else setDragging(false);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [handlePath]);

  async function openPdf() {
    const picked = await open({
      multiple: false,
      filters: [{ name: "PDF", extensions: ["pdf"] }],
      title: "Abrir PDF",
    });
    if (!picked) return;
    const path = Array.isArray(picked) ? picked[0] : picked;
    if (path) await handlePath(path);
  }

  async function openFolder() {
    const picked = await open({
      directory: true,
      multiple: false,
      title: "Abrir pasta com PDFs",
    });
    if (!picked) return;
    const path = Array.isArray(picked) ? picked[0] : picked;
    if (path) await handlePath(path);
  }

  async function requestStop(all: boolean) {
    stopAllRef.current = all;
    await invoke("request_cancel");
  }

  async function saveAgain(format: "md" | "txt" | "docx") {
    if (!result) return;
    await promptAndSave(result, format);
  }

  async function clearData() {
    try {
      const msg = await invoke<string>("clear_app_data");
      setInfo(msg);
      setError(null);
    } catch (e) {
      setError(errText(e));
    }
  }

  async function addRule() {
    const pattern = rulePattern.trim();
    if (!pattern) return;
    const next = [...rules, { kind: ruleKind, pattern }];
    try {
      await invoke("save_user_rules", { rules: next });
      setRules(next);
      setRulePattern("");
      setInfo("Regra salva. Vale no próximo processamento.");
    } catch (e) {
      setError(errText(e));
    }
  }

  async function removeRule(idx: number) {
    const next = rules.filter((_, i) => i !== idx);
    try {
      await invoke("save_user_rules", { rules: next });
      setRules(next);
    } catch (e) {
      setError(errText(e));
    }
  }

  async function runReview() {
    if (!result) return;
    setReviewBusy(true);
    setError(null);
    try {
      const report = await invoke<ReviewReport>("propose_review", {
        text: result.cleaned,
      });
      setReview(report);
      setAccepted(new Set(report.proposals.map((_, i) => i)));
      setInfo(report.note);
      setTextView("book");
    } catch (e) {
      setError(errText(e));
    } finally {
      setReviewBusy(false);
    }
  }

  async function runLtLocal() {
    if (!result) return;
    setReviewBusy(true);
    setError(null);
    setInfo("Consultando LanguageTool…");
    try {
      await invoke<string>("ensure_lt_server");
      const proposals = await invoke<DiffProposal[]>("check_lt_local", {
        text: result.cleaned,
      });
      setReview({
        proposals,
        vocabulary: [],
        engine: "LanguageTool",
        note:
          proposals.length === 0
            ? "LanguageTool não apontou correções neste texto."
            : `${proposals.length} sugestão(ões). Marque as que quiser e clique Aplicar.`,
      });
      setAccepted(new Set(proposals.map((_, i) => i)));
      setInfo(null);
      setTextView("book");
    } catch (e) {
      setError(errText(e));
      setInfo(null);
    } finally {
      setReviewBusy(false);
    }
  }

  async function runLtPremium() {
    if (!result) return;
    const ok = window.confirm(
      "LanguageTool Premium envia o texto para a internet (nuvem). Continuar?",
    );
    if (!ok) return;
    setReviewBusy(true);
    setError(null);
    try {
      const proposals = await invoke<DiffProposal[]>("check_lt_premium", {
        text: result.cleaned,
      });
      setReview({
        proposals,
        vocabulary: [],
        engine: "LanguageTool Premium (nuvem)",
        note: `${proposals.length} sugestão(ões) da nuvem. Revise antes de aplicar.`,
      });
      setAccepted(new Set());
      setTextView("book");
    } catch (e) {
      setError(errText(e));
    } finally {
      setReviewBusy(false);
    }
  }

  async function refreshModels() {
    try {
      const st = await invoke<{
        selected: string | null;
        catalog: { name: string; bytes: number; source?: string }[];
      }>("list_gguf_models");
      setGguf(st);
    } catch (e) {
      setError(errText(e));
    }
  }

  async function discoverLanguageTool() {
    try {
      const found = await invoke<{ found: boolean; url: string; detail: string }>(
        "discover_languagetool",
      );
      setLtUrl(found.url);
      setInfo(found.detail);
      setError(null);
    } catch (e) {
      setError(errText(e));
    }
  }

  async function refreshModelOffers() {
    try {
      const offers = await invoke<ModelOffer[]>("list_model_offers");
      setModelOffers(offers);
    } catch (e) {
      setError(errText(e));
    }
  }

  async function installModelOffer(offerId: string) {
    try {
      await invoke("install_model_offer", { offerId });
      await refreshModels();
      await refreshModelOffers();
      setInfo("Modelo configurado.");
      setError(null);
    } catch (e) {
      setError(errText(e));
    }
  }

  useEffect(() => {
    void refreshModels();
    void refreshModelOffers();
    void invoke<{ localUrl?: string; local_url?: string }>("get_lt_settings")
      .then((s) => setLtUrl(s.localUrl || s.local_url || "http://localhost:8081"))
      .catch(() => undefined);
    void invoke<{ baseUrl?: string; base_url?: string; model?: string }>("get_cloud_ai_settings")
      .then((s) => {
        setCloudUrl(s.baseUrl || s.base_url || "https://api.openai.com/v1");
        if (s.model) setCloudModel(s.model);
      })
      .catch(() => undefined);
  }, []);

  async function runCloudAi() {
    if (!result) return;
    const ok = window.confirm(
      "IA na nuvem: o texto do livro SAI do seu computador e vai para o serviço que você configurou. Continuar?",
    );
    if (!ok) return;
    setReviewBusy(true);
    setError(null);
    try {
      const report = await invoke<ReviewReport>("check_cloud_ai", {
        text: result.cleaned,
      });
      setReview(report);
      setAccepted(new Set());
      setInfo(report.note);
      setTextView("book");
    } catch (e) {
      setError(errText(e));
    } finally {
      setReviewBusy(false);
    }
  }

  async function applyAccepted() {
    if (!result || !review) return;
    const list = review.proposals.filter((_, i) => accepted.has(i));
    if (list.length === 0) {
      setInfo("Nenhuma sugestão marcada.");
      return;
    }
    try {
      const next = await invoke<string>("apply_review_diffs", {
        text: result.cleaned,
        accepted: list,
      });
      setResult({ ...result, cleaned: next, pages: [] });
      setAcceptedTrail((prev) => [...prev, ...list]);
      setReview(null);
      setAccepted(new Set());
      setInfo(
        `${list.length} correção(ões) aplicadas. Ainda não gravadas — use Salvar.`,
      );
      setTextView("book");
      // Mostra o texto já revisado no topo da caixa.
      requestAnimationFrame(() => jumpTo(0));
    } catch (e) {
      setError(errText(e));
    }
  }

  /** Prévia local das propostas marcadas (só visual até Aplicar). */
  function previewWithAccepted(
    text: string,
    proposals: DiffProposal[],
    marked: Set<number>,
  ): string {
    const list = proposals
      .map((p, i) => ({ p, i }))
      .filter(({ i }) => marked.has(i))
      .map(({ p }) => p)
      .sort((a, b) => b.byte_offset - a.byte_offset);
    let out = text;
    for (const p of list) {
      const from = Math.min(p.byte_offset, out.length);
      let at = out.indexOf(p.original, from);
      if (at < 0) at = out.indexOf(p.original);
      if (at < 0) continue;
      out = out.slice(0, at) + p.proposed + out.slice(at + p.original.length);
    }
    return out;
  }

  const s = result?.structure_stats ?? {};
  const c = result?.cleanup_stats ?? {};

  function jumpTo(fraction: number) {
    const el = previewRef.current;
    if (el) el.scrollTop = (el.scrollHeight - el.clientHeight) * fraction;
  }

  function focusWarnings() {
    jumpTo(0);
    previewRef.current?.focus();
  }

  const queueLabel =
    queue.length > 1 ? `Fila ${queueIndex + 1}/${queue.length}` : null;
  const showConference = !!result && result.page_count > 0;
  const showPdfPane = showConference || (!!busy && !!processingPath);
  const pdfPageTotal = result?.page_count ?? progress?.total ?? 0;

  function mapReviewEngineLabel(engine: string): string {
    const value = engine.trim().toLowerCase();
    if (value.startsWith("languagetool")) return "LanguageTool";
    if (value.startsWith("cloud")) return "IA na nuvem";
    if (value === "ia-local-indisponivel" || value === "ia-local-erro") return "IA local";
    if (value.startsWith("ia-local")) return "IA local";
    if (value.startsWith("basico")) return "Correções básicas";
    return "Sugestões";
  }

  const showingReviewPreview =
    !!result && !!review && accepted.size > 0 && textView === "book";

  const displayText = (() => {
    if (!result) return "";
    if (showingReviewPreview) {
      return previewWithAccepted(result.cleaned, review.proposals, accepted);
    }
    if (
      textView === "page" &&
      result.pages.length > 0 &&
      confPage >= 1 &&
      confPage <= result.pages.length
    ) {
      return result.pages[confPage - 1] ?? result.cleaned;
    }
    return result.cleaned;
  })();

  return (
    <main className="shell">
      <header className="topbar">
        <span className="brand">TXTMelhorator</span>
        <span className={`topbar-status${busy ? " is-busy" : ""}`}>
          {busy
            ? progress
              ? `${queueLabel ? queueLabel + " · " : ""}Página ${progress.done} / ${progress.total}`
              : `${queueLabel ? queueLabel + " · " : ""}Processando…`
            : result
              ? result.source_name
              : "Abra um PDF para começar"}
        </span>
      </header>

      <div className="workspace">
        {/* —— Coluna 1: Abrir —— */}
        <section className="col col-setup">
          <div className="col-head">
            <span className="col-title">Abrir</span>
          </div>
          <div className="col-body">
            <div className="stack">
              <button
                type="button"
                className="btn block"
                onClick={() => void openPdf()}
                disabled={busy}
              >
                Abrir PDF
              </button>
              <button
                type="button"
                className="btn ghost block"
                onClick={() => void openFolder()}
                disabled={busy}
              >
                Abrir pasta
              </button>
              {busy && (
                <div className="stack-row">
                  <button
                    type="button"
                    className="btn ghost"
                    onClick={() => void requestStop(false)}
                  >
                    Pular
                  </button>
                  <button
                    type="button"
                    className="btn ghost"
                    onClick={() => void requestStop(true)}
                  >
                    Parar
                  </button>
                </div>
              )}
            </div>

            <div className="field">
              <label htmlFor="ocr-lang">Idioma OCR</label>
              <select
                id="ocr-lang"
                value={lang}
                disabled={busy}
                onChange={(e) => setLang(e.target.value as OcrLang)}
              >
                <option value="auto">Auto</option>
                <option value="por+eng">Português + inglês</option>
                <option value="por">Só português</option>
                <option value="eng">Só inglês</option>
              </select>
            </div>

            <div
              className={`dropzone ${dragging ? "dragging" : ""} ${busy ? "busy" : ""}`}
            >
              {busy ? "Lendo o livro…" : "Arraste PDF ou pasta"}
              <small>{busy ? "Pular = este · Parar = fila" : "ou use os botões acima"}</small>
            </div>

            {error && <div className="banner error">{error}</div>}
            {info && <div className="banner warn">{info}</div>}
            {savedTo && <div className="banner ok">Salvo: {savedTo}</div>}

            {result && (
              <div className="stack">
                <div className="stack-row">
                  <button type="button" className="btn tiny" onClick={() => void saveAgain("md")}>
                    .md
                  </button>
                  <button type="button" className="btn ghost tiny" onClick={() => void saveAgain("txt")}>
                    .txt
                  </button>
                  <button type="button" className="btn ghost tiny" onClick={() => void saveAgain("docx")}>
                    .docx
                  </button>
                </div>
                {acceptedTrail.length > 0 && (
                  <p className="hint">{acceptedTrail.length} correção(ões) ainda não salvas</p>
                )}
              </div>
            )}

            <div className="rail-tabs">
              <button
                type="button"
                className={`btn ghost tiny${leftPanel === "revisao" ? " active" : ""}`}
                disabled={!result}
                onClick={() =>
                  setLeftPanel((p) => (p === "revisao" ? "none" : "revisao"))
                }
              >
                Revisão
              </button>
              <button
                type="button"
                className={`btn ghost tiny${leftPanel === "ajustes" ? " active" : ""}`}
                onClick={() =>
                  setLeftPanel((p) => (p === "ajustes" ? "none" : "ajustes"))
                }
              >
                Ajustes
              </button>
            </div>

            {leftPanel === "revisao" && result && (
              <div className="rail-panel">
                <h3>Revisão</h3>
                <p className="hint">Só sugere. Nada entra sem você aceitar.</p>
                <button
                  type="button"
                  className="btn block"
                  disabled={reviewBusy}
                  onClick={() => void runLtLocal()}
                >
                  {reviewBusy ? "Revisando…" : "LanguageTool"}
                </button>
                <button
                  type="button"
                  className="btn ghost block"
                  disabled={reviewBusy}
                  onClick={() => void runReview()}
                >
                  IA local
                </button>
                <button
                  type="button"
                  className="btn ghost block"
                  disabled={reviewBusy}
                  onClick={() => void runCloudAi()}
                >
                  IA na nuvem
                </button>
                <button
                  type="button"
                  className="btn ghost block"
                  disabled={reviewBusy}
                  onClick={() => void runLtPremium()}
                >
                  LT Premium
                </button>
              </div>
            )}

            {leftPanel === "ajustes" && (
              <div className="rail-panel">
                <h3>Regras do livro</h3>
                <div className="rules-form">
                  <select
                    value={ruleKind}
                    onChange={(e) => setRuleKind(e.target.value as RuleKind)}
                  >
                    <option value="header">Cabeçalho (remover)</option>
                    <option value="note">Nota</option>
                    <option value="no_join">Não juntar</option>
                  </select>
                  <input
                    type="text"
                    placeholder="Trecho a reconhecer…"
                    value={rulePattern}
                    onChange={(e) => setRulePattern(e.target.value)}
                  />
                  <button type="button" className="btn ghost" onClick={() => void addRule()}>
                    Adicionar regra
                  </button>
                </div>
                {rules.length > 0 && (
                  <ul className="rules-list">
                    {rules.map((r, i) => (
                      <li key={`${r.kind}-${r.pattern}-${i}`}>
                        <span>
                          {r.kind}: {r.pattern}
                        </span>
                        <button
                          type="button"
                          className="btn ghost tiny"
                          onClick={() => void removeRule(i)}
                        >
                          Remover
                        </button>
                      </li>
                    ))}
                  </ul>
                )}

                <h3>LanguageTool</h3>
                <div className="rules-form">
                  <input
                    type="text"
                    value={ltUrl}
                    onChange={(e) => setLtUrl(e.target.value)}
                    placeholder="http://localhost:8081"
                  />
                  <button
                    type="button"
                    className="btn ghost"
                    onClick={() => void discoverLanguageTool()}
                  >
                    Descobrir
                  </button>
                  <button
                    type="button"
                    className="btn ghost"
                    onClick={() =>
                      void invoke("save_lt_settings", {
                        settings: { localUrl: ltUrl, premiumEnabled: true },
                      })
                        .then(() => setInfo("URL salva"))
                        .catch((e) => setError(errText(e)))
                    }
                  >
                    Salvar URL
                  </button>
                  <input
                    type="text"
                    placeholder="Premium username"
                    value={ltUser}
                    onChange={(e) => setLtUser(e.target.value)}
                  />
                  <input
                    type="password"
                    placeholder="API key"
                    value={ltKey}
                    onChange={(e) => setLtKey(e.target.value)}
                  />
                  <button
                    type="button"
                    className="btn ghost"
                    onClick={() =>
                      void invoke("save_lt_premium_creds", {
                        username: ltUser,
                        apiKey: ltKey,
                      })
                        .then(() => {
                          setLtKey("");
                          setInfo("Credenciais no chaveiro");
                        })
                        .catch((e) => setError(errText(e)))
                    }
                  >
                    Guardar no Mac
                  </button>
                </div>

                <h3>IA local{gguf?.selected ? ` · ${gguf.selected}` : ""}</h3>
                {modelOffers.length > 0 ? (
                  <ul className="rules-list">
                    {modelOffers.map((offer) => (
                      <li key={offer.id}>
                        <span>
                          {offer.label} — {offer.detail}
                        </span>
                        <button
                          type="button"
                          className="btn ghost tiny"
                          onClick={() => void installModelOffer(offer.id)}
                        >
                          {offer.available_locally ? "Usar" : "Baixar"}
                        </button>
                      </li>
                    ))}
                  </ul>
                ) : (
                  <p className="hint">Nenhum modelo listado.</p>
                )}
                <button
                  type="button"
                  className="btn ghost tiny"
                  onClick={() => setShowModelUrlDownload((v) => !v)}
                >
                  {showModelUrlDownload ? "Ocultar URL" : "URL manual"}
                </button>
                {showModelUrlDownload && (
                  <div className="rules-form">
                    <input
                      type="text"
                      placeholder="URL .gguf"
                      value={ggufUrl}
                      onChange={(e) => setGgufUrl(e.target.value)}
                    />
                    <input
                      type="text"
                      placeholder="nome.gguf"
                      value={ggufName}
                      onChange={(e) => setGgufName(e.target.value)}
                    />
                    <button
                      type="button"
                      className="btn ghost"
                      onClick={() =>
                        void invoke("download_gguf_model", {
                          url: ggufUrl,
                          filename: ggufName || "model.gguf",
                          sha256: null,
                        })
                          .then(() => {
                            void refreshModels();
                            void refreshModelOffers();
                          })
                          .catch((e) => setError(errText(e)))
                      }
                    >
                      Baixar
                    </button>
                  </div>
                )}

                <h3>IA na nuvem</h3>
                <p className="hint">O texto sai do computador.</p>
                <div className="rules-form">
                  <input
                    type="text"
                    placeholder="URL base …/v1"
                    value={cloudUrl}
                    onChange={(e) => setCloudUrl(e.target.value)}
                  />
                  <input
                    type="text"
                    placeholder="modelo"
                    value={cloudModel}
                    onChange={(e) => setCloudModel(e.target.value)}
                  />
                  <button
                    type="button"
                    className="btn ghost"
                    onClick={() =>
                      void invoke("save_cloud_ai_settings", {
                        settings: {
                          baseUrl: cloudUrl,
                          model: cloudModel,
                          enabled: true,
                        },
                      })
                        .then(() => setInfo("URL/modelo salvos"))
                        .catch((e) => setError(errText(e)))
                    }
                  >
                    Salvar
                  </button>
                  <input
                    type="password"
                    placeholder="API key"
                    value={cloudKey}
                    onChange={(e) => setCloudKey(e.target.value)}
                  />
                  <button
                    type="button"
                    className="btn ghost"
                    onClick={() =>
                      void invoke("save_cloud_ai_key", { apiKey: cloudKey })
                        .then(() => {
                          setCloudKey("");
                          setInfo("Chave no chaveiro");
                        })
                        .catch((e) => setError(errText(e)))
                    }
                  >
                    Guardar chave
                  </button>
                </div>

                <button
                  type="button"
                  className="btn ghost"
                  disabled={busy}
                  onClick={() => void clearData()}
                >
                  Limpar dados do app
                </button>
              </div>
            )}
          </div>
        </section>

        {/* —— Coluna 2: PDF —— */}
        <section className="col col-pdf">
          <div className="col-head">
            <span className="col-title">Original</span>
            {showPdfPane && (
              <span className="hint">
                {confPage}
                {pdfPageTotal > 0 ? ` / ${pdfPageTotal}` : ""}
                {busy ? " · lendo" : ""}
              </span>
            )}
          </div>
          {showPdfPane ? (
            <>
              <div className="pdf-nav">
                <button
                  type="button"
                  className="btn ghost tiny"
                  disabled={busy || confPage <= 1 || pageBusy}
                  onClick={() => setConfPage((p) => Math.max(1, p - 1))}
                >
                  Ant
                </button>
                <span className="hint">Página</span>
                <button
                  type="button"
                  className="btn ghost tiny"
                  disabled={
                    busy || pdfPageTotal < 1 || confPage >= pdfPageTotal || pageBusy
                  }
                  onClick={() =>
                    setConfPage((p) =>
                      pdfPageTotal > 0 ? Math.min(pdfPageTotal, p + 1) : p,
                    )
                  }
                >
                  Próx
                </button>
              </div>
                <div className="page-frame">
                  {pageImg ? (
                    <img
                      src={pageImg}
                      alt={`Página ${confPage}`}
                      className="page-raster"
                    />
                  ) : pageBusy ? (
                    <span className="hint">Carregando…</span>
                  ) : (
                    <span className="hint">Sem imagem</span>
                  )}
                </div>
            </>
          ) : (
            <div className="page-frame empty">
              <p className="empty-hint">O PDF aparece aqui</p>
            </div>
          )}
        </section>

        {/* —— Coluna 3: Texto —— */}
        <section className="col col-text">
          <div className="col-head">
            <span className="col-title">Texto</span>
          </div>
          {busy && partialText && !result ? (
            <>
              <div className="text-toolbar">
                <p className="hint">Bruto desta página — limpeza no final</p>
              </div>
              <div className="book-pane">
                <pre className="preview partial">{partialText}</pre>
              </div>
            </>
          ) : result ? (
            <>
              <div className="text-toolbar">
                <h2 className="text-title">
                  {queueLabel ? `${queueLabel} · ` : ""}
                  {result.source_name}
                </h2>
                <div className="meta">
                  <span>{result.engine}</span>
                  <span>{result.languages_used}</span>
                  <span>{result.pages.length || "—"} págs.</span>
                  <span>
                    {(s.h1 ?? 0) + (s.h2 ?? 0) + (s.h3 ?? 0) + (s.h4 ?? 0)} títulos
                  </span>
                  <span>{s.prose ?? 0} §</span>
                  <span>{c.hyphenations_joined ?? 0} hífens</span>
                </div>
                {result.warnings.length > 0 && (
                  <button
                    type="button"
                    className="banner warn warn-click"
                    onClick={() => focusWarnings()}
                  >
                    {result.warnings.join(" · ")}
                  </button>
                )}
                <div className="seg">
                  <button
                    type="button"
                    className={textView === "page" ? "active" : undefined}
                    onClick={() => setTextView("page")}
                  >
                    Página
                  </button>
                  <button
                    type="button"
                    className={textView === "book" ? "active" : undefined}
                    onClick={() => setTextView("book")}
                  >
                    Livro
                  </button>
                  <button type="button" onClick={() => jumpTo(0)}>
                    Início
                  </button>
                  <button type="button" onClick={() => jumpTo(1)}>
                    Fim
                  </button>
                </div>
                {showingReviewPreview && (
                  <p className="hint">
                    Prévia com {accepted.size} sugestão(ões) marcada(s) — ainda não
                    gravadas. Clique Aplicar para confirmar.
                  </p>
                )}
              </div>
              <div className="book-pane">
                <pre ref={previewRef} tabIndex={0} className="preview">
                  {displayText}
                </pre>
              </div>
              {review && (
                <div className="suggestions">
                  <div className="suggestions-head">
                    <strong>{mapReviewEngineLabel(review.engine)}</strong>
                    <div className="stack-row">
                      <button
                        type="button"
                        className="btn ghost tiny"
                        onClick={() =>
                          setAccepted(new Set(review.proposals.map((_, i) => i)))
                        }
                      >
                        Todas
                      </button>
                      <button
                        type="button"
                        className="btn ghost tiny"
                        onClick={() => setAccepted(new Set())}
                      >
                        Nenhuma
                      </button>
                      <button
                        type="button"
                        className="btn tiny"
                        onClick={() => void applyAccepted()}
                      >
                        Aplicar
                      </button>
                    </div>
                  </div>
                  <p className="hint">{review.note}</p>
                  {review.proposals.length === 0 ? (
                    <p className="hint">Nenhuma sugestão.</p>
                  ) : (
                    <ul className="rules-list">
                      {review.proposals.map((p, i) => (
                        <li key={i}>
                          <label>
                            <input
                              type="checkbox"
                              checked={accepted.has(i)}
                              onChange={(e) => {
                                const next = new Set(accepted);
                                if (e.target.checked) next.add(i);
                                else next.delete(i);
                                setAccepted(next);
                              }}
                            />
                            <span>
                              <code>{p.original}</code> → <code>{p.proposed}</code>
                              <span className="hint"> — {p.reason}</span>
                            </span>
                          </label>
                        </li>
                      ))}
                    </ul>
                  )}
                </div>
              )}
            </>
          ) : (
            <div className="page-frame empty">
              <p className="empty-hint">O texto limpo aparece aqui</p>
            </div>
          )}
        </section>
      </div>
    </main>
  );
}

export default App;
