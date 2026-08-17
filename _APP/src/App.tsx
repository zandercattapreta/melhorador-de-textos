// ==============================================================================
// SCRIPT: App.tsx (txtmelhorator-app)
// DESCRIÇÃO: Rotina completa — fila, conferência sync, regras (R4), revisão (R5)
// CHAMADO POR: main.tsx
// CONTRATO (RESPOSTA ESPERADA): processar → revisar (aplica+Desfazer) → conferir → salvar
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
type ReviewPref = "none" | "lt" | "local_ai";
type ColumnsPref = "1" | "2+";

type JobPrefs = {
  columns: ColumnsPref;
  illustrations: boolean;
  lang: OcrLang;
  review: ReviewPref;
};

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
const LS_WIZARD_DONE = "txtmelhorator.wizard.firstDone";
const LS_JOB_PREFS = "txtmelhorator.wizard.prefs";
const CANCELLED = "CANCELLED";

const DEFAULT_PREFS: JobPrefs = {
  columns: "1",
  illustrations: false,
  lang: "por+eng",
  review: "none",
};

function loadLang(): OcrLang {
  const v = localStorage.getItem(LS_LANG);
  if (v === "auto" || v === "por" || v === "eng" || v === "por+eng") return v;
  return "por+eng";
}

function loadView(): TextView {
  return localStorage.getItem(LS_VIEW) === "book" ? "book" : "page";
}

function loadPrefs(): JobPrefs {
  try {
    const raw = localStorage.getItem(LS_JOB_PREFS);
    if (!raw) return { ...DEFAULT_PREFS, lang: loadLang() };
    const p = JSON.parse(raw) as Partial<JobPrefs>;
    const lang =
      p.lang === "auto" ||
      p.lang === "por" ||
      p.lang === "eng" ||
      p.lang === "por+eng"
        ? p.lang
        : loadLang();
    return {
      columns: p.columns === "2+" ? "2+" : "1",
      illustrations: !!p.illustrations,
      lang,
      review:
        p.review === "lt" || p.review === "local_ai" ? p.review : "none",
    };
  } catch {
    return { ...DEFAULT_PREFS, lang: loadLang() };
  }
}

function savePrefs(p: JobPrefs) {
  localStorage.setItem(LS_JOB_PREFS, JSON.stringify(p));
  localStorage.setItem(LS_LANG, p.lang);
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
  const [jobPrefs, setJobPrefs] = useState<JobPrefs>(loadPrefs);
  const [wizardOpen, setWizardOpen] = useState(
    () => localStorage.getItem(LS_WIZARD_DONE) !== "1",
  );
  const [wizardDraft, setWizardDraft] = useState<JobPrefs>(loadPrefs);
  const [wizardMode, setWizardMode] = useState<"first" | "book">("first");
  const [askBookWizard, setAskBookWizard] = useState(false);
  const [pendingOpen, setPendingOpen] = useState<"pdf" | "folder" | "drop" | null>(
    null,
  );
  const [pendingDropPath, setPendingDropPath] = useState<string | null>(null);
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
  const [acceptedTrail, setAcceptedTrail] = useState<DiffProposal[]>([]);
  const [preReview, setPreReview] = useState<{
    cleaned: string;
    pages: string[];
  } | null>(null);
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
  const prefsRef = useRef(jobPrefs);
  prefsRef.current = jobPrefs;
  const stopAllRef = useRef(false);
  const previewRef = useRef<HTMLPreElement>(null);
  /** Painel do texto ao vivo: gruda no fim (página recém-capturada) enquanto
   *  o usuário não rolar para trás; rolar de volta ao fim re-gruda. */
  const partialPaneRef = useRef<HTMLDivElement>(null);
  const partialStickRef = useRef(true);

  function handlePartialScroll() {
    const el = partialPaneRef.current;
    if (!el) return;
    partialStickRef.current =
      el.scrollHeight - el.scrollTop - el.clientHeight < 120;
  }

  useEffect(() => {
    // Texto novo chegou: acompanha o caminhar das páginas.
    const el = partialPaneRef.current;
    if (el && partialStickRef.current) {
      el.scrollTop = el.scrollHeight;
    }
  }, [partialText]);
  /** Melhor texto por página durante o OCR (bruto → rápido → LT/IA). */
  const liveReviewedRef = useRef<Map<number, string>>(new Map());
  const livePageGenRef = useRef<Map<number, number>>(new Map());
  const liveInflightRef = useRef<Set<Promise<void>>>(new Set());

  function refreshLivePartial() {
    const ordered = [...liveReviewedRef.current.entries()]
      .sort((a, b) => a[0] - b[0])
      .map(([, t]) => t);
    setPartialText(ordered.join("\n\n"));
  }

  /** Melhorize imediato da página (limpeza + estrutura + regras, SEM IA). */
  async function melhorizeChunk(text: string): Promise<string> {
    const trimmed = text.trim();
    if (!trimmed) return text;
    try {
      return await invoke<string>("melhorize_page", { text: trimmed });
    } catch {
      return trimmed;
    }
  }

  /** Mais lento: LT ou IA local (atualiza a página quando terminar). */
  async function reviewChunkDeep(
    text: string,
    rev: "lt" | "local_ai",
  ): Promise<string> {
    let trimmed = text.trim();
    if (!trimmed) return text;
    if (rev === "lt") {
      const proposals = await invoke<DiffProposal[]>("check_lt_local", {
        text: trimmed,
      });
      if (proposals.length === 0) return trimmed;
      return invoke<string>("apply_review_diffs", {
        text: trimmed,
        accepted: proposals,
      });
    }
    const report = await invoke<ReviewReport>("propose_review", {
      text: trimmed,
    });
    if (report.proposals.length === 0) return trimmed;
    return invoke<string>("apply_review_diffs", {
      text: trimmed,
      accepted: report.proposals,
    });
  }

  function bumpPageGen(page: number): number {
    const g = (livePageGenRef.current.get(page) ?? 0) + 1;
    livePageGenRef.current.set(page, g);
    return g;
  }

  function enqueueLivePageReview(page: number, raw: string) {
    const rev = prefsRef.current.review;
    const chunk = raw.trim();
    if (!chunk) return;

    const gen = bumpPageGen(page);
    // 1) Mostra bruto acumulado (não apaga páginas anteriores).
    liveReviewedRef.current.set(page, chunk);
    refreshLivePartial();

    const job = (async () => {
      if (stopAllRef.current) return;
      try {
        // 2) Melhorize imediato (limpeza+estrutura, sem IA) — a caixa mostra
        // texto já melhorado assim que a página sai da captura, SEMPRE.
        const improved = await melhorizeChunk(chunk);
        if (livePageGenRef.current.get(page) !== gen) return;
        liveReviewedRef.current.set(page, improved);
        refreshLivePartial();
        setInfo(`Página ${page} melhorada · captura segue…`);

        // 3) LT ou IA em paralelo, só se o usuário ligou (não bloqueia páginas).
        if (rev !== "lt" && rev !== "local_ai") return;
        const deep = await reviewChunkDeep(improved, rev);
        if (livePageGenRef.current.get(page) !== gen) return;
        liveReviewedRef.current.set(page, deep);
        refreshLivePartial();
        setInfo(`Página ${page} revisada · captura segue…`);
      } catch {
        if (livePageGenRef.current.get(page) !== gen) return;
        liveReviewedRef.current.set(page, chunk);
        refreshLivePartial();
      }
    })();
    liveInflightRef.current.add(job);
    void job.finally(() => liveInflightRef.current.delete(job));
  }

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
    setAcceptedTrail([]);
    setPreReview(null);
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
        if (page >= 1) setConfPage(page);
        // Melhorize sempre; LT/IA só se o usuário ligou (dentro da fila).
        enqueueLivePageReview(page, chunk);
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  async function promptAndSave(
    r: ProcessResult,
    format: "md" | "txt" | "docx",
    diffs: DiffProposal[] = acceptedTrail,
  ) {
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
        acceptedDiffs: diffs,
      });
      setSavedTo(dest);
    } catch (e) {
      setError(errText(e));
    }
  }

  /** Revisão aplica de imediato; guarda snapshot para Desfazer. */
  async function applyReviewNow(
    base: ProcessResult,
    proposals: DiffProposal[],
    engine: string,
  ): Promise<{ result: ProcessResult; applied: DiffProposal[] }> {
    if (proposals.length === 0) {
      setPreReview(null);
      setAcceptedTrail([]);
      setReview({
        proposals: [],
        vocabulary: [],
        engine,
        note: "Nenhuma correção necessária.",
      });
      setInfo("Revisão: nada a corrigir.");
      return { result: base, applied: [] };
    }
    setPreReview({ cleaned: base.cleaned, pages: base.pages });
    const next = await invoke<string>("apply_review_diffs", {
      text: base.cleaned,
      accepted: proposals,
    });
    const updated: ProcessResult = { ...base, cleaned: next, pages: [] };
    setResult(updated);
    setAcceptedTrail(proposals);
    setReview({
      proposals,
      vocabulary: [],
      engine,
      note: `${proposals.length} correção(ões) aplicadas. Use Desfazer se algo estranho.`,
    });
    setInfo(`${proposals.length} correção(ões) aplicadas.`);
    setTextView("book");
    requestAnimationFrame(() => jumpTo(0));
    return { result: updated, applied: proposals };
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
    setReview(null);
    setAcceptedTrail([]);
    setPreReview(null);
    liveReviewedRef.current.clear();
    livePageGenRef.current.clear();
    liveInflightRef.current.clear();
    partialStickRef.current = true;
    stopAllRef.current = false;
    const isPdf = path.toLowerCase().endsWith(".pdf");
    const revPref = prefsRef.current.review;
    if (isPdf) {
      // Primeira página antes do OCR (ainda sem lock no arquivo).
      setPageBusy(true);
      void invoke<string>("render_pdf_page", { path, page: 1 })
        .then((url) => setPageImg(url))
        .catch(() => undefined)
        .finally(() => setPageBusy(false));
    }
    try {
      // LT pronto antes da 1ª página — revisão roda junto com a captura.
      if (revPref === "lt") {
        setInfo("LanguageTool pronto — captura + revisão em paralelo…");
        await invoke<string>("ensure_lt_server");
      } else if (revPref === "local_ai") {
        setInfo("Captura + IA local em paralelo…");
      }

      const r = await invoke<ProcessResult>(
        isPdf ? "process_pdf" : "process_text_file",
        isPdf ? { path, languages: langRef.current } : { path },
      );

      // Espera revisões de página ainda em voo.
      await Promise.allSettled([...liveInflightRef.current]);

      let out = r;
      let applied: DiffProposal[] = [];
      setResult(r);

      // Passo final no texto já limpo/estruturado (âncora do arquivo salvo).
      if (revPref === "lt" || revPref === "local_ai") {
        setReviewBusy(true);
        try {
          if (revPref === "lt") {
            setInfo("Revisão final do livro…");
            const proposals = await invoke<DiffProposal[]>("check_lt_local", {
              text: r.cleaned,
            });
            const done = await applyReviewNow(r, proposals, "languagetool-local");
            out = done.result;
            applied = done.applied;
          } else {
            setInfo("Revisão final do livro (IA)…");
            const report = await invoke<ReviewReport>("propose_review", {
              text: r.cleaned,
            });
            const done = await applyReviewNow(
              r,
              report.proposals,
              report.engine || "ia-local",
            );
            out = done.result;
            applied = done.applied;
          }
        } catch (e) {
          setError(errText(e));
        } finally {
          setReviewBusy(false);
        }
      }

      await promptAndSave(out, "md", applied);
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
      // R5d: fila terminou — libera o GGUF residente (6 GiB) da memória.
      void invoke("unload_llama_model").catch(() => undefined);
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
      // R5d: livro único terminou — libera o GGUF residente da memória.
      void invoke("unload_llama_model").catch(() => undefined);
    },
    [processOne, runQueue],
  );

  useEffect(() => {
    const unlisten = getCurrentWebview().onDragDropEvent((event) => {
      if (event.payload.type === "enter" || event.payload.type === "over")
        setDragging(true);
      else if (event.payload.type === "drop") {
        setDragging(false);
        const path = event.payload.paths[0];
        if (!path) return;
        setPendingOpen("drop");
        setPendingDropPath(path);
        if (localStorage.getItem(LS_WIZARD_DONE) !== "1") {
          setWizardMode("first");
          setWizardDraft(loadPrefs());
          setWizardOpen(true);
        } else {
          setAskBookWizard(true);
        }
      } else setDragging(false);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  async function pickAndProcess(kind: "pdf" | "folder") {
    if (kind === "pdf") {
      const picked = await open({
        multiple: false,
        filters: [{ name: "PDF", extensions: ["pdf"] }],
        title: "Abrir PDF",
      });
      if (!picked) return;
      const path = Array.isArray(picked) ? picked[0] : picked;
      if (path) await handlePath(path);
      return;
    }
    const picked = await open({
      directory: true,
      multiple: false,
      title: "Abrir pasta com PDFs",
    });
    if (!picked) return;
    const path = Array.isArray(picked) ? picked[0] : picked;
    if (path) await handlePath(path);
  }

  function applyPrefs(p: JobPrefs) {
    setJobPrefs(p);
    setLang(p.lang);
    savePrefs(p);
    prefsRef.current = p;
    langRef.current = p.lang;
  }

  function finishWizard() {
    applyPrefs(wizardDraft);
    localStorage.setItem(LS_WIZARD_DONE, "1");
    setWizardOpen(false);
    const kind = pendingOpen;
    const drop = pendingDropPath;
    setPendingOpen(null);
    setPendingDropPath(null);
    if (kind === "drop" && drop) void handlePath(drop);
    else if (kind === "pdf" || kind === "folder") void pickAndProcess(kind);
  }

  function requestStart(kind: "pdf" | "folder" | "drop", dropPath?: string) {
    if (busy || wizardOpen || askBookWizard) return;
    const firstDone = localStorage.getItem(LS_WIZARD_DONE) === "1";
    if (!firstDone) {
      setWizardMode("first");
      setWizardDraft(jobPrefs);
      setPendingOpen(kind);
      setPendingDropPath(dropPath ?? null);
      setWizardOpen(true);
      return;
    }
    setPendingOpen(kind);
    setPendingDropPath(dropPath ?? null);
    setAskBookWizard(true);
  }

  function skipBookWizard() {
    setAskBookWizard(false);
    const kind = pendingOpen;
    const drop = pendingDropPath;
    setPendingOpen(null);
    setPendingDropPath(null);
    if (kind === "drop" && drop) void handlePath(drop);
    else if (kind === "pdf" || kind === "folder") void pickAndProcess(kind);
  }

  function openBookWizard() {
    setAskBookWizard(false);
    setWizardMode("book");
    setWizardDraft(jobPrefs);
    setWizardOpen(true);
  }

  async function openPdf() {
    requestStart("pdf");
  }

  async function openFolder() {
    requestStart("folder");
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
    setInfo("Revisando com IA local…");
    try {
      const report = await invoke<ReviewReport>("propose_review", {
        text: result.cleaned,
      });
      await applyReviewNow(result, report.proposals, report.engine || "ia-local");
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
    setInfo("Revisando com LanguageTool…");
    try {
      await invoke<string>("ensure_lt_server");
      const proposals = await invoke<DiffProposal[]>("check_lt_local", {
        text: result.cleaned,
      });
      await applyReviewNow(result, proposals, "languagetool-local");
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
      await applyReviewNow(result, proposals, "LanguageTool Premium (nuvem)");
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
      await applyReviewNow(result, report.proposals, report.engine || "cloud");
    } catch (e) {
      setError(errText(e));
    } finally {
      setReviewBusy(false);
    }
  }

  function undoReview() {
    if (!result || !preReview) return;
    setResult({
      ...result,
      cleaned: preReview.cleaned,
      pages: preReview.pages,
    });
    setAcceptedTrail([]);
    setPreReview(null);
    setReview(null);
    setInfo("Revisão desfeita.");
    setTextView("book");
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

  const fileTitle =
    result?.source_name ??
    (processingPath
      ? processingPath.split(/[/\\]/).pop() ?? processingPath
      : null);
  const progressPct =
    progress && progress.total > 0
      ? Math.min(100, Math.round((100 * progress.done) / progress.total))
      : 0;

  return (
    <main className={`shell${dragging ? " is-dragging" : ""}`}>
      <header className="topbar">
        <span className="brand">TXTMelhorator</span>
        <span className={`topbar-file${busy ? " is-busy" : ""}`}>
          {fileTitle ?? "Abra um PDF para começar"}
        </span>
      </header>

      {/* Layout B — barra única */}
      <div className="toolbar">
        <button
          type="button"
          className="btn"
          onClick={() => void openPdf()}
          disabled={busy}
        >
          Abrir PDF
        </button>
        <button
          type="button"
          className="btn ghost"
          onClick={() => void openFolder()}
          disabled={busy}
        >
          Abrir pasta
        </button>
        <span className="toolbar-sep" aria-hidden />
        <button
          type="button"
          className="btn ghost"
          disabled={!busy}
          onClick={() => void requestStop(false)}
        >
          Pular
        </button>
        <button
          type="button"
          className="btn ghost"
          disabled={!busy}
          onClick={() => void requestStop(true)}
        >
          Parar
        </button>
        <div className="toolbar-progress">
          <div className="toolbar-progress-track">
            <div
              className="toolbar-progress-fill"
              style={{ width: `${busy ? progressPct : 0}%` }}
            />
          </div>
          <span className="toolbar-progress-label">
            {busy && progress
              ? `${progress.done} / ${progress.total}`
              : busy
                ? "…"
                : queueLabel
                  ? queueLabel
                  : "—"}
          </span>
        </div>
        <label className="toolbar-ocr">
          <span>OCR</span>
          <select
            id="ocr-lang"
            value={lang}
            disabled={busy}
            onChange={(e) => {
              const next = e.target.value as OcrLang;
              applyPrefs({ ...jobPrefs, lang: next });
            }}
          >
            <option value="auto">Automático</option>
            <option value="por+eng">PT + EN</option>
            <option value="por">Português</option>
            <option value="eng">Inglês</option>
          </select>
        </label>
        <span className="toolbar-prefs hint" title="Preferências desta transcrição (wizard)">
          {jobPrefs.columns === "2+" ? "2+ col." : "1 col."}
          {" · "}
          {jobPrefs.illustrations ? "ilustrações" : "sem figuras"}
          {" · "}
          {jobPrefs.review === "lt"
            ? "LT"
            : jobPrefs.review === "local_ai"
              ? "IA local"
              : "sem revisão"}
        </span>
        <button
          type="button"
          className={`btn ghost${leftPanel === "revisao" ? " active" : ""}`}
          disabled={!result}
          onClick={() =>
            setLeftPanel((p) => (p === "revisao" ? "none" : "revisao"))
          }
        >
          Revisão
        </button>
        <button
          type="button"
          className={`btn ghost${leftPanel === "ajustes" ? " active" : ""}`}
          onClick={() =>
            setLeftPanel((p) => (p === "ajustes" ? "none" : "ajustes"))
          }
        >
          Ajustes
        </button>
      </div>

      {(error || info || savedTo) && (
        <div className="banner-strip">
          {error && <div className="banner error">{error}</div>}
          {info && <div className="banner warn">{info}</div>}
          {savedTo && <div className="banner ok">Salvo: {savedTo}</div>}
        </div>
      )}

      <div className="workspace">
        <section className="col col-pdf">
          <div className="col-head">
            <span className="col-title">Original</span>
            {showPdfPane && (
              <div className="pdf-nav">
                <button
                  type="button"
                  className="btn ghost tiny"
                  disabled={busy || confPage <= 1 || pageBusy}
                  onClick={() => setConfPage((p) => Math.max(1, p - 1))}
                >
                  Ant
                </button>
                <span className="hint">
                  {confPage}
                  {pdfPageTotal > 0 ? ` / ${pdfPageTotal}` : ""}
                  {busy ? " · lendo" : ""}
                </span>
                <button
                  type="button"
                  className="btn ghost tiny"
                  disabled={
                    busy ||
                    pdfPageTotal < 1 ||
                    confPage >= pdfPageTotal ||
                    pageBusy
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
            )}
          </div>
          {showPdfPane ? (
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
          ) : (
            <div className="page-frame empty">
              <p className="empty-hint">
                Abrir PDF / pasta — ou arraste para a janela
              </p>
            </div>
          )}
        </section>

        <section className="col col-text">
          <div className="col-head">
            <span className="col-title">Texto</span>
            {result && (
              <div className="stack-row">
                <button
                  type="button"
                  className="btn tiny"
                  onClick={() => void saveAgain("md")}
                >
                  .md
                </button>
                <button
                  type="button"
                  className="btn ghost tiny"
                  onClick={() => void saveAgain("txt")}
                >
                  .txt
                </button>
                <button
                  type="button"
                  className="btn ghost tiny"
                  onClick={() => void saveAgain("docx")}
                >
                  .docx
                </button>
              </div>
            )}
          </div>
          {busy && partialText && !result ? (
            <>
              <div className="text-toolbar">
                <p className="hint">
                  {jobPrefs.review === "none"
                    ? "Texto melhorado página a página — acabamento no final"
                    : "Texto melhorado página a página + revisão em paralelo"}
                </p>
              </div>
              <div
                className="book-pane"
                ref={partialPaneRef}
                onScroll={handlePartialScroll}
              >
                <pre className="preview partial">{partialText}</pre>
              </div>
            </>
          ) : result ? (
            <>
              <div className="text-toolbar">
                <div className="meta">
                  <span>{result.engine}</span>
                  <span>{result.languages_used}</span>
                  <span>{result.pages.length || "—"} págs.</span>
                  <span>
                    {(s.h1 ?? 0) + (s.h2 ?? 0) + (s.h3 ?? 0) + (s.h4 ?? 0)}{" "}
                    títulos
                  </span>
                  <span>{s.prose ?? 0} §</span>
                  <span>{c.hyphenations_joined ?? 0} hífens</span>
                </div>
                {acceptedTrail.length > 0 && (
                  <p className="hint">
                    {acceptedTrail.length} correção(ões) ainda não salvas
                  </p>
                )}
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
                {preReview && (
                  <p className="hint">
                    Revisão aplicada — Desfazer restaura o texto de antes.
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
                      {preReview && (
                        <button
                          type="button"
                          className="btn ghost tiny"
                          onClick={() => undoReview()}
                        >
                          Desfazer
                        </button>
                      )}
                    </div>
                  </div>
                  <p className="hint">{review.note}</p>
                  {review.proposals.length === 0 ? (
                    <p className="hint">Nada a corrigir.</p>
                  ) : (
                    <ul className="rules-list">
                      {review.proposals.map((p, i) => (
                        <li key={i}>
                          <span>
                            <code>{p.original}</code> →{" "}
                            <code>{p.proposed}</code>
                            <span className="hint"> — {p.reason}</span>
                          </span>
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

      {leftPanel !== "none" && (
        <>
          <button
            type="button"
            className="drawer-backdrop"
            aria-label="Fechar painel"
            onClick={() => setLeftPanel("none")}
          />
          <aside className="drawer" aria-label={leftPanel === "revisao" ? "Revisão" : "Ajustes"}>
            <div className="drawer-head">
              <h2>{leftPanel === "revisao" ? "Revisão" : "Ajustes"}</h2>
              <button
                type="button"
                className="btn ghost tiny"
                onClick={() => setLeftPanel("none")}
              >
                Fechar
              </button>
            </div>
            <div className="drawer-body">
              {leftPanel === "revisao" && result && (
                <div className="rail-panel">
                  <p className="hint">
                    Aplica as correções no texto. Desfazer se algo estranho.
                  </p>
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
                    <button
                      type="button"
                      className="btn ghost"
                      onClick={() => void addRule()}
                    >
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
          </aside>
        </>
      )}
      {askBookWizard && (
        <div className="modal-root" role="dialog" aria-modal="true">
          <button
            type="button"
            className="drawer-backdrop"
            aria-label="Cancelar"
            onClick={() => {
              setAskBookWizard(false);
              setPendingOpen(null);
              setPendingDropPath(null);
            }}
          />
          <div className="modal-card">
            <h2>Configurar esta transcrição?</h2>
            <p className="hint">
              Colunas, ilustrações, idioma e revisão (LanguageTool ou IA local)
              deste scan/OCR.
            </p>
            <div className="stack-row modal-actions">
              <button type="button" className="btn" onClick={() => openBookWizard()}>
                Sim
              </button>
              <button type="button" className="btn ghost" onClick={() => skipBookWizard()}>
                Não — usar última config
              </button>
            </div>
          </div>
        </div>
      )}

      {wizardOpen && (
        <div className="modal-root" role="dialog" aria-modal="true" aria-labelledby="wiz-title">
          {wizardMode !== "first" && (
            <button
              type="button"
              className="drawer-backdrop"
              aria-label="Fechar"
              onClick={() => {
                setWizardOpen(false);
                setPendingOpen(null);
                setPendingDropPath(null);
              }}
            />
          )}
          <div className="modal-card wizard-card">
            <h2 id="wiz-title">
              {wizardMode === "first"
                ? "Bem-vindo — configuração do scan"
                : "Configuração desta transcrição"}
            </h2>
            <p className="hint">
              Define como este OCR/scan será lido e revisado. Colunas e
              ilustrações guiam o app; idioma e revisão valem na hora. Revisão
              aplica e permite Desfazer.
            </p>

            <fieldset className="wiz-field">
              <legend>Colunas</legend>
              <label>
                <input
                  type="radio"
                  name="cols"
                  checked={wizardDraft.columns === "1"}
                  onChange={() =>
                    setWizardDraft((d) => ({ ...d, columns: "1" }))
                  }
                />
                Uma coluna
              </label>
              <label>
                <input
                  type="radio"
                  name="cols"
                  checked={wizardDraft.columns === "2+"}
                  onChange={() =>
                    setWizardDraft((d) => ({ ...d, columns: "2+" }))
                  }
                />
                Duas ou mais
              </label>
            </fieldset>

            <fieldset className="wiz-field">
              <legend>Ilustrações</legend>
              <label>
                <input
                  type="radio"
                  name="ill"
                  checked={!wizardDraft.illustrations}
                  onChange={() =>
                    setWizardDraft((d) => ({ ...d, illustrations: false }))
                  }
                />
                Não
              </label>
              <label>
                <input
                  type="radio"
                  name="ill"
                  checked={wizardDraft.illustrations}
                  onChange={() =>
                    setWizardDraft((d) => ({ ...d, illustrations: true }))
                  }
                />
                Sim (marcar `[figura]`)
              </label>
            </fieldset>

            <fieldset className="wiz-field">
              <legend>Língua do texto</legend>
              <select
                value={wizardDraft.lang}
                onChange={(e) =>
                  setWizardDraft((d) => ({
                    ...d,
                    lang: e.target.value as OcrLang,
                  }))
                }
              >
                <option value="auto">Automático</option>
                <option value="por+eng">Português + inglês</option>
                <option value="por">Português</option>
                <option value="eng">Inglês</option>
              </select>
            </fieldset>

            <fieldset className="wiz-field">
              <legend>Revisão após extrair</legend>
              <label>
                <input
                  type="radio"
                  name="rev"
                  checked={wizardDraft.review === "none"}
                  onChange={() =>
                    setWizardDraft((d) => ({ ...d, review: "none" }))
                  }
                />
                Nenhuma (só sob botão)
              </label>
              <label>
                <input
                  type="radio"
                  name="rev"
                  checked={wizardDraft.review === "lt"}
                  onChange={() =>
                    setWizardDraft((d) => ({ ...d, review: "lt" }))
                  }
                />
                LanguageTool local (aplica + Desfazer)
              </label>
              <label>
                <input
                  type="radio"
                  name="rev"
                  checked={wizardDraft.review === "local_ai"}
                  onChange={() =>
                    setWizardDraft((d) => ({ ...d, review: "local_ai" }))
                  }
                />
                IA local (aplica + Desfazer)
              </label>
            </fieldset>

            <div className="stack-row modal-actions">
              <button type="button" className="btn" onClick={() => finishWizard()}>
                Continuar
              </button>
              {wizardMode === "book" && (
                <button
                  type="button"
                  className="btn ghost"
                  onClick={() => {
                    setWizardOpen(false);
                    setPendingOpen(null);
                    setPendingDropPath(null);
                  }}
                >
                  Cancelar
                </button>
              )}
            </div>
          </div>
        </div>
      )}
    </main>
  );
}

export default App;
