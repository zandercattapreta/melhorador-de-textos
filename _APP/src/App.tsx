// ==============================================================================
// SCRIPT: App.tsx (melhorador-app)
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

const LS_LANG = "melhorador.lang";
const LS_SAVE_DIR = "melhorador.lastSaveDir";
const LS_VIEW = "melhorador.textView";
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
  const [showAdvanced, setShowAdvanced] = useState(false);
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
    if (!result || result.page_count < 1) {
      setPageImg(null);
      return;
    }
    let cancelled = false;
    setPageBusy(true);
    void invoke<string>("render_pdf_page", {
      path: result.source_path,
      page: confPage,
    })
      .then((url) => {
        if (!cancelled) setPageImg(url);
      })
      .catch((e) => {
        if (!cancelled) setError(errText(e));
      })
      .finally(() => {
        if (!cancelled) setPageBusy(false);
      });
    return () => {
      cancelled = true;
    };
  }, [result, confPage]);

  useEffect(() => {
    setConfPage(1);
    setPageImg(null);
    setReview(null);
    setAccepted(new Set());
    setAcceptedTrail([]);
  }, [result?.source_path]);

  useEffect(() => {
    const unlisten = listen<{ done: number; total: number; pageText: string }>(
      "extract-progress",
      (e) => {
        setProgress({ done: e.payload.done, total: e.payload.total });
        setPartialText((prev) => (prev + "\n" + e.payload.pageText).slice(-12000));
      },
    );
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
    stopAllRef.current = false;
    const isPdf = path.toLowerCase().endsWith(".pdf");
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
    } catch (e) {
      setError(errText(e));
    }
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

  function mapReviewEngineLabel(engine: string): string {
    const value = engine.trim().toLowerCase();
    if (value.startsWith("languagetool")) return "LanguageTool";
    if (value.startsWith("cloud")) return "IA na nuvem";
    if (value === "ia-local-indisponivel" || value === "ia-local-erro") return "IA local";
    if (value.startsWith("ia-local")) return "IA local";
    if (value.startsWith("basico")) return "Correções básicas";
    return "Sugestões";
  }

  const displayText = (() => {
    if (!result) return "";
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
        <span className="brand">⬒ Melhorador de Textos</span>
        <span className="hint">
          Seu texto nunca é reescrito nem inventado — só limpo e organizado, 100% no seu computador
        </span>
      </header>

      <div className="workspace">
        <section className="col col-setup">
          <div className="col-title">Setup</div>
          <div className="col-scroll">
            <section className="toolbar" style={{ margin: 0 }}>
              <div className="actions">
                <button type="button" onClick={() => void openPdf()} disabled={busy}>
                  Abrir PDF
                </button>
                <button type="button" onClick={() => void openFolder()} disabled={busy}>
                  Abrir pasta
                </button>
                {busy && (
                  <>
                    <button
                      type="button"
                      className="secondary"
                      onClick={() => void requestStop(false)}
                    >
                      Pular
                    </button>
                    <button
                      type="button"
                      className="secondary"
                      onClick={() => void requestStop(true)}
                    >
                      Parar
                    </button>
                  </>
                )}
                <button
                  type="button"
                  className="secondary"
                  onClick={() => void clearData()}
                  disabled={busy}
                >
                  Limpar dados
                </button>
              </div>
              <label className="lang">
                Idioma OCR
                <select
                  value={lang}
                  disabled={busy}
                  onChange={(e) => setLang(e.target.value as OcrLang)}
                >
                  <option value="auto">Auto (detectar)</option>
                  <option value="por+eng">Português + inglês</option>
                  <option value="por">Só português</option>
                  <option value="eng">Só inglês</option>
                </select>
              </label>
            </section>

            <section
              className={`dropzone ${dragging ? "dragging" : ""} ${busy ? "busy" : ""}`}
              style={{ margin: "8px 0", padding: "14px 12px" }}
            >
              {busy
                ? progress
                  ? `${queueLabel ? queueLabel + " · " : ""}Lendo página ${progress.done} de ${progress.total}…`
                  : `${queueLabel ? queueLabel + " · " : ""}Processando…`
                : "Arraste PDF/pasta aqui"}
              <small>
                {busy && progress
                  ? "Pular = este livro · Parar = fila"
                  : "Ou use Abrir PDF / Abrir pasta"}
              </small>
            </section>

            {error && <div className="banner error">{error}</div>}
            {info && <div className="banner warn">{info}</div>}
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
              <div className="review-panel">
                <div className="rules-head">
                  <strong>Revisão</strong>
                  <span className="hint">Só sugere. Você decide o que entra.</span>
                </div>
                <div className="actions" style={{ marginBottom: 8, flexWrap: "wrap" }}>
                  <button
                    type="button"
                    disabled={reviewBusy}
                    onClick={() => void runLtLocal()}
                  >
                    {reviewBusy ? "Revisando…" : "Revisar com LanguageTool"}
                  </button>
                  <button
                    type="button"
                    className="secondary"
                    disabled={reviewBusy}
                    onClick={() => void runReview()}
                  >
                    Revisar com IA local
                  </button>
                  <button
                    type="button"
                    className="secondary"
                    disabled={reviewBusy}
                    onClick={() => void runCloudAi()}
                  >
                    Revisar com IA na nuvem
                  </button>
                  <button
                    type="button"
                    className="secondary"
                    disabled={reviewBusy}
                    onClick={() => void runLtPremium()}
                  >
                    LanguageTool Premium
                  </button>
                </div>
                <div className="actions" style={{ flexWrap: "wrap" }}>
                  <button type="button" onClick={() => void saveAgain("md")}>
                    Salvar .md
                  </button>
                  <button type="button" onClick={() => void saveAgain("txt")}>
                    Salvar .txt
                  </button>
                  <button type="button" onClick={() => void saveAgain("docx")}>
                    Salvar .docx
                  </button>
                </div>
                {acceptedTrail.length > 0 && (
                  <p className="hint" style={{ marginTop: 8 }}>
                    {acceptedTrail.length} correção(ões) aplicadas e ainda não salvas.
                  </p>
                )}
              </div>
            )}

            <section className="rules-panel" style={{ marginTop: 8 }}>
              <div className="rules-head">
                <strong>Opções avançadas</strong>
                <button
                  type="button"
                  className="secondary"
                  onClick={() => setShowAdvanced((v) => !v)}
                >
                  {showAdvanced ? "Ocultar" : "Mostrar"}
                </button>
              </div>
              {showAdvanced && (
                <>
                  <p className="hint">Regras do livro (antes do próximo processamento)</p>
                  <div className="rules-form">
                    <select
                      value={ruleKind}
                      onChange={(e) => setRuleKind(e.target.value as RuleKind)}
                    >
                      <option value="header">É cabeçalho (remover)</option>
                      <option value="note">É nota</option>
                      <option value="no_join">Não juntar</option>
                    </select>
                    <input
                      type="text"
                      placeholder="Trecho a reconhecer…"
                      value={rulePattern}
                      onChange={(e) => setRulePattern(e.target.value)}
                    />
                    <button type="button" className="secondary" onClick={() => void addRule()}>
                      Adicionar
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
                            className="secondary"
                            onClick={() => void removeRule(i)}
                          >
                            Remover
                          </button>
                        </li>
                      ))}
                    </ul>
                  )}

                  <p className="hint" style={{ marginTop: 12 }}>
                    LanguageTool — URL local e conta Premium
                  </p>
                  <div className="rules-form">
                    <input
                      type="text"
                      value={ltUrl}
                      onChange={(e) => setLtUrl(e.target.value)}
                      placeholder="http://localhost:8081"
                    />
                    <button
                      type="button"
                      className="secondary"
                      onClick={() => void discoverLanguageTool()}
                    >
                      Descobrir LanguageTool
                    </button>
                    <button
                      type="button"
                      className="secondary"
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
                  </div>
                  <div className="rules-form">
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
                      className="secondary"
                      onClick={() =>
                        void invoke("save_lt_premium_creds", {
                          username: ltUser,
                          apiKey: ltKey,
                        })
                          .then(() => {
                            setLtKey("");
                            setInfo("Credenciais guardadas no Mac");
                          })
                          .catch((e) => setError(errText(e)))
                      }
                    >
                      Guardar no Mac
                    </button>
                  </div>

                  <p className="hint" style={{ marginTop: 12 }}>
                    IA local — escolhas prontas de modelo.
                    {gguf?.selected ? ` Em uso: ${gguf.selected}` : ""}
                  </p>
                  {modelOffers.length > 0 ? (
                    <ul className="rules-list">
                      {modelOffers.map((offer) => (
                        <li key={offer.id}>
                          <span>
                            {offer.label} — {offer.detail}
                          </span>
                          <button
                            type="button"
                            className="secondary"
                            onClick={() => void installModelOffer(offer.id)}
                          >
                            {offer.available_locally ? "Usar" : "Baixar"}
                          </button>
                        </li>
                      ))}
                    </ul>
                  ) : (
                    <p className="hint">Nenhuma oferta disponível no momento.</p>
                  )}
                  <button
                    type="button"
                    className="secondary"
                    style={{ marginTop: 8 }}
                    onClick={() => setShowModelUrlDownload((v) => !v)}
                  >
                    {showModelUrlDownload
                      ? "Ocultar URL manual de modelo"
                      : "Mostrar URL manual de modelo"}
                  </button>
                  {showModelUrlDownload && (
                    <div className="rules-form" style={{ marginTop: 8 }}>
                      <input
                        type="text"
                        placeholder="URL do arquivo .gguf"
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
                        className="secondary"
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
                        Baixar modelo
                      </button>
                    </div>
                  )}

                  <p className="hint" style={{ marginTop: 12 }}>
                    IA na nuvem — API no formato OpenAI (URL + modelo + chave). O texto
                    sai do computador.
                  </p>
                  <div className="rules-form">
                    <input
                      type="text"
                      placeholder="URL base (…/v1)"
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
                      className="secondary"
                      onClick={() =>
                        void invoke("save_cloud_ai_settings", {
                          settings: {
                            baseUrl: cloudUrl,
                            model: cloudModel,
                            enabled: true,
                          },
                        })
                          .then(() => setInfo("IA nuvem: URL/modelo salvos"))
                          .catch((e) => setError(errText(e)))
                      }
                    >
                      Salvar URL/modelo
                    </button>
                  </div>
                  <div className="rules-form">
                    <input
                      type="password"
                      placeholder="API key"
                      value={cloudKey}
                      onChange={(e) => setCloudKey(e.target.value)}
                    />
                    <button
                      type="button"
                      className="secondary"
                      onClick={() =>
                        void invoke("save_cloud_ai_key", { apiKey: cloudKey })
                          .then(() => {
                            setCloudKey("");
                            setInfo("Chave da IA nuvem guardada no Mac");
                          })
                          .catch((e) => setError(errText(e)))
                      }
                    >
                      Guardar chave no Mac
                    </button>
                  </div>
                </>
              )}
            </section>
          </div>
        </section>

        <section className="col col-pdf">
          <div className="col-title">PDF</div>
          <div className="col-scroll">
            {showConference && result ? (
              <>
                <div className="conference-nav">
                  <button
                    type="button"
                    className="secondary"
                    disabled={confPage <= 1 || pageBusy}
                    onClick={() => setConfPage((p) => Math.max(1, p - 1))}
                  >
                    Ant
                  </button>
                  <span className="hint">
                    Página {confPage} / {result.page_count}
                  </span>
                  <button
                    type="button"
                    className="secondary"
                    disabled={confPage >= result.page_count || pageBusy}
                    onClick={() => setConfPage((p) => Math.min(result.page_count, p + 1))}
                  >
                    Próx
                  </button>
                </div>
                <div className="page-frame">
                  {pageBusy && !pageImg && <span className="hint">Carregando página…</span>}
                  {pageImg ? (
                    <img
                      src={pageImg}
                      alt={`Página ${confPage} do original`}
                      className="page-raster"
                    />
                  ) : (
                    !pageBusy && <span className="hint">Sem imagem da página.</span>
                  )}
                </div>
              </>
            ) : (
              <div className="page-frame">
                <span className="hint">Abra um PDF</span>
              </div>
            )}
          </div>
        </section>

        <section className="col col-text">
          <div className="col-title">Texto</div>
          <div className="col-scroll">
            {result ? (
              <>
                <div className="result-head">
                  <h2>
                    {queueLabel ? `${queueLabel} · ` : ""}
                    {result.source_name}
                  </h2>
                </div>

                <div className="stats">
                  <span>motor: {result.engine}</span>
                  <span>idioma: {result.languages_used}</span>
                  <span>páginas texto: {result.pages.length || "—"}</span>
                  <span>títulos: {(s.h1 ?? 0) + (s.h2 ?? 0) + (s.h3 ?? 0) + (s.h4 ?? 0)}</span>
                  <span>parágrafos: {s.prose ?? 0}</span>
                  <span>hifenizações unidas: {c.hyphenations_joined ?? 0}</span>
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

                <div className="jump">
                  <span className="hint">Texto:</span>
                  <button
                    type="button"
                    className={textView === "page" ? "active-toggle" : undefined}
                    onClick={() => setTextView("page")}
                  >
                    Página
                  </button>
                  <button
                    type="button"
                    className={textView === "book" ? "active-toggle" : undefined}
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

                <pre ref={previewRef} tabIndex={0} className="preview full conference-text">
                  {displayText}
                </pre>

                {review && (
                  <div className="review-panel">
                    <div className="rules-head">
                      <strong>Sugestões — {mapReviewEngineLabel(review.engine)}</strong>
                      <div className="actions">
                        <button
                          type="button"
                          className="secondary"
                          onClick={() =>
                            setAccepted(new Set(review.proposals.map((_, i) => i)))
                          }
                        >
                          Marcar todas
                        </button>
                        <button
                          type="button"
                          className="secondary"
                          onClick={() => setAccepted(new Set())}
                        >
                          Desmarcar
                        </button>
                        <button type="button" onClick={() => void applyAccepted()}>
                          Aplicar marcadas
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
                              />{" "}
                              <code>{p.original}</code> → <code>{p.proposed}</code>
                              <span className="hint"> — {p.reason}</span>
                            </label>
                          </li>
                        ))}
                      </ul>
                    )}
                  </div>
                )}
              </>
            ) : (
              <div className="page-frame">
                <span className="hint">Abra um PDF</span>
              </div>
            )}
          </div>
        </section>
      </div>
    </main>
  );
}

export default App;
